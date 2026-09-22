//! Exact coverage predicates and clipping used by the rasterisers.
//!
//! Nothing here touches a [`Frame`](crate::Frame). These are the pure
//! geometric questions a rasteriser asks -- which pixel indices can a shape
//! possibly touch, and does this exact pixel centre lie inside that shape --
//! kept separate so the drawing entry points in the parent module stay short
//! enough to read.

use crate::geom::{Point, Vector};
use crate::ratio::{
    ceil_ratio, floor_ratio, half_ratio, int_ratio, pixel_centre_ratio, quarter_ratio, Overflow,
    Ratio,
};

/// The pixel indices a shape's extent covers on one axis.
///
/// An empty result and an arithmetic failure are different outcomes and must
/// not share a representation. A shape that lies off screen, or whose extent
/// contains no pixel centre, draws nothing and that is the correct answer. An
/// extent that overflows exact arithmetic is a failure the caller must hear
/// about. Collapsing both into `None` is what let an overflow be silently
/// rendered as a missing shape.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum PixelSpan {
    /// No pixel centre falls within the extent; nothing is drawn.
    Empty,
    /// The inclusive range of pixel indices to visit, with `first <= last`.
    Covering {
        /// First pixel index, inclusive.
        first: u16,
        /// Last pixel index, inclusive.
        last: u16,
    },
}

/// Converts an inclusive scene-coordinate interval into clipped pixel indices.
///
/// # Errors
///
/// Returns `Err(Overflow)` only when the conversion overflows exact arithmetic.
/// An interval that is inverted, contains no pixel centre, or falls entirely
/// outside `0..limit` yields [`PixelSpan::Empty`].
pub(super) fn clipped_pixel_range(lo: Ratio, hi: Ratio, limit: u16) -> Result<PixelSpan, Overflow> {
    if hi < lo {
        return Ok(PixelSpan::Empty);
    }
    let alpha = ceil_ratio(lo.checked_sub(half_ratio())?)?;
    let omega = floor_ratio(hi.checked_sub(half_ratio())?)?;
    if omega < alpha {
        return Ok(PixelSpan::Empty);
    }
    let top = i64::from(limit).checked_sub(1).ok_or(Overflow)?;
    let begin = if alpha < 0 { 0 } else { alpha };
    let finish = if omega > top { top } else { omega };
    if finish < begin {
        return Ok(PixelSpan::Empty);
    }
    Ok(PixelSpan::Covering {
        first: u16::try_from(begin).ok().ok_or(Overflow)?,
        last: u16::try_from(finish).ok().ok_or(Overflow)?,
    })
}

/// Returns the exact centre of the pixel with the given indices.
///
/// The centre is `(x + 1 / 2, y + 1 / 2)` as [`Ratio`]s. A `u16` index cannot
/// overflow the numerator, so this is total.
pub(super) fn pixel_centre(x: u16, y: u16) -> Point {
    Point {
        x: pixel_centre_ratio(x),
        y: pixel_centre_ratio(y),
    }
}

/// Returns the exact cross product for the segment test.
///
/// Computes `(b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)`.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn segment_cross(p: Point, a: Point, b: Point) -> Result<Ratio, Overflow> {
    let bx_ax = b.x.checked_sub(a.x)?;
    let py_ay = p.y.checked_sub(a.y)?;
    let left = bx_ax.checked_mul(py_ay)?;
    let by_ay = b.y.checked_sub(a.y)?;
    let px_ax = p.x.checked_sub(a.x)?;
    let right = by_ay.checked_mul(px_ax)?;
    left.checked_sub(right)
}

/// Tests whether `p` lies exactly on the closed segment `a` to `b`.
///
/// Collinearity comes from [`segment_cross`] and containment from exact
/// comparisons.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn point_on_segment(p: Point, a: Point, b: Point) -> Result<bool, Overflow> {
    let cross = segment_cross(p, a, b)?;
    if cross != int_ratio(0) {
        return Ok(false);
    }
    if p.x < a.x && p.x < b.x {
        return Ok(false);
    }
    if a.x < p.x && b.x < p.x {
        return Ok(false);
    }
    if p.y < a.y && p.y < b.y {
        return Ok(false);
    }
    if a.y < p.y && b.y < p.y {
        return Ok(false);
    }
    Ok(true)
}

/// Tests whether `p` lies on any edge of the polygon.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn point_on_polygon_edge(p: Point, vertices: &[Point]) -> Result<bool, Overflow> {
    let mut iter = vertices.iter();
    let Some(first) = iter.next() else {
        return Ok(false);
    };
    let mut prev = *first;
    for current in iter {
        if point_on_segment(p, prev, *current)? {
            return Ok(true);
        }
        prev = *current;
    }
    point_on_segment(p, prev, *first)
}

/// Counts ray crossings of the polygon with the horizontal ray from `p`.
///
/// Uses the half-open rule that an edge counts exactly when one endpoint is
/// at or below `p.y` and the other is strictly above it. Edges whose
/// intersection overflows are treated as non-crossing. The count parity is
/// independent of vertex order and of edge direction.
///
/// # Errors
///
/// Returns `Err(Overflow)` only if the counter itself would overflow `usize` (unreachable for real polygons).
pub(super) fn ray_crossings(p: Point, vertices: &[Point]) -> Result<usize, Overflow> {
    let mut iter = vertices.iter();
    let Some(first) = iter.next() else {
        return Ok(0);
    };
    let mut prev = *first;
    let mut count: usize = 0;
    for current in iter {
        let next_count = edge_crossing(p, prev, *current, count)?;
        count = next_count;
        prev = *current;
    }
    edge_crossing(p, prev, *first, count)
}

/// Adds one crossing for the directed edge `a` to `b` when appropriate.
///
/// Returns the updated count.
///
/// # Errors
///
/// Returns `Err(Overflow)` only if the counter itself would overflow `usize` (unreachable for real polygons).
pub(super) fn edge_crossing(p: Point, a: Point, b: Point, count: usize) -> Result<usize, Overflow> {
    let a_below = a.y <= p.y;
    let b_below = b.y <= p.y;
    if a_below == b_below {
        return Ok(count);
    }
    let num = p.y.checked_sub(a.y)?;
    let den = b.y.checked_sub(a.y)?;
    let t = num.checked_div(den)?;
    let dx = b.x.checked_sub(a.x)?;
    let shift = t.checked_mul(dx)?;
    let x_int = a.x.checked_add(shift)?;
    if x_int <= p.x {
        return Ok(count);
    }
    count.checked_add(1).ok_or(Overflow)
}

/// Tests whether a point is inside a polygon.
///
/// # Exact edge rule
///
/// A point lying exactly on any edge counts as inside. Otherwise the
/// even-odd rule applies: a ray from the point in the positive `x` direction
/// is cast, and the point is inside when the number of crossings is odd.
/// Edges are counted with the half-open rule (the lower endpoint inclusive,
/// the upper exclusive), so a ray through a vertex counts exactly once and
/// the result never depends on floating-point rounding, vertex order, or
/// winding direction. All arithmetic is exact [`Ratio`] arithmetic.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn point_in_polygon(p: Point, vertices: &[Point]) -> Result<bool, Overflow> {
    if vertices.len() < 3 {
        return Ok(false);
    }
    if point_on_polygon_edge(p, vertices)? {
        return Ok(true);
    }
    let crossings = ray_crossings(p, vertices)?;
    let rem = crossings.checked_rem(2).ok_or(Overflow)?;
    Ok(rem == 1)
}

/// Returns the displacement from `start` to `tip`.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn vector_between(start: Point, tip: Point) -> Result<Vector, Overflow> {
    Ok(Vector {
        x: tip.x.checked_sub(start.x)?,
        y: tip.y.checked_sub(start.y)?,
    })
}

/// Returns the squared Euclidean length of a displacement.
///
/// Computes `x * x + y * y` exactly.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn squared_length(delta: Vector) -> Result<Ratio, Overflow> {
    let xx = delta.x.checked_mul(delta.x)?;
    let yy = delta.y.checked_mul(delta.y)?;
    xx.checked_add(yy)
}

/// Returns the exact dot product of two displacements.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn dot_product(first: Vector, second: Vector) -> Result<Ratio, Overflow> {
    let xx = first.x.checked_mul(second.x)?;
    let yy = first.y.checked_mul(second.y)?;
    xx.checked_add(yy)
}

/// Returns the inclusive interval centred on a coordinate.
///
/// Computes `(centre - radius, centre + radius)`.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn axis_range(centre_coord: Ratio, radius: Ratio) -> Result<(Ratio, Ratio), Overflow> {
    Ok((
        centre_coord.checked_sub(radius)?,
        centre_coord.checked_add(radius)?,
    ))
}

/// Returns the point `centre` shifted by the given offsets.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn shifted_point(
    centre: Point,
    delta_x: Ratio,
    delta_y: Ratio,
) -> Result<Point, Overflow> {
    Ok(Point {
        x: centre.x.checked_add(delta_x)?,
        y: centre.y.checked_add(delta_y)?,
    })
}

/// Returns the closest point on the segment to the sample.
///
/// The projection parameter is clamped to `[0, 1]`.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn closest_on_segment(
    edge: Vector,
    join: Vector,
    origin: Point,
) -> Result<Point, Overflow> {
    let extent = squared_length(edge)?;
    if extent == int_ratio(0) {
        return Ok(origin);
    }
    let along = dot_product(join, edge)?;
    let fraction = along.checked_div(extent)?;
    let clamped = if fraction < int_ratio(0) {
        int_ratio(0)
    } else if int_ratio(1) < fraction {
        int_ratio(1)
    } else {
        fraction
    };
    shifted_point(
        origin,
        clamped.checked_mul(edge.x)?,
        clamped.checked_mul(edge.y)?,
    )
}

/// Tests whether a pixel centre is inside the disc.
///
/// Uses the exact inequality `(cx - px)^2 + (cy - py)^2 <= r^2` in [`Ratio`]
/// arithmetic.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn disc_covers(centre: Point, radius: Ratio, sample: Point) -> Result<bool, Overflow> {
    if radius <= int_ratio(0) {
        return Ok(false);
    }
    let gap = vector_between(sample, centre)?;
    let dist2 = squared_length(gap)?;
    let bound = radius.checked_mul(radius)?;
    Ok(dist2 <= bound)
}

/// Tests whether a pixel centre is within half a unit of a segment.
///
/// This is the exact rule for [`draw_line`]: a 1-unit-thick line is the set
/// of points whose Euclidean distance to the segment is at most `1 / 2`.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn line_covers(start: Point, tip: Point, sample: Point) -> Result<bool, Overflow> {
    let edge = vector_between(start, tip)?;
    let join = vector_between(start, sample)?;
    let nearest = closest_on_segment(edge, join, start)?;
    let gap = vector_between(nearest, sample)?;
    let dist2 = squared_length(gap)?;
    Ok(dist2 <= quarter_ratio())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_centre_exact_values() {
        // Test that pixel_centre produces exactly (2x + 1) / 2 for x, y coordinates.
        let pt_origin = pixel_centre(0, 0);
        assert_eq!(pt_origin.x.numer(), 1);
        assert_eq!(pt_origin.x.denom().get(), 2);
        assert_eq!(pt_origin.y.numer(), 1);
        assert_eq!(pt_origin.y.denom().get(), 2);

        // Test boundary values: u16::MAX.
        let pt_max = pixel_centre(u16::MAX, u16::MAX);
        // For x = u16::MAX = 65535: (2 * 65535 + 1) / 2 = 131_071 / 2.
        assert_eq!(pt_max.x.numer(), 131_071);
        assert_eq!(pt_max.x.denom().get(), 2);
        assert_eq!(pt_max.y.numer(), 131_071);
        assert_eq!(pt_max.y.denom().get(), 2);
    }
}
