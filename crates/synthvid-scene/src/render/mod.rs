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
mod shape;

use core::fmt;

use crate::camera::{rotation_at, zoom_at, Camera};
use crate::frame::Frame;
use crate::geom::Similarity;
use crate::motion::position_at;
use crate::ratio::Ratio;
use crate::scene::Scene;
use crate::trig::{cos_turns, sin_turns};
use crate::units::{Dimensions, FrameIndex, ObjectIndex};

use background::paint_background;
use shape::draw_object;

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
            Self::MismatchedDimensions { expected, actual } => write!(
                f,
                "frame buffer dimensions {actual:?} do not match scene dimensions {expected:?}"
            ),
            Self::BufferTooLarge => write!(f, "frame buffer length overflows usize"),
            Self::CameraOverflow { frame } => write!(
                f,
                "camera placement for frame {frame:?} overflows exact arithmetic"
            ),
            Self::BackgroundOverflow { frame } => write!(
                f,
                "background for frame {frame:?} overflows exact arithmetic"
            ),
            Self::ObjectOverflow { frame, object } => write!(
                f,
                "object {object:?} placement for frame {frame:?} overflows exact arithmetic"
            ),
        }
    }
}

impl core::error::Error for RenderError {}

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
    let overflow = || RenderError::CameraOverflow { frame };
    let centre = position_at(&camera.motion, frame).map_err(|_| overflow())?;
    let angle = rotation_at(&camera.rotation, frame).map_err(|_| overflow())?;
    let magnification = zoom_at(&camera.zoom, frame).map_err(|_| overflow())?;
    let zoom = magnification.get();
    let angle_value = angle.get();
    let Ok(turn) = angle_value.checked_neg() else {
        return Err(overflow());
    };
    let Ok(cos) = cos_turns(turn) else {
        return Err(overflow());
    };
    let Ok(sin) = sin_turns(turn) else {
        return Err(overflow());
    };
    let Ok(neg_sin) = sin.checked_neg() else {
        return Err(overflow());
    };
    let Ok(a) = zoom.checked_mul(cos) else {
        return Err(overflow());
    };
    let Ok(b) = zoom.checked_mul(neg_sin) else {
        return Err(overflow());
    };
    let Ok(c) = zoom.checked_mul(sin) else {
        return Err(overflow());
    };
    let Ok(ax) = a.checked_mul(centre.x) else {
        return Err(overflow());
    };
    let Ok(bx) = b.checked_mul(centre.y) else {
        return Err(overflow());
    };
    let Ok(sum_x) = ax.checked_add(bx) else {
        return Err(overflow());
    };
    let Ok(tx) = sum_x.checked_neg() else {
        return Err(overflow());
    };
    let Ok(cx) = c.checked_mul(centre.x) else {
        return Err(overflow());
    };
    let Ok(dx) = a.checked_mul(centre.y) else {
        return Err(overflow());
    };
    let Ok(sum_y) = cx.checked_add(dx) else {
        return Err(overflow());
    };
    let Ok(ty) = sum_y.checked_neg() else {
        return Err(overflow());
    };
    let candidate = Similarity::new(a, b, tx, ty);
    if candidate.to_affine().is_err() {
        return Err(overflow());
    }
    Ok(CameraFrame {
        transform: candidate,
        zoom,
    })
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
    let mut fresh = Frame::zeroed(scene.dimensions).map_err(|_| RenderError::BufferTooLarge)?;
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
        .map_err(|_| RenderError::BackgroundOverflow { frame })?;
    let camera = camera_frame(&scene.camera, frame)?;
    for (index, object) in scene.objects.iter().enumerate() {
        if object.visible.contains(frame) {
            let Some(index_u32) = u32::try_from(index).ok() else {
                return Err(RenderError::ObjectOverflow {
                    frame,
                    object: ObjectIndex::new(u32::MAX),
                });
            };
            let object_index = ObjectIndex::new(index_u32);
            let at =
                position_at(&object.motion, frame).map_err(|_| RenderError::ObjectOverflow {
                    frame,
                    object: object_index,
                })?;
            draw_object(
                out,
                &object.shape,
                at,
                &camera,
                object.fill,
                frame,
                object_index,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Magnification, Rotation, Turns, Zoom};
    use crate::color::Rgb8;
    use crate::frame::PixelCoord;
    use crate::geom::Point;
    use crate::scene::{Background, FrameSpan, Motion, Object, Shape};
    use crate::testutil::make_ratio;
    use crate::units::{FrameCount, FrameIndex, FrameRate, Height, ObjectIndex, Seed, Width};

    /// Builds a camera that follows `origin` with no rotation and unit magnification.
    fn still_camera_at(origin: Point) -> Camera {
        let angle = Turns::new(make_ratio(0, 1).unwrap());
        let unit = Zoom::Fixed(Magnification::new(make_ratio(1, 1).unwrap()).unwrap());
        Camera::new(Motion::Fixed(origin), Rotation::Fixed(angle), unit)
    }

    /// Builds a camera fixed at the scene origin with unit magnification, which is the identity placement.
    fn identity_camera() -> Camera {
        let zero = make_ratio(0, 1).unwrap();
        still_camera_at(Point::new(zero, zero))
    }

    /// Builds a scene with the given backdrop and objects on an 8x8 frame.
    fn small_scene(background: Background, objects: Vec<Object>) -> Scene {
        let width = Width::new(8).unwrap();
        let height = Height::new(8).unwrap();
        let rate = FrameRate::from_fps(30).unwrap();
        let count = FrameCount::new(4).unwrap();
        let camera = identity_camera();
        Scene::new(
            Dimensions::new(width, height),
            rate,
            count,
            None,
            background,
            camera,
            objects,
        )
    }

    /// Builds a visibility span from `start` (inclusive) to `end` (exclusive).
    fn make_span(start: u32, end: u32) -> FrameSpan {
        FrameSpan::new(FrameIndex::new(start), FrameIndex::new(end)).unwrap()
    }

    /// Builds a disc object fixed at integer `(x, y)` with the given radius.
    fn fixed_disc(x: i64, y: i64, radius: i64, fill: Rgb8, span: FrameSpan) -> Object {
        let px = Ratio::from_integer(x);
        let py = Ratio::from_integer(y);
        let radius_ratio = Ratio::from_integer(radius);
        Object::new(
            Shape::Disc {
                radius: radius_ratio,
            },
            fill,
            Motion::Fixed(Point::new(px, py)),
            span,
        )
    }

    #[test]
    fn test_render_frame_twice_byte_identical() {
        let red = Rgb8::new(200, 40, 40);
        let green = Rgb8::new(20, 180, 60);
        let span = make_span(0, 4);
        let first = fixed_disc(3, 3, 2, red, span);
        let second = fixed_disc(6, 6, 3, green, span);
        let seed_bg = Background::Blobs {
            seed: Seed::new(9),
            count: 6,
            min_radius: 1,
            max_radius: 3,
        };
        let scene = small_scene(seed_bg, vec![first, second]);
        for n in 0..4_u32 {
            let frame = FrameIndex::new(n);
            let once = render_frame(&scene, frame).unwrap();
            let twice = render_frame(&scene, frame).unwrap();
            assert_eq!(
                once.data(),
                twice.data(),
                "rendering frame {n} twice must be byte-identical"
            );
        }
    }

    #[test]
    fn test_render_into_dirty_buffer_equals_fresh() {
        let blue = Rgb8::new(30, 60, 220);
        let yellow = Rgb8::new(230, 210, 40);
        let span = make_span(0, 4);
        let object = fixed_disc(4, 4, 3, yellow, span);
        let scene = small_scene(Background::Solid(blue), vec![object]);
        let frame = FrameIndex::new(1);
        let fresh = render_frame(&scene, frame).unwrap();
        let mut dirty = Frame::zeroed(scene.dimensions).unwrap();
        dirty.fill(Rgb8::new(255, 0, 255));
        assert!(
            render_into(&scene, frame, &mut dirty).is_ok(),
            "rendering into a matching buffer must succeed"
        );
        assert_eq!(
            dirty.data(),
            fresh.data(),
            "rendering into a dirty buffer must equal a fresh render"
        );
    }

    #[test]
    fn test_render_into_rejects_mismatched_dimensions() {
        let scene = small_scene(Background::Solid(Rgb8::new(1, 2, 3)), Vec::new());
        let wide = Width::new(16).unwrap();
        let tall = scene.dimensions.height;
        let wrong = Dimensions::new(wide, tall);
        let mut out = Frame::zeroed(wrong).unwrap();
        assert_eq!(
            render_into(&scene, FrameIndex::new(0), &mut out),
            Err(RenderError::MismatchedDimensions {
                expected: scene.dimensions,
                actual: wrong,
            }),
            "a buffer with different dimensions must be rejected"
        );
    }

    #[test]
    fn test_empty_scene_equals_hidden_objects() {
        let ground = Background::Solid(Rgb8::new(12, 34, 56));
        let empty = small_scene(ground, Vec::new());
        let hidden_span = make_span(2, 3);
        let object = fixed_disc(4, 4, 3, Rgb8::new(9, 9, 9), hidden_span);
        let with_hidden = small_scene(ground, vec![object]);
        let background_only = render_frame(&empty, FrameIndex::new(0)).unwrap();
        let with_objects = render_frame(&with_hidden, FrameIndex::new(0)).unwrap();
        assert_eq!(
            background_only.data(),
            with_objects.data(),
            "an object outside its span must draw nothing"
        );
        let shown = render_frame(&with_hidden, FrameIndex::new(2)).unwrap();
        assert!(
            shown.data() != background_only.data(),
            "an object inside its span must change the frame"
        );
    }

    #[test]
    fn test_declaration_order_last_wins() {
        let black = Rgb8::new(0, 0, 0);
        let red = Rgb8::new(255, 0, 0);
        let blue = Rgb8::new(0, 0, 255);
        let span = make_span(0, 4);
        let under = fixed_disc(4, 4, 3, red, span);
        let over = fixed_disc(4, 4, 3, blue, span);
        let first_wins = small_scene(Background::Solid(black), vec![over.clone(), under.clone()]);
        let second_wins = small_scene(Background::Solid(black), vec![under, over]);
        let first = render_frame(&first_wins, FrameIndex::new(0)).unwrap();
        let second = render_frame(&second_wins, FrameIndex::new(0)).unwrap();
        assert_eq!(
            first.pixel(PixelCoord::new(4, 4)),
            Some(red),
            "the later declaration must win the shared pixel"
        );
        assert_eq!(
            second.pixel(PixelCoord::new(4, 4)),
            Some(blue),
            "swapping the order must swap the winning colour"
        );
    }

    #[test]
    fn test_camera_overflow_names_frame() {
        let scene = small_scene(Background::Solid(Rgb8::new(0, 0, 0)), Vec::new());
        let max = make_ratio(i64::MAX, 1).unwrap();
        let one = make_ratio(1, 1).unwrap();
        let zero = make_ratio(0, 1).unwrap();
        let unit = Magnification::new(one).unwrap();
        let camera = Camera::new(
            Motion::Fixed(Point::new(one, zero)),
            Rotation::Linear {
                start: Turns::new(max),
                per_frame: Turns::new(max),
            },
            Zoom::Fixed(unit),
        );
        let mut overflowing = scene;
        overflowing.camera = camera;
        let result = render_frame(&overflowing, FrameIndex::new(1));
        assert!(
            matches!(
                result,
                Err(RenderError::CameraOverflow { frame: actual }) if actual == FrameIndex::new(1)
            ),
            "a camera placement overflow must name the frame"
        );
    }

    #[test]
    fn test_object_overflow_names_object_index() {
        let span = make_span(0, 4);
        let valid = fixed_disc(1, 1, 1, Rgb8::new(200, 40, 40), span);
        let max = make_ratio(i64::MAX, 1).unwrap();
        let zero = make_ratio(0, 1).unwrap();
        let one = make_ratio(1, 1).unwrap();
        let overflow = Object::new(
            Shape::Rect {
                half_width: one,
                half_height: one,
            },
            Rgb8::new(20, 180, 60),
            Motion::Fixed(Point::new(max, zero)),
            span,
        );
        let overflowing = small_scene(Background::Solid(Rgb8::new(0, 0, 0)), vec![valid, overflow]);
        let result = render_frame(&overflowing, FrameIndex::new(0));
        assert!(
            matches!(
                result,
                Err(RenderError::ObjectOverflow {
                    frame: actual_frame,
                    object: actual_object
                }) if actual_frame == FrameIndex::new(0)
                    && actual_object == ObjectIndex::new(1)
            ),
            "an object placement overflow must name the frame and object index"
        );
    }

    #[test]
    fn test_off_screen_shape_renders_successfully() {
        let span = make_span(0, 4);
        let off = fixed_disc(100, 100, 1, Rgb8::new(255, 0, 0), span);
        let scene = small_scene(Background::Solid(Rgb8::new(0, 0, 0)), vec![off]);
        let frame = render_frame(&scene, FrameIndex::new(0)).unwrap();
        let expected = Frame::from_color(scene.dimensions, Rgb8::new(0, 0, 0)).unwrap();
        assert_eq!(
            frame.data(),
            expected.data(),
            "a shape entirely outside the frame must draw nothing"
        );
    }

    #[test]
    fn test_no_objects_equals_background() {
        let scene = small_scene(Background::Solid(Rgb8::new(12, 34, 56)), Vec::new());
        let frame = render_frame(&scene, FrameIndex::new(0)).unwrap();
        let expected = Frame::from_color(scene.dimensions, Rgb8::new(12, 34, 56)).unwrap();
        assert_eq!(
            frame.data(),
            expected.data(),
            "a scene with no objects must equal its background alone"
        );
    }

    #[test]
    fn test_identity_camera_maps_scene_to_pixels() {
        let black = Rgb8::new(0, 0, 0);
        let red = Rgb8::new(255, 0, 0);
        let span = make_span(0, 4);
        let cx = make_ratio(5, 2).unwrap();
        let cy = make_ratio(5, 2).unwrap();
        let rr = make_ratio(1, 1).unwrap();
        let object = Object::new(
            Shape::Disc { radius: rr },
            red,
            Motion::Fixed(Point::new(cx, cy)),
            span,
        );
        let scene = small_scene(Background::Solid(black), vec![object]);
        let frame = render_frame(&scene, FrameIndex::new(0)).unwrap();
        assert_eq!(
            frame.pixel(PixelCoord::new(2, 2)),
            Some(red),
            "a disc at (2.5, 2.5) must cover pixel (2, 2)"
        );
        assert_eq!(
            frame.pixel(PixelCoord::new(0, 0)),
            Some(black),
            "a disc at (2.5, 2.5) must not reach pixel (0, 0)"
        );
    }

    #[test]
    fn test_camera_translation_shifts_objects() {
        let black = Rgb8::new(0, 0, 0);
        let red = Rgb8::new(255, 0, 0);
        let span = make_span(0, 4);
        let half = make_ratio(5, 2).unwrap();
        let radius = make_ratio(1, 1).unwrap();
        let object = Object::new(
            Shape::Disc { radius },
            red,
            Motion::Fixed(Point::new(half, half)),
            span,
        );
        let two = make_ratio(2, 1).unwrap();
        let zero = make_ratio(0, 1).unwrap();
        let one = make_ratio(1, 1).unwrap();
        let unit = Magnification::new(one).unwrap();
        let shifted_camera = Camera::new(
            Motion::Fixed(Point::new(two, zero)),
            Rotation::Fixed(Turns::new(zero)),
            Zoom::Fixed(unit),
        );
        let w = Width::new(8).unwrap();
        let h = Height::new(8).unwrap();
        let rate = FrameRate::from_fps(30).unwrap();
        let count = FrameCount::new(4).unwrap();
        let scene = Scene::new(
            Dimensions::new(w, h),
            rate,
            count,
            None,
            Background::Solid(black),
            shifted_camera,
            vec![object],
        );
        let frame = render_frame(&scene, FrameIndex::new(0)).unwrap();
        assert_eq!(
            frame.pixel(PixelCoord::new(0, 2)),
            Some(red),
            "moving the camera by (2, 0) must shift the disc left by two pixels"
        );
        assert_eq!(
            frame.pixel(PixelCoord::new(2, 2)),
            Some(black),
            "the disc must vacate the pixel it covered before the shift"
        );
    }
}
