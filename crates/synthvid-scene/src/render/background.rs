//! Backdrop painting for frame rendering.
//!
//! [`paint_background`] fills the whole frame from a [`Background`], in frame
//! space and without touching the camera transform. Every pixel is written,
//! so no previous buffer contents can survive. All arithmetic is exact; no
//! floating-point operation appears here.

use crate::color::Rgb8;
use crate::frame::Frame;
use crate::geom::Point;
use crate::ratio::Ratio;
use crate::rng::Rng;
use crate::scene::{Background, Direction};
use crate::units::Seed;

use super::super::raster::fill_disc;

/// Black ground painted before the deterministic discs of [`Background::Blobs`].
const BLOB_GROUND: Rgb8 = Rgb8::new(0, 0, 0);

/// Returns the checker colour at pixel `(x, y)`.
///
/// The frame is tiled with squares of side `side` pixels; the square at the
/// origin takes `a`, and colours alternate like a chessboard. `side` is never
/// zero by construction, so the divisions below always succeed.
const fn checker_colour(x: u16, y: u16, side: u16, a: Rgb8, b: Rgb8) -> Rgb8 {
    let Some(qx) = x.checked_div(side) else {
        return a;
    };
    let Some(qy) = y.checked_div(side) else {
        return a;
    };
    let Some(sum) = qx.checked_add(qy) else {
        return a;
    };
    let Some(rem) = sum.checked_rem(2) else {
        return a;
    };
    if rem == 0 {
        a
    } else {
        b
    }
}

/// Interpolates one 8-bit channel between `from` and `to`.
///
/// `num / den` is the position along the gradient with `0 <= num <= den` and
/// `den >= 1`. The value is `(from * (den - num) + to * num + den / 2) / den`,
/// computed in `u32` with symmetric rounding, so the start edge is exactly
/// `from` and the far edge is exactly `to`.
fn gradient_channel(from: u8, to: u8, num: u32, den: u32) -> u8 {
    let from_wide = u32::from(from);
    let to_wide = u32::from(to);
    let Some(rest) = den.checked_sub(num) else {
        return from;
    };
    let Some(first) = from_wide.checked_mul(rest) else {
        return from;
    };
    let Some(second) = to_wide.checked_mul(num) else {
        return from;
    };
    let Some(total) = first.checked_add(second) else {
        return from;
    };
    let Some(half) = den.checked_div(2) else {
        return from;
    };
    let Some(rounded) = total.checked_add(half) else {
        return from;
    };
    let Some(value) = rounded.checked_div(den) else {
        return from;
    };
    let Some(byte) = u8::try_from(value).ok() else {
        return from;
    };
    byte
}

/// Paints a [`Background::Checker`] backdrop over the whole frame.
fn paint_checker(frame: &mut Frame, side: u16, a: Rgb8, b: Rgb8) {
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    for y in 0..height {
        for x in 0..width {
            let _ = frame.set_pixel(x, y, checker_colour(x, y, side, a, b));
        }
    }
}

/// Paints a [`Background::Gradient`] backdrop over the whole frame.
///
/// Along the gradient axis the first row or column is exactly `from` and the
/// last is exactly `to`; see [`gradient_channel`]. A frame one pixel wide or
/// tall along that axis has no distance to interpolate over and is filled
/// with `from`.
fn paint_gradient(frame: &mut Frame, from: Rgb8, to: Rgb8, direction: Direction) {
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    let limit = match direction {
        Direction::Horizontal => width,
        Direction::Vertical => height,
    };
    if limit <= 1 {
        frame.fill(from);
        return;
    }
    let den = u32::from(limit).checked_sub(1).unwrap_or(1);
    for y in 0..height {
        for x in 0..width {
            let num = match direction {
                Direction::Horizontal => u32::from(x),
                Direction::Vertical => u32::from(y),
            };
            let colour = Rgb8::new(
                gradient_channel(from.r, to.r, num, den),
                gradient_channel(from.g, to.g, num, den),
                gradient_channel(from.b, to.b, num, den),
            );
            let _ = frame.set_pixel(x, y, colour);
        }
    }
}

/// Draws one deterministic disc of a [`Background::Blobs`] backdrop.
///
/// The centre is uniform over the frame, the radius is uniform in
/// `[lo, hi]`, and each channel is uniform in `[0, 255]`, all drawn in that
/// order from the supplied generator. A blob whose values overflow is
/// skipped, leaving the ground untouched at its position.
fn draw_blob(frame: &mut Frame, rng: &mut Rng, width: u16, height: u16, lo: u16, hi: u16) {
    let raw_x = rng.next_bounded(u64::from(width));
    let raw_y = rng.next_bounded(u64::from(height));
    let Some(cx) = u16::try_from(raw_x).ok() else {
        return;
    };
    let Some(cy) = u16::try_from(raw_y).ok() else {
        return;
    };
    let Some(extra) = u32::from(hi)
        .checked_sub(u32::from(lo))
        .and_then(|gap| gap.checked_add(1))
    else {
        return;
    };
    let raw_r = rng.next_bounded(u64::from(extra));
    let Some(step) = u32::try_from(raw_r).ok() else {
        return;
    };
    let Some(radius_wide) = u32::from(lo).checked_add(step) else {
        return;
    };
    let Some(radius_u16) = u16::try_from(radius_wide).ok() else {
        return;
    };
    let Some(red) = u8::try_from(rng.next_bounded(256)).ok() else {
        return;
    };
    let Some(green) = u8::try_from(rng.next_bounded(256)).ok() else {
        return;
    };
    let Some(blue) = u8::try_from(rng.next_bounded(256)).ok() else {
        return;
    };
    let Some(px) = Ratio::from_integer(i64::from(cx)) else {
        return;
    };
    let Some(py) = Ratio::from_integer(i64::from(cy)) else {
        return;
    };
    let Some(radius) = Ratio::from_integer(i64::from(radius_u16)) else {
        return;
    };
    fill_disc(
        frame,
        Point::new(px, py),
        radius,
        Rgb8::new(red, green, blue),
    );
}

/// Paints a [`Background::Blobs`] backdrop over the whole frame.
///
/// Black ground first, then `count` discs from [`draw_blob`] in generator
/// order, so the same seed always yields the same discs. When `min_radius`
/// exceeds `max_radius` the bounds are swapped rather than rejected, keeping
/// evaluation total.
fn paint_blobs(frame: &mut Frame, seed: Seed, count: u16, min_radius: u16, max_radius: u16) {
    frame.fill(BLOB_GROUND);
    let (lo, hi) = if min_radius <= max_radius {
        (min_radius, max_radius)
    } else {
        (max_radius, min_radius)
    };
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    let mut rng = Rng::from_seed(seed);
    for _ in 0..count {
        draw_blob(frame, &mut rng, width, height, lo, hi);
    }
}

/// Paints a [`Background::Grid`] backdrop over the whole frame.
///
/// Pixels whose `x` or `y` index is a multiple of `spacing` take the line
/// colour; every other pixel takes the ground colour. Lines are one pixel
/// wide.
fn paint_grid(frame: &mut Frame, spacing: u16, line: Rgb8, ground: Rgb8) {
    frame.fill(ground);
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    for y in 0..height {
        for x in 0..width {
            let Some(rx) = x.checked_rem(spacing) else {
                continue;
            };
            let Some(ry) = y.checked_rem(spacing) else {
                continue;
            };
            if rx == 0 || ry == 0 {
                let _ = frame.set_pixel(x, y, line);
            }
        }
    }
}

/// Paints the [`Background`] over the whole frame.
///
/// Every pixel is written, so no previous buffer contents can survive. The
/// background is drawn in frame space and never passes through the camera
/// transform.
pub(super) fn paint_background(frame: &mut Frame, background: Background) {
    match background {
        Background::Solid(colour) => frame.fill(colour),
        Background::Checker { cell, a, b } => paint_checker(frame, cell.get(), a, b),
        Background::Gradient {
            from,
            to,
            direction,
        } => paint_gradient(frame, from, to, direction),
        Background::Blobs {
            seed,
            count,
            min_radius,
            max_radius,
        } => paint_blobs(frame, seed, count, min_radius, max_radius),
        Background::Grid {
            spacing,
            line,
            ground,
        } => paint_grid(frame, spacing.get(), line, ground),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Camera, Magnification, Rotation, Turns, Zoom};
    use crate::scene::{Motion, Scene};
    use crate::testutil::make_ratio;
    use crate::units::{Dimensions, FrameCount, FrameRate, Height, Width};

    /// Builds the identity camera: still at the origin with unit magnification.
    fn identity_camera() -> Option<Camera> {
        let zero = make_ratio(0, 1)?;
        let angle = Turns::new(zero);
        let unit = Zoom::Fixed(Magnification::new(make_ratio(1, 1)?)?);
        Some(Camera::new(
            Motion::Fixed(Point::new(zero, zero)),
            Rotation::Fixed(angle),
            unit,
        ))
    }

    /// Builds an 8x8 scene with the given backdrop and no objects.
    fn bare_scene(background: Background) -> Option<Scene> {
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
            Vec::new(),
        ))
    }

    #[test]
    fn test_checker_pixels() {
        let cell = core::num::NonZeroU16::new(2).unwrap_or(core::num::NonZeroU16::MIN);
        let red = Rgb8::new(255, 0, 0);
        let blue = Rgb8::new(0, 0, 255);
        let Some(scene) = bare_scene(Background::Checker {
            cell,
            a: red,
            b: blue,
        }) else {
            return;
        };
        let Some(mut frame) = Frame::zeroed(scene.dimensions) else {
            return;
        };
        paint_background(&mut frame, scene.background);
        assert_eq!(frame.pixel(0, 0), Some(red), "origin square must be a");
        assert_eq!(
            frame.pixel(2, 0),
            Some(blue),
            "next square across must be b"
        );
        assert_eq!(frame.pixel(0, 2), Some(blue), "next square down must be b");
        assert_eq!(
            frame.pixel(2, 2),
            Some(red),
            "diagonal square must be a again"
        );
    }

    #[test]
    fn test_grid_pixels() {
        let spacing = core::num::NonZeroU16::new(4).unwrap_or(core::num::NonZeroU16::MIN);
        let white = Rgb8::new(255, 255, 255);
        let black = Rgb8::new(0, 0, 0);
        let Some(scene) = bare_scene(Background::Grid {
            spacing,
            line: white,
            ground: black,
        }) else {
            return;
        };
        let Some(mut frame) = Frame::zeroed(scene.dimensions) else {
            return;
        };
        paint_background(&mut frame, scene.background);
        assert_eq!(frame.pixel(0, 3), Some(white), "column zero must be a line");
        assert_eq!(frame.pixel(3, 0), Some(white), "row zero must be a line");
        assert_eq!(
            frame.pixel(1, 1),
            Some(black),
            "off-line pixels must be ground"
        );
        assert_eq!(
            frame.pixel(4, 4),
            Some(white),
            "multiples of spacing must be lines"
        );
    }

    #[test]
    fn test_gradient_edges() {
        let from = Rgb8::new(10, 20, 30);
        let to = Rgb8::new(200, 180, 160);
        let Some(scene) = bare_scene(Background::Gradient {
            from,
            to,
            direction: Direction::Horizontal,
        }) else {
            return;
        };
        let Some(mut frame) = Frame::zeroed(scene.dimensions) else {
            return;
        };
        paint_background(&mut frame, scene.background);
        assert_eq!(
            frame.pixel(0, 0),
            Some(from),
            "gradient start edge must be from"
        );
        assert_eq!(frame.pixel(7, 0), Some(to), "gradient far edge must be to");
        let middle = frame.pixel(3, 0);
        assert!(
            middle.is_some() && middle != Some(from) && middle != Some(to),
            "gradient interior must lie strictly between its edges"
        );
    }

    #[test]
    fn test_blobs_deterministic() {
        let blobs = Background::Blobs {
            seed: Seed::new(1234),
            count: 9,
            min_radius: 1,
            max_radius: 4,
        };
        let Some(scene) = bare_scene(blobs) else {
            return;
        };
        let Some(mut once) = Frame::zeroed(scene.dimensions) else {
            return;
        };
        let Some(mut twice) = Frame::zeroed(scene.dimensions) else {
            return;
        };
        paint_background(&mut once, scene.background);
        paint_background(&mut twice, scene.background);
        assert_eq!(
            once.data(),
            twice.data(),
            "blob layout from one seed must paint identically twice"
        );
        let Some(mut ground_only) = Frame::zeroed(scene.dimensions) else {
            return;
        };
        paint_background(
            &mut ground_only,
            Background::Blobs {
                seed: Seed::new(1234),
                count: 0,
                min_radius: 1,
                max_radius: 4,
            },
        );
        assert!(
            ground_only.data() != once.data(),
            "a zero-count blob field must differ from a populated one"
        );
    }
}
