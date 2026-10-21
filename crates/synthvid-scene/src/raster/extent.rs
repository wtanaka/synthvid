//! Extent boxes for the rasterizers.
//!
//! Each drawing entry point in the parent module is paired here with a pure
//! function returning the exact rational box it would cover, without touching
//! a [`Frame`](crate::Frame). The manifest reports the box the renderer drew;
//! were it to compute its own, the repository would hold two implementations
//! of one piece of geometry and the ground truth could drift from the pixels
//! quietly. A drawing function returning `()` cannot be asked what it did;
//! these can.

use core::error::Error;
use core::fmt;

use super::{Corners, Cross, Segment};
use crate::geom::Point;
use crate::ratio::{half_ratio, Numerator, Overflow, Ratio};

/// Error returned when attempting to construct an invalid [`Bounds`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BoundsError {
    /// Bounds must have maximum coordinates at or after minimum coordinates.
    Inverted,
}

impl fmt::Display for BoundsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inverted => {
                write!(
                    f,
                    "maximum coordinates must be at or after minimum coordinates"
                )
            }
        }
    }
}

impl Error for BoundsError {}

/// A bounding box or an indication that nothing was drawn.
///
/// An empty result and an arithmetic failure are different outcomes and must
/// not share a representation. A shape with invalid parameters (non-positive
/// radius, non-positive thickness, inverted box, fewer than three vertices)
/// draws nothing and that is the correct answer. An extent that overflows exact
/// arithmetic is a failure the caller must hear about. Collapsing both into
/// `None` is what let an overflow be silently rendered as a missing shape.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BoundsOrEmpty {
    /// Nothing is drawn; this is the correct outcome.
    Empty,
    /// The bounding box that was computed.
    Covering(Bounds),
}

/// An exact rational bounding box.
///
/// The box is the inclusive set `{ p : min_x <= p.x <= max_x,
/// min_y <= p.y <= max_y }`, matching the inclusive edge rule of the
/// rasterizers. [`Bounds::new`] rejects an empty box where either maximum
/// lies strictly below its minimum; [`Bounds::intersect`] and
/// [`Bounds::area`] treat such a box as empty as well, so a literal built by
/// hand can never report a negative area.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Bounds {
    /// Smallest `x` in the box, in scene units.
    pub min_x: Ratio,
    /// Smallest `y` in the box, in scene units.
    pub min_y: Ratio,
    /// Largest `x` in the box, in scene units.
    pub max_x: Ratio,
    /// Largest `y` in the box, in scene units.
    pub max_y: Ratio,
}

impl Bounds {
    /// Creates a bounding box, rejecting an empty one.
    ///
    /// # Errors
    ///
    /// Returns `Err(BoundsError::Inverted)` when the `max` corner lies strictly
    /// below the `min` corner on either axis.
    pub fn new(corners: Corners) -> Result<Self, BoundsError> {
        let Corners {
            min: Point {
                across: min_x,
                down: min_y,
            },
            max: Point {
                across: max_x,
                down: max_y,
            },
        } = corners;
        if max_x < min_x {
            return Err(BoundsError::Inverted);
        }
        if max_y < min_y {
            return Err(BoundsError::Inverted);
        }
        Ok(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    /// Returns the overlap of two boxes.
    ///
    /// # Errors
    ///
    /// Returns `Err(BoundsError::Inverted)` when the boxes are disjoint. Touching at an edge
    /// or a corner still overlaps: the result is the degenerate box along that edge or point.
    pub fn intersect(self, other: Self) -> Result<Self, BoundsError> {
        let min_x = if self.min_x < other.min_x {
            other.min_x
        } else {
            self.min_x
        };
        let min_y = if self.min_y < other.min_y {
            other.min_y
        } else {
            self.min_y
        };
        let max_x = if self.max_x < other.max_x {
            self.max_x
        } else {
            other.max_x
        };
        let max_y = if self.max_y < other.max_y {
            self.max_y
        } else {
            other.max_y
        };
        Self::new(Corners {
            min: Point {
                across: min_x,
                down: min_y,
            },
            max: Point {
                across: max_x,
                down: max_y,
            },
        })
    }

    /// Returns the geometric area of the box.
    ///
    /// Computes `(max_x - min_x) * (max_y - min_y)` exactly.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` when any intermediate or final value overflows.
    /// An inverted box is treated as empty and returns `Ok(Ratio::zero())`.
    pub fn area(self) -> Result<Ratio, Overflow> {
        if self.max_x < self.min_x {
            return Ok(Ratio::from_integer(Numerator(0)));
        }
        if self.max_y < self.min_y {
            return Ok(Ratio::from_integer(Numerator(0)));
        }
        let width = self.max_x.checked_sub(self.min_x)?;
        let height = self.max_y.checked_sub(self.min_y)?;
        width.checked_mul(height)
    }
}

/// Returns the box covered by [`fill_disc`](super::fill_disc).
///
/// The disc is `{ p : |p - centre|^2 <= radius^2 }`, whose tight box is
/// `[centre - radius, centre + radius]` on each axis.
///
/// # Errors
///
/// Returns `Err(Overflow)` only when the conversion overflows exact arithmetic.
/// A non-positive radius yields [`BoundsOrEmpty::Empty`].
pub fn disc_extent(centre: Point, radius: Ratio) -> Result<BoundsOrEmpty, Overflow> {
    if radius <= Ratio::from_integer(Numerator(0)) {
        return Ok(BoundsOrEmpty::Empty);
    }
    let min_x = centre.across.checked_sub(radius)?;
    let max_x = centre.across.checked_add(radius)?;
    let min_y = centre.down.checked_sub(radius)?;
    let max_y = centre.down.checked_add(radius)?;
    match Bounds::new(Corners {
        min: Point {
            across: min_x,
            down: min_y,
        },
        max: Point {
            across: max_x,
            down: max_y,
        },
    }) {
        Ok(bounds) => Ok(BoundsOrEmpty::Covering(bounds)),
        Err(BoundsError::Inverted) => Ok(BoundsOrEmpty::Empty),
    }
}

/// Returns the box covered by [`fill_polygon`](super::fill_polygon).
///
/// The box is the minimum and maximum vertex coordinates on each axis, which
/// is tight for the inclusive edge rule: every covered center lies on an
/// edge or strictly inside, hence within the vertex ranges.
///
/// # Errors
///
/// Returns `Err(Overflow)` only when the conversion overflows exact arithmetic.
/// Fewer than three vertices yields [`BoundsOrEmpty::Empty`].
pub fn polygon_extent(vertices: &[Point]) -> Result<BoundsOrEmpty, Overflow> {
    if vertices.len() < 3 {
        return Ok(BoundsOrEmpty::Empty);
    }
    let mut iter = vertices.iter();
    let first = iter.next().ok_or(Overflow)?;
    let mut min_x = first.across;
    let mut max_x = first.across;
    let mut min_y = first.down;
    let mut max_y = first.down;
    for vertex in iter {
        if vertex.across < min_x {
            min_x = vertex.across;
        }
        if max_x < vertex.across {
            max_x = vertex.across;
        }
        if vertex.down < min_y {
            min_y = vertex.down;
        }
        if max_y < vertex.down {
            max_y = vertex.down;
        }
    }
    match Bounds::new(Corners {
        min: Point {
            across: min_x,
            down: min_y,
        },
        max: Point {
            across: max_x,
            down: max_y,
        },
    }) {
        Ok(bounds) => Ok(BoundsOrEmpty::Covering(bounds)),
        Err(BoundsError::Inverted) => Ok(BoundsOrEmpty::Empty),
    }
}

/// Returns the box covered by [`fill_rect`](super::fill_rect).
///
/// The rectangle is `{ p : min.x <= p.x <= max.x, min.y <= p.y <= max.y }`,
/// so its own corners are already the box.
///
/// When `max` lies strictly below `min` on either axis, yields
/// [`BoundsOrEmpty::Empty`].
#[must_use]
pub fn rect_extent(rect: Corners) -> BoundsOrEmpty {
    match Bounds::new(rect) {
        Ok(bounds) => BoundsOrEmpty::Covering(bounds),
        Err(BoundsError::Inverted) => BoundsOrEmpty::Empty,
    }
}

/// Returns the box covered by [`draw_line`](super::draw_line).
///
/// A 1-unit-thick line is the set of points within `1 / 2` of the segment,
/// so the box is the segment range expanded by half a unit on every side.
/// This also covers a zero-length segment, which draws the disc of radius
/// `1 / 2` around the point.
///
/// # Errors
///
/// Returns `Err(Overflow)` when any intermediate or final value overflows.
pub fn line_extent(segment: Segment) -> Result<BoundsOrEmpty, Overflow> {
    let Segment { start, end } = segment;
    let half = half_ratio();
    let (mut min_x, mut max_x) = if start.across < end.across {
        (start.across, end.across)
    } else {
        (end.across, start.across)
    };
    let (mut min_y, mut max_y) = if start.down < end.down {
        (start.down, end.down)
    } else {
        (end.down, start.down)
    };
    min_x = min_x.checked_sub(half)?;
    max_x = max_x.checked_add(half)?;
    min_y = min_y.checked_sub(half)?;
    max_y = max_y.checked_add(half)?;
    match Bounds::new(Corners {
        min: Point {
            across: min_x,
            down: min_y,
        },
        max: Point {
            across: max_x,
            down: max_y,
        },
    }) {
        Ok(bounds) => Ok(BoundsOrEmpty::Covering(bounds)),
        Err(BoundsError::Inverted) => Ok(BoundsOrEmpty::Empty),
    }
}

/// Returns the box covered by [`draw_cross`](super::draw_cross).
///
/// The cross is the union of its horizontal and vertical bars, so the box is
/// `[centre - reach, centre + reach]` on each axis where `reach` is the
/// larger of `arm` and `thickness / 2`.
///
/// # Errors
///
/// Returns `Err(Overflow)` only when the conversion overflows exact arithmetic.
/// A negative `arm` or a non-positive `thickness` yields [`BoundsOrEmpty::Empty`].
pub fn cross_extent(cross: Cross) -> Result<BoundsOrEmpty, Overflow> {
    let Cross {
        centre,
        arm,
        thickness,
    } = cross;
    if arm < Ratio::from_integer(Numerator(0)) {
        return Ok(BoundsOrEmpty::Empty);
    }
    if thickness <= Ratio::from_integer(Numerator(0)) {
        return Ok(BoundsOrEmpty::Empty);
    }
    let half = thickness.checked_div(Ratio::from_integer(Numerator(2)))?;
    let reach = if half < arm { arm } else { half };
    let min_x = centre.across.checked_sub(reach)?;
    let max_x = centre.across.checked_add(reach)?;
    let min_y = centre.down.checked_sub(reach)?;
    let max_y = centre.down.checked_add(reach)?;
    match Bounds::new(Corners {
        min: Point {
            across: min_x,
            down: min_y,
        },
        max: Point {
            across: max_x,
            down: max_y,
        },
    }) {
        Ok(bounds) => Ok(BoundsOrEmpty::Covering(bounds)),
        Err(BoundsError::Inverted) => Ok(BoundsOrEmpty::Empty),
    }
}

#[cfg(test)]
mod tests;
