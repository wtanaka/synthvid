//! Closed-form frame rendering.
//!
//! [`render_frame`] draws a single frame of a [`Scene`] into a fresh
//! [`Frame`]; [`render_into`] draws into a caller-supplied buffer so that a
//! sequence can reuse one allocation. Both evaluate every motion at the
//! requested frame index only, through [`position_at`],
//! so a frame never depends on any other frame.
//!
//! The draw order is fixed: the background first, in frame space, then
//! each visible [`Object`](crate::scene::Object) in declaration order, with
//! later objects overwriting earlier ones. The [`Camera`]
//! transform applies to objects only; the background is the backdrop the
//! camera looks at, not something the camera moves.
//!
//! The backdrop painters live in the `background` submodule and the shape
//! painters in the `shape` submodule; this file holds the error type,
//! the camera placement, and the two entry points. Every quantity here is
//! exact [`Ratio`] arithmetic. No floating-point operation appears anywhere
//! in this module.

mod background;
mod background_types;
mod placement;
mod shape;

use core::error::Error;
use core::fmt;

use crate::camera::{rotation_at, zoom_at, Camera, Magnification, Turns};
use crate::frame::{Frame, FrameError};
use crate::geom::{Point, Similarity};
use crate::motion::position_at;
use crate::ratio::{Overflow, Ratio};
use crate::scene::{Membership, Scene};
use crate::trig::{cos_turns, sin_turns};
use crate::units::{Dimensions, FrameIndex, ObjectIndex};

use background::paint_background;
use placement::{object_index, placement, ObjectPosition, Slot};
pub use placement::{object_states, ObjectState};
use shape::{draw_object, Brush};

/// Error returned when a frame cannot be rendered.
///
/// Rendering itself is total: backgrounds, motions, and shapes all have a
/// defined output for every input. Only the destination buffer can fail, when
/// it does not match the scene or cannot be allocated.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RenderError {
    /// The supplied buffer has different dimensions from the scene.
    ///
    /// `expected` is the scene dimensions; `actual` is the buffer dimensions.
    MismatchedDimensions {
        /// Dimensions the scene requires.
        expected: Dimensions,
        /// Dimensions the supplied buffer carries.
        actual: Dimensions,
    },
    /// The frame buffer length overflows `usize` and cannot be allocated.
    ///
    /// Unreachable for any dimension that fits in [`Dimensions`], which caps
    /// each side at `u16::MAX`, on platforms where `usize` is wider than 32
    /// bits. The variant exists so the failure stays explicit rather than
    /// hidden.
    BufferTooLarge,
    /// The camera placement for that frame could not be evaluated exactly.
    CameraOverflow {
        /// Frame whose camera placement overflowed exact arithmetic.
        frame: FrameIndex,
    },
    /// The background for that frame could not be drawn in exact arithmetic.
    BackgroundOverflow {
        /// Frame whose background extent overflowed exact arithmetic.
        frame: FrameIndex,
    },
    /// That object's placement for that frame could not be evaluated exactly.
    ObjectOverflow {
        /// Frame whose object placement overflowed exact arithmetic.
        frame: FrameIndex,
        /// Position of the failing object in `Scene::objects`.
        object: ObjectIndex,
    },
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MismatchedDimensions { expected, actual } => {
                let actual_width = actual.width.get();
                let actual_height = actual.height.get();
                let expected_width = expected.width.get();
                let expected_height = expected.height.get();
                write!(
                    f,
                    "frame buffer dimensions {actual_width}x{actual_height} do not match scene dimensions {expected_width}x{expected_height}"
                )
            }
            Self::BufferTooLarge => write!(f, "frame buffer length overflows usize"),
            Self::CameraOverflow { frame } => {
                let frame_number = frame.get();
                write!(
                    f,
                    "camera placement for frame {frame_number} overflows exact arithmetic"
                )
            }
            Self::BackgroundOverflow { frame } => {
                let frame_number = frame.get();
                write!(
                    f,
                    "background for frame {frame_number} overflows exact arithmetic"
                )
            }
            Self::ObjectOverflow { frame, object } => {
                let frame_number = frame.get();
                let object_number = object.get();
                write!(
                    f,
                    "object {object_number} placement for frame {frame_number} overflows exact arithmetic"
                )
            }
        }
    }
}

impl Error for RenderError {}

/// Camera placement evaluated for one frame.
///
/// The scene-to-screen [`Similarity`] maps a scene point `w` to
/// `spin * zoom * (w - centre)`, where `centre` is the camera motion position,
/// `zoom` is the uniform magnification, and `spin` rotates by the negation of
/// the camera rotation angle about the camera centre. Negation is the camera
/// convention: turning the camera one way moves the scene the other way.
/// A similarity can only rotate and uniformly scale, so a disc mapped through
/// it stays a disc.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct CameraFrame {
    /// Scene-to-screen transform applied to every drawn shape.
    transform: Similarity,
    /// Uniform magnification applied to shape extents; strictly positive.
    zoom: Ratio,
}

/// Evaluates the [`Camera`] at a frame index.
///
/// The rotation angle comes from [`rotation_at`] and the magnification from
/// [`zoom_at`]. A [`crate::camera::Zoom`] cannot describe a non-positive
/// magnification at any frame, so no clamp is needed here.
///
/// # Errors
///
/// Returns [`RenderError::CameraOverflow`] when any intermediate value
/// overflows exact arithmetic.
fn camera_frame(camera: &Camera, frame: FrameIndex) -> Result<CameraFrame, RenderError> {
    let overflow = |Overflow| RenderError::CameraOverflow { frame };
    let centre = position_at(&camera.motion, frame).map_err(overflow)?;
    let angle = rotation_at(&camera.rotation, frame).map_err(overflow)?;
    let magnification = zoom_at(&camera.zoom, frame).map_err(overflow)?;
    Ok(CameraFrame {
        transform: camera_transform(centre, angle, magnification).map_err(overflow)?,
        zoom: magnification.get(),
    })
}

/// The scene-to-screen similarity for a camera centred on `centre`, turned
/// by `angle`, and magnified by `magnification`.
///
/// # Errors
///
/// Returns `Err(Overflow)` when any intermediate value, or the similarity's
/// affine form, overflows exact arithmetic.
fn camera_transform(
    centre: Point,
    angle: Turns,
    magnification: Magnification,
) -> Result<Similarity, Overflow> {
    let zoom = magnification.get();
    let turn = angle.get().checked_neg()?;
    let cos = cos_turns(turn)?;
    let sin = sin_turns(turn)?;
    let neg_sin = sin.checked_neg()?;
    let scaled_cos = zoom.checked_mul(cos)?;
    let scaled_neg_sin = zoom.checked_mul(neg_sin)?;
    let scaled_sin = zoom.checked_mul(sin)?;
    let ax = scaled_cos.checked_mul(centre.across)?;
    let bx = scaled_neg_sin.checked_mul(centre.down)?;
    let tx = ax.checked_add(bx)?.checked_neg()?;
    let cx = scaled_sin.checked_mul(centre.across)?;
    let dx = scaled_cos.checked_mul(centre.down)?;
    let ty = cx.checked_add(dx)?.checked_neg()?;
    let candidate = Similarity {
        xx: scaled_cos,
        xy: scaled_neg_sin,
        tx,
        ty,
    };
    // A similarity whose affine form overflows cannot be applied, so it is
    // refused here rather than at the first shape drawn through it.
    candidate.to_affine().and(Ok(candidate))
}

/// Renders one frame of a scene into a fresh [`Frame`].
///
/// Draws the background, evaluates the camera at `frame`, then draws each
/// object whose visibility span contains `frame` in declaration order.
/// Rendering the same frame twice yields byte-identical buffers.
///
/// # Errors
///
/// Returns [`RenderError::BufferTooLarge`] when the frame buffer length
/// overflows `usize` and the fresh buffer cannot be allocated. Propagates
/// [`RenderError::CameraOverflow`] and [`RenderError::ObjectOverflow`] from
/// [`render_into`].
pub fn render_frame(scene: &Scene, frame: FrameIndex) -> Result<Frame, RenderError> {
    let mut fresh = Frame::zeroed(scene.dimensions)
        .map_err(|FrameError::InvalidBufferSize| RenderError::BufferTooLarge)?;
    render_into(scene, frame, &mut fresh)?;
    Ok(fresh)
}

/// Renders one frame of a scene into a caller-supplied buffer.
///
/// Behaves exactly like [`render_frame`], except the destination is reused:
/// the background overwrites every pixel first, so previous contents never
/// leak through. A shape that falls entirely outside the frame draws nothing;
/// only a failure to evaluate a placement is an error.
///
/// # Errors
///
/// Returns [`RenderError::MismatchedDimensions`] when `out` carries different
/// dimensions from the scene. Returns [`RenderError::CameraOverflow`] when the
/// camera placement overflows exact arithmetic. Returns
/// [`RenderError::ObjectOverflow`] when an object placement overflows exact
/// arithmetic.
pub fn render_into(scene: &Scene, frame: FrameIndex, out: &mut Frame) -> Result<(), RenderError> {
    if out.dimensions() != scene.dimensions {
        return Err(RenderError::MismatchedDimensions {
            expected: scene.dimensions,
            actual: out.dimensions(),
        });
    }
    paint_background(out, scene.background)
        .map_err(|Overflow| RenderError::BackgroundOverflow { frame })?;
    let camera = camera_frame(&scene.camera, frame)?;
    for (position, object) in scene.objects.iter().enumerate() {
        if object.visible.contains(frame) == Membership::Inside {
            let slot = Slot {
                frame,
                object: object_index(frame, ObjectPosition(position))?,
            };
            let state = placement(object, &camera, slot)?;
            let brush = Brush {
                camera: &camera,
                colour: object.fill,
                slot,
            };
            draw_object(out, &object.shape, state.centre_scene, brush)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
