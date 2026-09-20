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
use crate::ratio::{int_ratio, Ratio};
use crate::scene::Scene;
use crate::trig::{cos_turns, sin_turns};
use crate::units::{Dimensions, FrameIndex};

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
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MismatchedDimensions { expected, actual } => write!(
                f,
                "frame buffer dimensions {actual:?} do not match scene dimensions {expected:?}"
            ),
            Self::BufferTooLarge => write!(f, "frame buffer length overflows usize"),
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
/// it stays a disc; [`render_into`] falls back to the identity placement when
/// any intermediate value overflows, so evaluation stays total.
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
/// magnification at any frame, so no clamp is needed here. Falls back to the
/// identity placement when exact arithmetic overflows, so evaluation stays
/// total.
fn camera_frame(camera: &Camera, frame: FrameIndex) -> CameraFrame {
    let fallback = CameraFrame {
        transform: Similarity::identity(),
        zoom: int_ratio(1),
    };
    let centre = position_at(&camera.motion, frame);
    let Some(angle) = rotation_at(&camera.rotation, frame) else {
        return fallback;
    };
    let Some(magnification) = zoom_at(&camera.zoom, frame) else {
        return fallback;
    };
    let zoom = magnification.get();
    let angle_value = angle.get();
    let turn = angle_value.checked_neg().unwrap_or(angle_value);
    let cos = cos_turns(turn);
    let sin = sin_turns(turn);
    let Some(neg_sin) = sin.checked_neg() else {
        return fallback;
    };
    let Some(a) = zoom.checked_mul(cos) else {
        return fallback;
    };
    let Some(b) = zoom.checked_mul(neg_sin) else {
        return fallback;
    };
    let Some(c) = zoom.checked_mul(sin) else {
        return fallback;
    };
    let Some(ax) = a.checked_mul(centre.x) else {
        return fallback;
    };
    let Some(bx) = b.checked_mul(centre.y) else {
        return fallback;
    };
    let Some(sum_x) = ax.checked_add(bx) else {
        return fallback;
    };
    let Some(tx) = sum_x.checked_neg() else {
        return fallback;
    };
    let Some(cx) = c.checked_mul(centre.x) else {
        return fallback;
    };
    let Some(dx) = a.checked_mul(centre.y) else {
        return fallback;
    };
    let Some(sum_y) = cx.checked_add(dx) else {
        return fallback;
    };
    let Some(ty) = sum_y.checked_neg() else {
        return fallback;
    };
    let candidate = Similarity::new(a, b, tx, ty);
    if candidate.to_affine().is_none() {
        return fallback;
    }
    CameraFrame {
        transform: candidate,
        zoom,
    }
}

/// Renders one frame of a scene into a fresh [`Frame`].
///
/// Draws the background, evaluates the camera at `frame`, then draws each
/// object whose visibility span contains `frame` in declaration order.
/// Rendering the same frame twice yields byte-identical buffers. Fails only
/// when the fresh buffer cannot be allocated; see [`RenderError`].
///
/// # Errors
///
/// Returns [`RenderError::BufferTooLarge`] when the frame buffer length
/// overflows `usize` and the fresh buffer cannot be allocated.
pub fn render_frame(scene: &Scene, frame: FrameIndex) -> Result<Frame, RenderError> {
    let Some(mut fresh) = Frame::zeroed(scene.dimensions) else {
        return Err(RenderError::BufferTooLarge);
    };
    render_into(scene, frame, &mut fresh)?;
    Ok(fresh)
}

/// Renders one frame of a scene into a caller-supplied buffer.
///
/// Behaves exactly like [`render_frame`], except the destination is reused:
/// the background overwrites every pixel first, so previous contents never
/// leak through. Fails with [`RenderError::MismatchedDimensions`] when `out`
/// does not match the scene dimensions.
///
/// # Errors
///
/// Returns [`RenderError::MismatchedDimensions`] when `out` carries different
/// dimensions from the scene.
pub fn render_into(scene: &Scene, frame: FrameIndex, out: &mut Frame) -> Result<(), RenderError> {
    if out.dimensions() != scene.dimensions {
        return Err(RenderError::MismatchedDimensions {
            expected: scene.dimensions,
            actual: out.dimensions(),
        });
    }
    paint_background(out, scene.background);
    let camera = camera_frame(&scene.camera, frame);
    for object in &scene.objects {
        if object.visible.contains(frame) {
            let at = position_at(&object.motion, frame);
            draw_object(out, &object.shape, at, &camera, object.fill);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Magnification, Rotation, Turns, Zoom};
    use crate::color::Rgb8;
    use crate::geom::Point;
    use crate::scene::{Background, FrameSpan, Motion, Object, Shape};
    use crate::testutil::make_ratio;
    use crate::units::{FrameCount, FrameRate, Height, Seed, Width};

    /// Builds a camera that follows `origin` with no rotation and unit magnification.
    fn still_camera_at(origin: Point) -> Option<Camera> {
        let angle = Turns::new(make_ratio(0, 1)?);
        let unit = Zoom::Fixed(Magnification::new(make_ratio(1, 1)?)?);
        Some(Camera::new(
            Motion::Fixed(origin),
            Rotation::Fixed(angle),
            unit,
        ))
    }

    /// Builds a camera fixed at the scene origin with unit magnification, which is the identity placement.
    fn identity_camera() -> Option<Camera> {
        let zero = make_ratio(0, 1)?;
        still_camera_at(Point::new(zero, zero))
    }

    /// Builds a scene with the given backdrop and objects on an 8x8 frame.
    fn small_scene(background: Background, objects: Vec<Object>) -> Option<Scene> {
        let width = Width::new(8)?;
        let height = Height::new(8)?;
        let rate = FrameRate::from_fps(30)?;
        let count = FrameCount::new(4)?;
        let camera = identity_camera()?;
        Some(Scene::new(
            Dimensions::new(width, height),
            rate,
            count,
            None,
            background,
            camera,
            objects,
        ))
    }

    /// Builds a visibility span from `start` (inclusive) to `end` (exclusive).
    fn make_span(start: u32, end: u32) -> Option<FrameSpan> {
        FrameSpan::new(FrameIndex::new(start), FrameIndex::new(end))
    }

    /// Builds a disc object fixed at integer `(x, y)` with the given radius.
    fn fixed_disc(x: i64, y: i64, radius: i64, fill: Rgb8, span: FrameSpan) -> Option<Object> {
        let px = Ratio::from_integer(x)?;
        let py = Ratio::from_integer(y)?;
        let radius_ratio = Ratio::from_integer(radius)?;
        Some(Object::new(
            Shape::Disc {
                radius: radius_ratio,
            },
            fill,
            Motion::Fixed(Point::new(px, py)),
            span,
        ))
    }

    #[test]
    fn test_render_frame_twice_byte_identical() {
        let red = Rgb8::new(200, 40, 40);
        let green = Rgb8::new(20, 180, 60);
        let Some(span) = make_span(0, 4) else { return };
        let Some(first) = fixed_disc(3, 3, 2, red, span) else {
            return;
        };
        let Some(second) = fixed_disc(6, 6, 3, green, span) else {
            return;
        };
        let seed_bg = Background::Blobs {
            seed: Seed::new(9),
            count: 6,
            min_radius: 1,
            max_radius: 3,
        };
        let Some(scene) = small_scene(seed_bg, vec![first, second]) else {
            return;
        };
        for n in 0..4_u32 {
            let frame = FrameIndex::new(n);
            let Ok(once) = render_frame(&scene, frame) else {
                return;
            };
            let Ok(twice) = render_frame(&scene, frame) else {
                return;
            };
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
        let Some(span) = make_span(0, 4) else { return };
        let Some(object) = fixed_disc(4, 4, 3, yellow, span) else {
            return;
        };
        let Some(scene) = small_scene(Background::Solid(blue), vec![object]) else {
            return;
        };
        let frame = FrameIndex::new(1);
        let Ok(fresh) = render_frame(&scene, frame) else {
            return;
        };
        let Some(mut dirty) = Frame::zeroed(scene.dimensions) else {
            return;
        };
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
        let Some(scene) = small_scene(Background::Solid(Rgb8::new(1, 2, 3)), Vec::new()) else {
            return;
        };
        let Some(wide) = Width::new(16) else { return };
        let tall = scene.dimensions.height;
        let wrong = Dimensions::new(wide, tall);
        let Some(mut out) = Frame::zeroed(wrong) else {
            return;
        };
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
        let Some(empty) = small_scene(ground, Vec::new()) else {
            return;
        };
        let Some(hidden_span) = make_span(2, 3) else {
            return;
        };
        let Some(object) = fixed_disc(4, 4, 3, Rgb8::new(9, 9, 9), hidden_span) else {
            return;
        };
        let Some(with_hidden) = small_scene(ground, vec![object]) else {
            return;
        };
        let Ok(background_only) = render_frame(&empty, FrameIndex::new(0)) else {
            return;
        };
        let Ok(with_objects) = render_frame(&with_hidden, FrameIndex::new(0)) else {
            return;
        };
        assert_eq!(
            background_only.data(),
            with_objects.data(),
            "an object outside its span must draw nothing"
        );
        let Ok(shown) = render_frame(&with_hidden, FrameIndex::new(2)) else {
            return;
        };
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
        let Some(span) = make_span(0, 4) else { return };
        let Some(under) = fixed_disc(4, 4, 3, red, span) else {
            return;
        };
        let Some(over) = fixed_disc(4, 4, 3, blue, span) else {
            return;
        };
        let Some(first_wins) =
            small_scene(Background::Solid(black), vec![over.clone(), under.clone()])
        else {
            return;
        };
        let Some(second_wins) = small_scene(Background::Solid(black), vec![under, over]) else {
            return;
        };
        let Ok(first) = render_frame(&first_wins, FrameIndex::new(0)) else {
            return;
        };
        let Ok(second) = render_frame(&second_wins, FrameIndex::new(0)) else {
            return;
        };
        assert_eq!(
            first.pixel(4, 4),
            Some(red),
            "the later declaration must win the shared pixel"
        );
        assert_eq!(
            second.pixel(4, 4),
            Some(blue),
            "swapping the order must swap the winning colour"
        );
    }

    #[test]
    fn test_identity_camera_maps_scene_to_pixels() {
        let black = Rgb8::new(0, 0, 0);
        let red = Rgb8::new(255, 0, 0);
        let Some(span) = make_span(0, 4) else { return };
        let (Some(cx), Some(cy), Some(rr)) = (make_ratio(5, 2), make_ratio(5, 2), make_ratio(1, 1))
        else {
            return;
        };
        let object = Object::new(
            Shape::Disc { radius: rr },
            red,
            Motion::Fixed(Point::new(cx, cy)),
            span,
        );
        let Some(scene) = small_scene(Background::Solid(black), vec![object]) else {
            return;
        };
        let Ok(frame) = render_frame(&scene, FrameIndex::new(0)) else {
            return;
        };
        assert_eq!(
            frame.pixel(2, 2),
            Some(red),
            "a disc at (2.5, 2.5) must cover pixel (2, 2)"
        );
        assert_eq!(
            frame.pixel(0, 0),
            Some(black),
            "a disc at (2.5, 2.5) must not reach pixel (0, 0)"
        );
    }

    #[test]
    fn test_camera_translation_shifts_objects() {
        let black = Rgb8::new(0, 0, 0);
        let red = Rgb8::new(255, 0, 0);
        let Some(span) = make_span(0, 4) else { return };
        let Some(half) = make_ratio(5, 2) else { return };
        let Some(radius) = make_ratio(1, 1) else {
            return;
        };
        let object = Object::new(
            Shape::Disc { radius },
            red,
            Motion::Fixed(Point::new(half, half)),
            span,
        );
        let Some(two) = make_ratio(2, 1) else { return };
        let Some(zero) = make_ratio(0, 1) else { return };
        let Some(one) = make_ratio(1, 1) else { return };
        let Some(unit) = Magnification::new(one) else {
            return;
        };
        let shifted_camera = Camera::new(
            Motion::Fixed(Point::new(two, zero)),
            Rotation::Fixed(Turns::new(zero)),
            Zoom::Fixed(unit),
        );
        let (Some(w), Some(h)) = (Width::new(8), Height::new(8)) else {
            return;
        };
        let Some(rate) = FrameRate::from_fps(30) else {
            return;
        };
        let Some(count) = FrameCount::new(4) else {
            return;
        };
        let scene = Scene::new(
            Dimensions::new(w, h),
            rate,
            count,
            None,
            Background::Solid(black),
            shifted_camera,
            vec![object],
        );
        let Ok(frame) = render_frame(&scene, FrameIndex::new(0)) else {
            return;
        };
        assert_eq!(
            frame.pixel(0, 2),
            Some(red),
            "moving the camera by (2, 0) must shift the disc left by two pixels"
        );
        assert_eq!(
            frame.pixel(2, 2),
            Some(black),
            "the disc must vacate the pixel it covered before the shift"
        );
    }
}
