//! Backdrop painting for frame rendering.
//!
//! [`paint_background`] fills the whole frame from a [`Background`], in frame
//! space and without touching the view transform. Every pixel is written,
//! so no previous buffer contents can survive. All arithmetic is exact; no
//! floating-point operation appears here.

use core::num::{NonZeroU16, NonZeroU32, NonZeroU64};

use crate::color::{Channel, Rgb8};
use crate::frame::{Frame, PixelColumn, PixelCoord, PixelOutOfBounds, PixelRow};
use crate::geom::Point;
use crate::ratio::{Numerator, Overflow, Ratio};
use crate::rng::{BoundedDraw, DrawBound, Rng};
use crate::scene::{Background, BlobCount, BlobRadii, BlobRadius, Direction};
use crate::units::Seed;

use super::super::raster::fill_disc;
use super::background_types::{CheckerPattern, GradientEnds, GridPattern};

/// Black ground painted before the deterministic discs of [`Background::Blobs`].
const BLOB_GROUND: Rgb8 = Rgb8 {
    red: Channel(0),
    green: Channel(0),
    blue: Channel(0),
};

/// Paints a [`Background::Checker`] backdrop over the whole frame.
///
/// Squares are tiled from the origin. Two tile indices share the even colour
/// exactly when their parities agree, which is the parity of their sum, so
/// the sum itself is never formed and cannot overflow.
fn paint_checker(frame: &mut Frame, pattern: &CheckerPattern) -> Result<(), Overflow> {
    let side = pattern.cell.get();
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    for row in 0..height {
        for column in 0..width {
            let column_odd = column.div_euclid(side).rem_euclid(2) == 1;
            let row_odd = row.div_euclid(side).rem_euclid(2) == 1;
            let colour = if column_odd == row_odd {
                pattern.even
            } else {
                pattern.odd
            };
            frame
                .set_pixel(PixelCoord::new(PixelColumn(column), PixelRow(row)), colour)
                .map_err(|PixelOutOfBounds| Overflow)?;
        }
    }
    Ok(())
}

/// Paints a [`Background::Gradient`] backdrop over the whole frame.
///
/// Along the gradient axis the first row or column is exactly `from` and the
/// last is exactly `to`. Each channel is `(from * (den - num) + to * num +
/// den / 2) / den`, computed in `u32` with symmetric rounding, where `num` is
/// the pixel's index along the axis and `den` is the last index. A frame one
/// pixel wide or tall along that axis has no distance to interpolate over and
/// is filled with `from`.
///
/// # Errors
///
/// Returns `Err(Overflow)` if an intermediate value does not fit its integer type.
fn paint_gradient(
    frame: &mut Frame,
    ends: &GradientEnds,
    direction: Direction,
) -> Result<(), Overflow> {
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    let limit = match direction {
        Direction::Horizontal => width,
        Direction::Vertical => height,
    };
    let Some(span) = limit.checked_sub(1).and_then(NonZeroU16::new) else {
        frame.fill(ends.from);
        return Ok(());
    };
    let den = NonZeroU32::from(span).get();
    let half = den.div_euclid(2);
    let starts = [ends.from.red.0, ends.from.green.0, ends.from.blue.0];
    let finals = [ends.to.red.0, ends.to.green.0, ends.to.blue.0];
    for row in 0..height {
        for column in 0..width {
            let along = match direction {
                Direction::Horizontal => column,
                Direction::Vertical => row,
            };
            let num = u32::from(along);
            let rest = den.checked_sub(num).ok_or(Overflow)?;
            let mut channels = [0_u8; 3];
            for ((slot, start), end) in channels.iter_mut().zip(starts).zip(finals) {
                let first = u32::from(start).checked_mul(rest).ok_or(Overflow)?;
                let second = u32::from(end).checked_mul(num).ok_or(Overflow)?;
                let total = first.checked_add(second).ok_or(Overflow)?;
                let rounded = total.checked_add(half).ok_or(Overflow)?;
                let value = rounded.div_euclid(den);
                *slot = u8::try_from(value).map_err(Overflow::from)?;
            }
            let colour = Rgb8 {
                red: Channel(channels[0]),
                green: Channel(channels[1]),
                blue: Channel(channels[2]),
            };
            frame
                .set_pixel(PixelCoord::new(PixelColumn(column), PixelRow(row)), colour)
                .map_err(|PixelOutOfBounds| Overflow)?;
        }
    }
    Ok(())
}

/// Paints a [`Background::Grid`] backdrop over the whole frame.
///
/// Pixels whose `x` or `y` index is a multiple of `spacing` take the line
/// colour; every other pixel takes the ground colour. Lines are one pixel
/// wide.
fn paint_grid(frame: &mut Frame, grid: &GridPattern) -> Result<(), Overflow> {
    frame.fill(grid.ground);
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    let spacing = grid.spacing.get();
    for row in 0..height {
        for column in 0..width {
            if column.rem_euclid(spacing) == 0 || row.rem_euclid(spacing) == 0 {
                frame
                    .set_pixel(
                        PixelCoord::new(PixelColumn(column), PixelRow(row)),
                        grid.line,
                    )
                    .map_err(|PixelOutOfBounds| Overflow)?;
            }
        }
    }
    Ok(())
}

/// Paints black ground and then `count` generator-drawn discs; see
/// [`paint_background`] for the draw order.
fn paint_blobs(
    frame: &mut Frame,
    seed: Seed,
    blob_count: BlobCount,
    radii: BlobRadii,
) -> Result<(), Overflow> {
    let BlobCount(count) = blob_count;
    frame.fill(BLOB_GROUND);
    let BlobRadius(lo) = radii.min();
    let BlobRadius(hi) = radii.max();
    let across = DrawBound(NonZeroU64::from(frame.width().get()));
    let down = DrawBound(NonZeroU64::from(frame.height().get()));
    let channel_bound = DrawBound(NonZeroU64::from(
        NonZeroU16::MIN.checked_add(255).ok_or(Overflow)?,
    ));
    let mut rng = Rng::from_seed(seed);
    for _ in 0..count {
        let BoundedDraw(raw_x) = rng.next_bounded(across);
        let BoundedDraw(raw_y) = rng.next_bounded(down);
        let cx = u16::try_from(raw_x).map_err(Overflow::from)?;
        let cy = u16::try_from(raw_y).map_err(Overflow::from)?;
        let extra = u32::from(hi)
            .checked_sub(u32::from(lo))
            .and_then(|gap| NonZeroU32::MIN.checked_add(gap))
            .ok_or(Overflow)?;
        let BoundedDraw(raw_r) = rng.next_bounded(DrawBound(NonZeroU64::from(extra)));
        let step = u32::try_from(raw_r).map_err(Overflow::from)?;
        let radius_wide = u32::from(lo).checked_add(step).ok_or(Overflow)?;
        let radius_u16 = u16::try_from(radius_wide).map_err(Overflow::from)?;
        let red = u8::try_from(rng.next_bounded(channel_bound).0).map_err(Overflow::from)?;
        let green = u8::try_from(rng.next_bounded(channel_bound).0).map_err(Overflow::from)?;
        let blue = u8::try_from(rng.next_bounded(channel_bound).0).map_err(Overflow::from)?;
        let px = Ratio::from_integer(Numerator(i64::from(cx)));
        let py = Ratio::from_integer(Numerator(i64::from(cy)));
        let radius = Ratio::from_integer(Numerator(i64::from(radius_u16)));
        fill_disc(
            frame,
            Point {
                across: px,
                down: py,
            },
            radius,
            Rgb8 {
                red: Channel(red),
                green: Channel(green),
                blue: Channel(blue),
            },
        )?;
    }
    Ok(())
}

/// Paints the [`Background`] over the whole frame.
///
/// Every pixel is written, so no previous buffer contents can survive. The
/// background is drawn in frame space and never passes through the view
/// transform.
///
/// A blob backdrop first paints black ground, then `count` discs in generator
/// order, so the same seed always yields the same discs. The radius range is
/// ordered by construction, so evaluation is total. Each disc draws its centre uniform over the frame, its
/// radius uniform in `[lo, hi]`, and each channel uniform in `[0, 255]`, in
/// that order. A blob whose values overflow is skipped, leaving the ground
/// untouched at its position.
pub(super) fn paint_background(frame: &mut Frame, background: Background) -> Result<(), Overflow> {
    match background {
        Background::Solid(colour) => {
            frame.fill(colour);
            Ok(())
        }
        Background::Checker { cell, even, odd } => {
            paint_checker(frame, &CheckerPattern { cell, even, odd })
        }
        Background::Gradient {
            from,
            to,
            direction,
        } => paint_gradient(frame, &GradientEnds { from, to }, direction),
        Background::Blobs { seed, count, radii } => paint_blobs(frame, seed, count, radii),
        Background::Grid {
            spacing,
            line,
            ground,
        } => paint_grid(
            frame,
            &GridPattern {
                spacing,
                line,
                ground,
            },
        ),
    }
}

#[cfg(test)]
mod tests;
