//! Rasterization of geometry onto a frame.
//!
//! The `fill_*` and `draw_*` entry points decide pixel coverage by testing
//! exact pixel centers against exact geometry, and clip to the frame before
//! iterating, so a shape far outside the frame costs nothing. The predicates
//! that answer those questions live in the private `coverage` submodule.

mod args;
mod coverage;
mod coverage_types;
pub mod extent;

pub use args::{Corners, Cross, HalfSize, Segment};
pub use extent::{
    cross_extent, disc_extent, line_extent, polygon_extent, rect_extent, Bounds, BoundsError,
    BoundsOrEmpty,
};

use crate::color::Rgb8;
use crate::frame::{Frame, PixelColumn, PixelCoord, PixelOutOfBounds, PixelRow};
use crate::geom::Point;
use crate::ratio::{half_ratio, Numerator, Overflow, Ratio};
use coverage::{
    axis_range, clipped_pixel_range, disc_covers, line_covers, pixel_center, point_in_polygon,
    PixelSpan,
};
use coverage_types::{Column, Inclusion, Interval, Reach, Row};

/// The scene-space box a shape occupies, one closed interval per axis.
#[derive(Copy, Clone, Debug)]
struct Region {
    /// The horizontal extent.
    across: Interval,
    /// The vertical extent.
    down: Interval,
}

/// Paints `colour` onto every pixel of `frame` whose centre lies in `region`
/// and is reported inside by `covers`.
///
/// `region` is clipped to the frame with `clipped_pixel_range` before any
/// pixel is visited, so a region far outside the frame costs nothing.
fn paint_covered(
    frame: &mut Frame,
    region: Region,
    colour: Rgb8,
    mut covers: impl FnMut(Point) -> Result<Inclusion, Overflow>,
) -> Result<(), Overflow> {
    let (columns_first, columns_last) = match clipped_pixel_range(region.across, frame.width().0)? {
        PixelSpan::Empty => return Ok(()),
        PixelSpan::Covering { span } => span.0.into_inner(),
    };
    let (rows_first, rows_last) = match clipped_pixel_range(region.down, frame.height().0)? {
        PixelSpan::Empty => return Ok(()),
        PixelSpan::Covering { span } => span.0.into_inner(),
    };
    let mut row = rows_first;
    loop {
        let mut column = columns_first;
        loop {
            if covers(pixel_center(Column(column), Row(row)))? == Inclusion::Inside {
                frame
                    .set_pixel(PixelCoord::new(PixelColumn(column), PixelRow(row)), colour)
                    .map_err(|PixelOutOfBounds| Overflow)?;
            }
            if column == columns_last {
                break;
            }
            let Some(east) = column.checked_add(1) else {
                break;
            };
            column = east;
        }
        if row == rows_last {
            break;
        }
        let Some(south) = row.checked_add(1) else {
            break;
        };
        row = south;
    }
    Ok(())
}

/// Fills the pixels whose centers fall inside the disc.
///
/// The disc is `{ p : |p - centre|^2 <= radius^2 }`, tested with exact
/// [`Ratio`] arithmetic. A non-positive radius draws nothing. The bounding
/// box `[centre - radius, centre + radius]` is clipped to the frame with
/// `clipped_pixel_range` before any pixel is visited.
fn fill_disc_inner(
    frame: &mut Frame,
    centre: Point,
    radius: Ratio,
    colour: Rgb8,
) -> Result<(), Overflow> {
    if radius <= Ratio::from_integer(Numerator(0)) {
        return Ok(());
    }
    let region = Region {
        across: axis_range(Reach {
            centre: centre.across,
            radius,
        })?,
        down: axis_range(Reach {
            centre: centre.down,
            radius,
        })?,
    };
    paint_covered(frame, region, colour, |point| {
        disc_covers(centre, radius, point)
    })
}

/// Fills the pixels whose centers fall inside the polygon.
///
/// See `point_in_polygon` for the documented, exact edge rule: centers on
/// an edge count as inside, otherwise the even-odd rule with a half-open
/// vertex rule decides. Polygons with fewer than three vertices draw
/// nothing. The vertex bounding box is clipped to the frame before any
/// pixel is visited.
fn fill_polygon_inner(frame: &mut Frame, vertices: &[Point], colour: Rgb8) -> Result<(), Overflow> {
    if vertices.len() < 3 {
        return Ok(());
    }
    let mut iter = vertices.iter();
    let first = iter.next().ok_or(Overflow)?;
    let mut across = Interval {
        lo: first.across,
        hi: first.across,
    };
    let mut down = Interval {
        lo: first.down,
        hi: first.down,
    };
    for vertex in iter {
        if vertex.across < across.lo {
            across.lo = vertex.across;
        }
        if across.hi < vertex.across {
            across.hi = vertex.across;
        }
        if vertex.down < down.lo {
            down.lo = vertex.down;
        }
        if down.hi < vertex.down {
            down.hi = vertex.down;
        }
    }
    paint_covered(frame, Region { across, down }, colour, |point| {
        point_in_polygon(point, vertices)
    })
}

/// Fills the axis-aligned rectangle from `min` to `max`.
///
/// The rectangle is the set `{ p : min.across <= p.across <= max.across,
/// min.down <= p.down <= max.down }`, using the same inclusive edge rule as
/// [`fill_polygon`]. When `max` lies strictly below `min` on either axis the
/// rectangle is empty and nothing is drawn. The implementation fills through
/// the identical polygon path with corners `min`, `(max.across, min.down)`, `max`,
/// `(min.across, max.down)`, so a rectangle and its equivalent four-vertex polygon
/// always produce byte-identical frames.
fn fill_rect_inner(frame: &mut Frame, rect: Corners, colour: Rgb8) -> Result<(), Overflow> {
    let Corners { min, max } = rect;
    if max.across < min.across {
        return Ok(());
    }
    if max.down < min.down {
        return Ok(());
    }
    let corners = [
        min,
        Point {
            across: max.across,
            down: min.down,
        },
        max,
        Point {
            across: min.across,
            down: max.down,
        },
    ];
    fill_polygon_inner(frame, &corners, colour)
}

/// The bounding box of `segment`, widened by half a unit on every side.
fn line_region(segment: Segment) -> Result<Region, Overflow> {
    let Segment { start, end } = segment;
    let widened = |ends: (Ratio, Ratio)| -> Result<Interval, Overflow> {
        let (lo, hi) = if ends.0 < ends.1 {
            ends
        } else {
            (ends.1, ends.0)
        };
        Ok(Interval {
            lo: lo.checked_sub(half_ratio())?,
            hi: hi.checked_add(half_ratio())?,
        })
    };
    Ok(Region {
        across: widened((start.across, end.across))?,
        down: widened((start.down, end.down))?,
    })
}

/// Draws a 1-unit-thick line segment from `start` to `end`.
///
/// A pixel is drawn when its center lies within `1 / 2` of the segment,
/// tested with exact [`Ratio`] arithmetic (see `line_covers`). A
/// zero-length segment draws the disc of radius `1 / 2` around the point.
/// The segment bounding box expanded by half a unit is clipped to the frame
/// before any pixel is visited.
fn draw_line_inner(frame: &mut Frame, segment: Segment, colour: Rgb8) -> Result<(), Overflow> {
    paint_covered(frame, line_region(segment)?, colour, |point| {
        line_covers(segment, point)
    })
}

/// The rectangle reaching `half.width` either side of `centre`
/// horizontally and `half.height` either side vertically.
fn centred_box(centre: Point, half: HalfSize) -> Result<Corners, Overflow> {
    Ok(Corners {
        min: Point {
            across: centre.across.checked_sub(half.width)?,
            down: centre.down.checked_sub(half.height)?,
        },
        max: Point {
            across: centre.across.checked_add(half.width)?,
            down: centre.down.checked_add(half.height)?,
        },
    })
}

/// Draws an axis-aligned cross centered at `centre`.
///
/// `arm` is the half-length from the center to the tip of each bar along its
/// axis; `thickness` is the full width of each bar. The cross is the union of
/// the horizontal bar `[cx - arm, cx + arm]` by
/// `[cy - thickness / 2, cy + thickness / 2]` and the vertical bar
/// `[cx - thickness / 2, cx + thickness / 2]` by `[cy - arm, cy + arm]`,
/// each filled with the inclusive rectangle rule of [`fill_rect`]. A
/// negative `arm` or a non-positive `thickness` draws nothing.
fn draw_cross_inner(frame: &mut Frame, cross: Cross, colour: Rgb8) -> Result<(), Overflow> {
    let Cross {
        centre,
        arm,
        thickness,
    } = cross;
    if arm < Ratio::from_integer(Numerator(0)) {
        return Ok(());
    }
    if thickness <= Ratio::from_integer(Numerator(0)) {
        return Ok(());
    }
    let half_thick = thickness.checked_div(Ratio::from_integer(Numerator(2)))?;
    let horizontal = HalfSize {
        width: arm,
        height: half_thick,
    };
    let vertical = HalfSize {
        width: half_thick,
        height: arm,
    };
    fill_rect_inner(frame, centred_box(centre, horizontal)?, colour)?;
    fill_rect_inner(frame, centred_box(centre, vertical)?, colour)?;
    Ok(())
}

/// Fills the pixels whose centers fall inside the disc.
///
/// A non-positive radius, or a disc entirely off screen, draws nothing and
/// returns `Ok`. Returns `Err(Overflow)` when the bounding box cannot be
/// computed in exact arithmetic.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the extent overflows exact arithmetic.
pub fn fill_disc(
    frame: &mut Frame,
    centre: Point,
    radius: Ratio,
    colour: Rgb8,
) -> Result<(), Overflow> {
    fill_disc_inner(frame, centre, radius, colour)
}

/// Fills the pixels whose centers fall inside the polygon.
///
/// Fewer than three vertices, or a polygon entirely off screen, draws nothing
/// and returns `Ok`.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the extent overflows exact arithmetic.
pub fn fill_polygon(frame: &mut Frame, vertices: &[Point], colour: Rgb8) -> Result<(), Overflow> {
    fill_polygon_inner(frame, vertices, colour)
}

/// Fills the axis-aligned rectangle from `min` to `max`.
///
/// An inverted or off-screen rectangle draws nothing and returns `Ok`.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the extent overflows exact arithmetic.
pub fn fill_rect(frame: &mut Frame, rect: Corners, colour: Rgb8) -> Result<(), Overflow> {
    fill_rect_inner(frame, rect, colour)
}

/// Draws a 1-unit-thick line segment from `start` to `end`.
///
/// A segment entirely off screen draws nothing and returns `Ok`.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the extent overflows exact arithmetic.
pub fn draw_line(frame: &mut Frame, segment: Segment, colour: Rgb8) -> Result<(), Overflow> {
    draw_line_inner(frame, segment, colour)
}

/// Draws an axis-aligned cross centred at `centre`.
///
/// A negative `arm` or a non-positive `thickness` draws nothing and returns
/// `Ok`.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the extent overflows exact arithmetic.
pub fn draw_cross(frame: &mut Frame, cross: Cross, colour: Rgb8) -> Result<(), Overflow> {
    draw_cross_inner(frame, cross, colour)
}

#[cfg(test)]
mod tests;
