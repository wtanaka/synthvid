//! Exact coverage predicates and clipping used by the rasterizers.
//!
//! Nothing here touches a [`Frame`](crate::Frame). These are the pure
//! geometric questions a rasterizer asks -- which pixel indices can a shape
//! possibly touch, and does this exact pixel center lie inside that shape --
//! kept separate so the drawing entry points in the parent module stay short
//! enough to read.

use crate::geom::{Point, Vector};
use crate::ratio::helpers::AxisPixel;
use crate::ratio::{
    ceil_ratio, floor_ratio, half_ratio, pixel_centre_ratio, quarter_ratio, Numerator, Overflow,
    Ratio,
};
use core::num::{NonZeroI64, NonZeroU16};

use super::coverage_types::{
    Column, Inclusion, Interval, Parity, PixelRun, Reach, Row, Segment, VectorPair,
};

/// The pixel indices a shape's extent covers on one axis.
///
/// An empty result and an arithmetic failure are different outcomes and must
/// not share a representation. A shape that lies off screen, or whose extent
/// contains no pixel center, draws nothing and that is the correct answer. An
/// extent that overflows exact arithmetic is a failure the caller must hear
/// about. Collapsing both into `None` is what let an overflow be silently
/// rendered as a missing shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PixelSpan {
    /// No pixel center falls within the extent; nothing is drawn.
    Empty,
    /// The inclusive range of pixel indices to visit, never inverted.
    Covering {
        /// First and last pixel index, both inclusive.
        span: PixelRun,
    },
}

/// Converts an inclusive scene-coordinate interval into clipped pixel indices.
///
/// # Errors
///
/// Returns `Err(Overflow)` only when the conversion overflows exact arithmetic.
/// An interval that is inverted, contains no pixel center, or falls entirely
/// outside `0..limit` yields [`PixelSpan::Empty`].
pub(super) fn clipped_pixel_range(
    interval: Interval,
    limit: NonZeroU16,
) -> Result<PixelSpan, Overflow> {
    let Interval { lo, hi } = interval;
    if hi < lo {
        return Ok(PixelSpan::Empty);
    }
    let alpha = ceil_ratio(lo.checked_sub(half_ratio())?)?;
    let omega = floor_ratio(hi.checked_sub(half_ratio())?)?;
    if omega < alpha {
        return Ok(PixelSpan::Empty);
    }
    let zero = Ratio::from_integer(Numerator(0));
    let top = Ratio::from_integer(Numerator(
        NonZeroI64::from(limit)
            .get()
            .checked_sub(1)
            .ok_or(Overflow)?,
    ));
    let begin = if alpha < zero { zero } else { alpha };
    let finish = if omega > top { top } else { omega };
    if finish < begin {
        return Ok(PixelSpan::Empty);
    }
    // Both ends are whole numbers, ratios over one, so the numerator is the
    // pixel index.
    let first = u16::try_from(begin.numer().0).map_err(Overflow::from)?;
    let last = u16::try_from(finish.numer().0).map_err(Overflow::from)?;
    Ok(PixelSpan::Covering {
        span: PixelRun(first..=last),
    })
}

/// Returns the exact center of the pixel with the given indices.
///
/// The center is `(x + 1 / 2, y + 1 / 2)` as [`Ratio`]s. A `u16` index cannot
/// overflow the numerator, so this is total.
pub(super) fn pixel_centre(column: Column, row: Row) -> Point {
    Point {
        across: pixel_centre_ratio(AxisPixel(column.0)),
        down: pixel_centre_ratio(AxisPixel(row.0)),
    }
}

/// Returns the exact cross product for the segment test.
///
/// Computes
/// `(end.x - start.x) * (sample.y - start.y) - (end.y - start.y) * (sample.x - start.x)`
/// for the segment from `start` to `end`.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn segment_cross(sample: Point, segment: Segment) -> Result<Ratio, Overflow> {
    let start = segment.start;
    let end = segment.end;
    let bx_ax = end.across.checked_sub(start.across)?;
    let py_ay = sample.down.checked_sub(start.down)?;
    let left = bx_ax.checked_mul(py_ay)?;
    let by_ay = end.down.checked_sub(start.down)?;
    let px_ax = sample.across.checked_sub(start.across)?;
    let right = by_ay.checked_mul(px_ax)?;
    left.checked_sub(right)
}

/// Tests whether `sample` lies exactly on the closed segment.
///
/// Collinearity comes from [`segment_cross`] and containment from exact
/// comparisons.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn point_on_segment(sample: Point, segment: Segment) -> Result<Inclusion, Overflow> {
    let start = segment.start;
    let end = segment.end;
    let cross = segment_cross(sample, segment)?;
    if cross != Ratio::from_integer(Numerator(0)) {
        return Ok(Inclusion::Outside);
    }
    if sample.across < start.across && sample.across < end.across {
        return Ok(Inclusion::Outside);
    }
    if start.across < sample.across && end.across < sample.across {
        return Ok(Inclusion::Outside);
    }
    if sample.down < start.down && sample.down < end.down {
        return Ok(Inclusion::Outside);
    }
    if start.down < sample.down && end.down < sample.down {
        return Ok(Inclusion::Outside);
    }
    Ok(Inclusion::Inside)
}

/// Tests whether `sample` lies on any edge of the polygon.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn point_on_polygon_edge(
    sample: Point,
    vertices: &[Point],
) -> Result<Inclusion, Overflow> {
    let mut iter = vertices.iter();
    let Some(first) = iter.next() else {
        return Ok(Inclusion::Outside);
    };
    let mut prev = *first;
    for current in iter {
        let edge = Segment {
            start: prev,
            end: *current,
        };
        if point_on_segment(sample, edge)? == Inclusion::Inside {
            return Ok(Inclusion::Inside);
        }
        prev = *current;
    }
    point_on_segment(
        sample,
        Segment {
            start: prev,
            end: *first,
        },
    )
}

/// Counts ray crossings of the polygon with the horizontal ray from `sample`.
///
/// Uses the half-open rule that an edge counts exactly when one endpoint is
/// at or below `sample.y` and the other is strictly above it. Edges whose
/// intersection overflows are treated as non-crossing. The parity is
/// independent of vertex order and of edge direction.
///
/// # Errors
///
/// Returns `Err(Overflow)` only if exact arithmetic on an edge overflows.
pub(super) fn ray_crossings(sample: Point, vertices: &[Point]) -> Result<Parity, Overflow> {
    let mut iter = vertices.iter();
    let Some(first) = iter.next() else {
        return Ok(Parity::Even);
    };
    let mut prev = *first;
    let mut parity = Parity::Even;
    for current in iter {
        let edge = Segment {
            start: prev,
            end: *current,
        };
        parity = edge_crossing(sample, edge, parity)?;
        prev = *current;
    }
    edge_crossing(
        sample,
        Segment {
            start: prev,
            end: *first,
        },
        parity,
    )
}

/// Flips the parity for the directed edge when it crosses the ray from `sample`.
///
/// Returns the updated parity.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn edge_crossing(
    sample: Point,
    edge: Segment,
    parity: Parity,
) -> Result<Parity, Overflow> {
    let start = edge.start;
    let end = edge.end;
    let a_below = start.down <= sample.down;
    let b_below = end.down <= sample.down;
    if a_below == b_below {
        return Ok(parity);
    }
    let num = sample.down.checked_sub(start.down)?;
    let den = end.down.checked_sub(start.down)?;
    let fraction = num.checked_div(den)?;
    let dx = end.across.checked_sub(start.across)?;
    let shift = fraction.checked_mul(dx)?;
    let x_int = start.across.checked_add(shift)?;
    if x_int <= sample.across {
        return Ok(parity);
    }
    Ok(parity.flip())
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
pub(super) fn point_in_polygon(sample: Point, vertices: &[Point]) -> Result<Inclusion, Overflow> {
    if vertices.len() < 3 {
        return Ok(Inclusion::Outside);
    }
    if point_on_polygon_edge(sample, vertices)? == Inclusion::Inside {
        return Ok(Inclusion::Inside);
    }
    match ray_crossings(sample, vertices)? {
        Parity::Odd => Ok(Inclusion::Inside),
        Parity::Even => Ok(Inclusion::Outside),
    }
}

/// Returns the squared Euclidean length of a displacement.
///
/// Computes `x * x + y * y` exactly.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn squared_length(delta: Vector) -> Result<Ratio, Overflow> {
    let xx = delta.across.checked_mul(delta.across)?;
    let yy = delta.down.checked_mul(delta.down)?;
    xx.checked_add(yy)
}

/// Returns the exact dot product of two displacements.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn dot_product(pair: VectorPair) -> Result<Ratio, Overflow> {
    let xx = pair.first.across.checked_mul(pair.second.across)?;
    let yy = pair.first.down.checked_mul(pair.second.down)?;
    xx.checked_add(yy)
}

/// Returns the inclusive interval centered on a coordinate.
///
/// Computes `(centre - radius, centre + radius)`.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn axis_range(reach: Reach) -> Result<Interval, Overflow> {
    Ok(Interval {
        lo: reach.centre.checked_sub(reach.radius)?,
        hi: reach.centre.checked_add(reach.radius)?,
    })
}

/// Returns the point `centre` shifted by the given displacement.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn shifted_point(centre: Point, delta: Vector) -> Result<Point, Overflow> {
    Ok(Point {
        across: centre.across.checked_add(delta.across)?,
        down: centre.down.checked_add(delta.down)?,
    })
}

/// Returns the closest point on the segment to the sample.
///
/// The projection parameter is clamped to `[0, 1]`.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn closest_on_segment(segment: Segment, sample: Point) -> Result<Point, Overflow> {
    let origin = segment.start;
    let edge = segment.displacement()?;
    let join = Segment {
        start: origin,
        end: sample,
    }
    .displacement()?;
    let extent = squared_length(edge)?;
    if extent == Ratio::from_integer(Numerator(0)) {
        return Ok(origin);
    }
    let along = dot_product(VectorPair {
        first: join,
        second: edge,
    })?;
    let fraction = along.checked_div(extent)?;
    let clamped = if fraction < Ratio::from_integer(Numerator(0)) {
        Ratio::from_integer(Numerator(0))
    } else if Ratio::from_integer(Numerator(1)) < fraction {
        Ratio::from_integer(Numerator(1))
    } else {
        fraction
    };
    shifted_point(
        origin,
        Vector {
            across: clamped.checked_mul(edge.across)?,
            down: clamped.checked_mul(edge.down)?,
        },
    )
}

/// Tests whether a pixel center is inside the disc.
///
/// Uses the exact inequality `(cx - px)^2 + (cy - py)^2 <= r^2` in [`Ratio`]
/// arithmetic.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn disc_covers(
    centre: Point,
    radius: Ratio,
    sample: Point,
) -> Result<Inclusion, Overflow> {
    if radius <= Ratio::from_integer(Numerator(0)) {
        return Ok(Inclusion::Outside);
    }
    let gap = Segment {
        start: sample,
        end: centre,
    }
    .displacement()?;
    let dist2 = squared_length(gap)?;
    let bound = radius.checked_mul(radius)?;
    if dist2 <= bound {
        Ok(Inclusion::Inside)
    } else {
        Ok(Inclusion::Outside)
    }
}

/// Tests whether a pixel center is within half a unit of a segment.
///
/// This is the exact rule for [`draw_line`]: a 1-unit-thick line is the set
/// of points whose Euclidean distance to the segment is at most `1 / 2`.
///
/// # Errors
///
/// Returns `Err(Overflow)` on overflow.
pub(super) fn line_covers(segment: Segment, sample: Point) -> Result<Inclusion, Overflow> {
    let nearest = closest_on_segment(segment, sample)?;
    let gap = Segment {
        start: nearest,
        end: sample,
    }
    .displacement()?;
    let dist2 = squared_length(gap)?;
    if dist2 <= quarter_ratio() {
        Ok(Inclusion::Inside)
    } else {
        Ok(Inclusion::Outside)
    }
}

#[cfg(test)]
mod tests;
