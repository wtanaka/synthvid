//! Exact coverage predicates and clipping used by the rasterisers.
//!
//! Nothing here touches a [`Frame`](crate::Frame). These are the pure
//! geometric questions a rasteriser asks -- which pixel indices can a shape
//! possibly touch, and does this exact pixel centre lie inside that shape --
//! kept separate so the drawing entry points in the parent module stay short
//! enough to read.

use core::num::NonZeroI64;

use crate::geom::{Point, Vector};
use crate::ratio::{ceil_ratio, floor_ratio, half_ratio, int_ratio, quarter_ratio, Ratio};

/// Converts an inclusive scene-coordinate interval into clipped pixel indices.
///
/// Given that valid pixel centres lie in `[lo, hi]` in scene units, computes
/// the inclusive pixel index range whose centres fall in that interval,
/// clipped to `[0, limit)`. Pixel `i` has centre `i + 1 / 2`, so the raw
/// range is `[ceil(lo - 1 / 2), floor(hi - 1 / 2)]`. Returns `None` when the
/// interval is empty, overflows, or lies entirely outside the frame. This is
/// the explicit clipping step: callers iterate only over the returned range
/// and never rely on per-pixel bounds checks to discard out-of-frame work.
pub(super) fn clipped_pixel_range(lo: Ratio, hi: Ratio, limit: u16) -> Option<(u16, u16)> {
    if hi < lo {
        return None;
    }
    let alpha = ceil_ratio(lo.checked_sub(half_ratio())?)?;
    let omega = floor_ratio(hi.checked_sub(half_ratio())?)?;
    if omega < alpha {
        return None;
    }
    let top = i64::from(limit).checked_sub(1)?;
    let begin = if alpha < 0 { 0 } else { alpha };
    let finish = if omega > top { top } else { omega };
    if finish < begin {
        return None;
    }
    Some((u16::try_from(begin).ok()?, u16::try_from(finish).ok()?))
}

/// Returns the exact centre of the pixel with the given indices.
///
/// The centre is `(x + 1 / 2, y + 1 / 2)` as [`Ratio`]s. Returns `None`
/// only on overflow, which cannot occur for `u16` indices.
pub(super) fn pixel_centre(x: u16, y: u16) -> Option<Point> {
    let denom = NonZeroI64::new(2).unwrap_or(NonZeroI64::MIN);
    let fallback = int_ratio(0);
    let centre_x =
        Ratio::new(i64::from(x).checked_mul(2)?.checked_add(1)?, denom).unwrap_or(fallback);
    let centre_y =
        Ratio::new(i64::from(y).checked_mul(2)?.checked_add(1)?, denom).unwrap_or(fallback);
    Some(Point {
        x: centre_x,
        y: centre_y,
    })
}

/// Returns the exact cross product for the segment test.
///
/// Computes `(b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)`.
/// Returns `None` on overflow.
pub(super) fn segment_cross(p: Point, a: Point, b: Point) -> Option<Ratio> {
    b.x.checked_sub(a.x)?
        .checked_mul(p.y.checked_sub(a.y)?)?
        .checked_sub(b.y.checked_sub(a.y)?.checked_mul(p.x.checked_sub(a.x)?)?)
}

/// Tests whether `p` lies exactly on the closed segment `a` to `b`.
///
/// Collinearity comes from [`segment_cross`] and containment from exact
/// comparisons. Overflow yields `false` deterministically.
pub(super) fn point_on_segment(p: Point, a: Point, b: Point) -> bool {
    let Some(cross) = segment_cross(p, a, b) else {
        return false;
    };
    if cross != int_ratio(0) {
        return false;
    }
    if p.x < a.x && p.x < b.x {
        return false;
    }
    if a.x < p.x && b.x < p.x {
        return false;
    }
    if p.y < a.y && p.y < b.y {
        return false;
    }
    if a.y < p.y && b.y < p.y {
        return false;
    }
    true
}

/// Tests whether `p` lies on any edge of the polygon.
pub(super) fn point_on_polygon_edge(p: Point, vertices: &[Point]) -> bool {
    let mut iter = vertices.iter();
    let Some(first) = iter.next() else {
        return false;
    };
    let mut prev = *first;
    for current in iter {
        if point_on_segment(p, prev, *current) {
            return true;
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
pub(super) fn ray_crossings(p: Point, vertices: &[Point]) -> Option<usize> {
    let mut iter = vertices.iter();
    let Some(first) = iter.next() else {
        return Some(0);
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
/// Returns the updated count, or `None` only if the counter itself would
/// overflow `usize` (unreachable for real polygons).
pub(super) fn edge_crossing(p: Point, a: Point, b: Point, count: usize) -> Option<usize> {
    let a_below = a.y <= p.y;
    let b_below = b.y <= p.y;
    if a_below == b_below {
        return Some(count);
    }
    let num = p.y.checked_sub(a.y)?;
    let den = b.y.checked_sub(a.y)?;
    let t = num.checked_div(den)?;
    let dx = b.x.checked_sub(a.x)?;
    let shift = t.checked_mul(dx)?;
    let x_int = a.x.checked_add(shift)?;
    if x_int <= p.x {
        return Some(count);
    }
    count.checked_add(1)
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
pub(super) fn point_in_polygon(p: Point, vertices: &[Point]) -> bool {
    if vertices.len() < 3 {
        return false;
    }
    if point_on_polygon_edge(p, vertices) {
        return true;
    }
    let Some(crossings) = ray_crossings(p, vertices) else {
        return false;
    };
    let Some(rem) = crossings.checked_rem(2) else {
        return false;
    };
    rem == 1
}

/// Returns the displacement from `start` to `tip`.
///
/// Returns `None` on overflow.
pub(super) fn vector_between(start: Point, tip: Point) -> Option<Vector> {
    Some(Vector {
        x: tip.x.checked_sub(start.x)?,
        y: tip.y.checked_sub(start.y)?,
    })
}

/// Returns the squared Euclidean length of a displacement.
///
/// Computes `x * x + y * y` exactly. Returns `None` on overflow.
pub(super) fn squared_length(delta: Vector) -> Option<Ratio> {
    delta
        .x
        .checked_mul(delta.x)?
        .checked_add(delta.y.checked_mul(delta.y)?)
}

/// Returns the exact dot product of two displacements.
///
/// Returns `None` on overflow.
pub(super) fn dot_product(first: Vector, second: Vector) -> Option<Ratio> {
    first
        .x
        .checked_mul(second.x)?
        .checked_add(first.y.checked_mul(second.y)?)
}

/// Returns the inclusive interval centred on a coordinate.
///
/// Computes `(centre - radius, centre + radius)`. Returns `None` on overflow.
pub(super) fn axis_range(centre_coord: Ratio, radius: Ratio) -> Option<(Ratio, Ratio)> {
    Some((
        centre_coord.checked_sub(radius)?,
        centre_coord.checked_add(radius)?,
    ))
}

/// Returns the point `centre` shifted by the given offsets.
///
/// Returns `None` on overflow.
pub(super) fn shifted_point(centre: Point, delta_x: Ratio, delta_y: Ratio) -> Option<Point> {
    Some(Point {
        x: centre.x.checked_add(delta_x)?,
        y: centre.y.checked_add(delta_y)?,
    })
}

/// Returns the closest point on the segment to the sample.
///
/// The projection parameter is clamped to `[0, 1]`. Returns `None` on
/// overflow.
pub(super) fn closest_on_segment(edge: Vector, join: Vector, origin: Point) -> Option<Point> {
    let extent = squared_length(edge)?;
    if extent == int_ratio(0) {
        return Some(origin);
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
/// arithmetic. Overflow yields `false` deterministically.
pub(super) fn disc_covers(centre: Point, radius: Ratio, sample: Point) -> bool {
    if radius <= int_ratio(0) {
        return false;
    }
    let Some(gap) = vector_between(sample, centre) else {
        return false;
    };
    let Some(dist2) = squared_length(gap) else {
        return false;
    };
    let Some(bound) = radius.checked_mul(radius) else {
        return false;
    };
    dist2 <= bound
}

/// Tests whether a pixel centre is within half a unit of a segment.
///
/// This is the exact rule for [`draw_line`]: a 1-unit-thick line is the set
/// of points whose Euclidean distance to the segment is at most `1 / 2`.
/// Overflow yields `false` deterministically.
pub(super) fn line_covers(start: Point, tip: Point, sample: Point) -> bool {
    let Some(edge) = vector_between(start, tip) else {
        return false;
    };
    let Some(join) = vector_between(start, sample) else {
        return false;
    };
    let Some(nearest) = closest_on_segment(edge, join, start) else {
        return false;
    };
    let Some(gap) = vector_between(nearest, sample) else {
        return false;
    };
    let Some(dist2) = squared_length(gap) else {
        return false;
    };
    dist2 <= quarter_ratio()
}
