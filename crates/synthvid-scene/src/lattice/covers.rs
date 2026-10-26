//! Containment tests on the lattice.
//!
//! The rules are those of the exact coverage predicates in [`crate::raster`],
//! evaluated on integer lattice points. Each comparison that the rasterizer
//! makes on exact rationals is made here on the sign of a 128-bit cross product.

use super::error::LatticeError;
use super::point::LatticePoint;
use super::shape::{DiscRadius, LatticePolygon, LatticeShape};
use super::wide::Wide;

/// Whether an exact test places a lattice point inside a shape or outside it.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Containment {
    /// The point lies inside, or on a boundary that counts as inside.
    Inside,
    /// The point lies outside.
    Outside,
}

/// The parity of a ray-crossing count.
///
/// Only parity decides point-in-polygon, so the count itself is never kept.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Parity {
    /// An even number of crossings: outside.
    Even,
    /// An odd number of crossings: inside.
    Odd,
}

/// A directed edge of a polygon, from `start` to `end`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct LatticeSegment {
    /// The first endpoint.
    start: LatticePoint,
    /// The second endpoint.
    end: LatticePoint,
}

/// Decides whether a lattice point lies inside a shape.
///
/// A disc contains the points whose squared distance from its center is at
/// most the squared radius. A disc of radius zero contains nothing, as in the
/// rasterizer. A polygon contains the points that lie on its boundary, and
/// otherwise the points that a ray in the positive across direction crosses an
/// odd number of times.
///
/// # Errors
///
/// Returns `Err(LatticeError::Overflow)` if an exact intermediate value leaves
/// the `i128` range.
pub fn covers(shape: &LatticeShape, point: LatticePoint) -> Result<Containment, LatticeError> {
    match shape {
        LatticeShape::Disc { center, radius } => disc_covers(*center, *radius, point),
        LatticeShape::Polygon(polygon) => polygon_covers(polygon, point),
    }
}

/// Tests whether `point` lies in the closed disc around `center`.
///
/// # Errors
///
/// Returns `Err(LatticeError::Overflow)` if an intermediate value overflows.
fn disc_covers(
    center: LatticePoint,
    radius: DiscRadius,
    point: LatticePoint,
) -> Result<Containment, LatticeError> {
    let reach = Wide::of_radius(radius);
    if reach == Wide::zero() {
        return Ok(Containment::Outside);
    }
    let gap = point.displacement_to(center)?;
    let across_squared = gap.across.product(gap.across)?;
    let down_squared = gap.down.product(gap.down)?;
    let distance_squared = across_squared.sum(down_squared)?;
    let bound = reach.product(reach)?;
    if distance_squared <= bound {
        Ok(Containment::Inside)
    } else {
        Ok(Containment::Outside)
    }
}

/// Tests whether `point` lies in the polygon.
///
/// A point on any edge is inside. Otherwise the even-odd rule decides.
///
/// # Errors
///
/// Returns `Err(LatticeError::Overflow)` if an intermediate value overflows.
fn polygon_covers(
    polygon: &LatticePolygon,
    point: LatticePoint,
) -> Result<Containment, LatticeError> {
    let segments = polygon_edges(polygon);
    for segment in segments.iter().copied() {
        if point_on_segment(point, segment)? == Containment::Inside {
            return Ok(Containment::Inside);
        }
    }
    let mut parity = Parity::Even;
    for segment in segments {
        parity = edge_crossing(point, segment, parity)?;
    }
    match parity {
        Parity::Odd => Ok(Containment::Inside),
        Parity::Even => Ok(Containment::Outside),
    }
}

/// Returns the directed edges of the polygon, each vertex joined to the next and
/// the last joined back to the first.
fn polygon_edges(polygon: &LatticePolygon) -> Vec<LatticeSegment> {
    let ring: Vec<LatticePoint> = polygon.vertices().collect();
    let following = ring.iter().copied().cycle().skip(1);
    ring.iter()
        .copied()
        .zip(following)
        .map(|(start, end)| LatticeSegment { start, end })
        .collect()
}

/// Tests whether `point` lies on the closed segment.
///
/// Collinearity is the zero cross product of the segment with the offset to
/// the point. Containment is the inclusive bounding box of the two endpoints.
///
/// # Errors
///
/// Returns `Err(LatticeError::Overflow)` if an intermediate value overflows.
fn point_on_segment(
    point: LatticePoint,
    segment: LatticeSegment,
) -> Result<Containment, LatticeError> {
    let edge = segment.start.displacement_to(segment.end)?;
    let offset = segment.start.displacement_to(point)?;
    if edge.cross(offset)? != Wide::zero() {
        return Ok(Containment::Outside);
    }
    let across_low = segment.start.across.min(segment.end.across);
    let across_high = segment.start.across.max(segment.end.across);
    let down_low = segment.start.down.min(segment.end.down);
    let down_high = segment.start.down.max(segment.end.down);
    if point.across < across_low
        || across_high < point.across
        || point.down < down_low
        || down_high < point.down
    {
        Ok(Containment::Outside)
    } else {
        Ok(Containment::Inside)
    }
}

/// Applies the half-open crossing rule to one edge and returns the updated parity.
///
/// The edge counts when one endpoint is at or below the point's down
/// coordinate and the other is strictly above it. A counted edge flips the
/// parity when it crosses the ray to the right of the point. The crossing lies
/// at `x = start.across + (point.down - start.down) * edge.across / edge.down`,
/// so the test is that `cross / edge.down` is positive, where `cross` is the
/// cross product of the edge with the offset to the point.
///
/// # Errors
///
/// Returns `Err(LatticeError::Overflow)` if an intermediate value overflows.
fn edge_crossing(
    point: LatticePoint,
    segment: LatticeSegment,
    parity: Parity,
) -> Result<Parity, LatticeError> {
    let start_below = segment.start.down <= point.down;
    let end_below = segment.end.down <= point.down;
    if start_below == end_below {
        return Ok(parity);
    }
    let edge = segment.start.displacement_to(segment.end)?;
    let offset = segment.start.displacement_to(point)?;
    let turn = edge.cross(offset)?;
    let crosses_right = if edge.down > Wide::zero() {
        turn > Wide::zero()
    } else {
        turn < Wide::zero()
    };
    if crosses_right {
        Ok(match parity {
            Parity::Even => Parity::Odd,
            Parity::Odd => Parity::Even,
        })
    } else {
        Ok(parity)
    }
}
