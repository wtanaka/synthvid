//! Typed inputs and answers for the coverage predicates.
//!
//! Each struct bundles parameters that would otherwise sit side by side with
//! the same type, so the compiler, not the argument order, tells them apart.
//! Each enum replaces a bare `bool` or count that a caller could read either
//! way round.

use core::ops::RangeInclusive;

use crate::geom::Vector;
use crate::ratio::{Overflow, Ratio};

/// A pixel column index, as the coverage predicates address it.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct Column(pub(super) u16);

/// A pixel row index, as the coverage predicates address it.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct Row(pub(super) u16);

/// Whether an exact test places a point inside a shape or outside it.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum Inclusion {
    /// The point lies inside, or on the boundary the rule counts as inside.
    Inside,
    /// The point lies outside.
    Outside,
}

/// The parity of a ray-crossing count.
///
/// Only parity decides point-in-polygon, so the count itself is never kept.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum Parity {
    /// An even number of crossings: outside.
    Even,
    /// An odd number of crossings: inside.
    Odd,
}

impl Parity {
    /// Returns the parity after one more crossing.
    pub(super) const fn flip(self) -> Self {
        match self {
            Self::Even => Self::Odd,
            Self::Odd => Self::Even,
        }
    }
}

pub(super) use super::args::Segment;

impl Segment {
    /// Returns the displacement from `start` to `end`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` on overflow.
    pub(super) fn displacement(self) -> Result<Vector, Overflow> {
        Ok(Vector {
            across: self.end.across.checked_sub(self.start.across)?,
            down: self.end.down.checked_sub(self.start.down)?,
        })
    }
}

/// Two displacements whose dot product is taken, in a fixed order.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct VectorPair {
    /// The left operand.
    pub(super) first: Vector,
    /// The right operand.
    pub(super) second: Vector,
}

/// A closed interval of scene coordinates on one axis, `lo` to `hi`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct Interval {
    /// The lower bound, inclusive.
    pub(super) lo: Ratio,
    /// The upper bound, inclusive.
    pub(super) hi: Ratio,
}

/// A centre on one axis together with the radius reaching out from it.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct Reach {
    /// The coordinate of the centre.
    pub(super) centre: Ratio,
    /// The distance from the centre to either edge.
    pub(super) radius: Ratio,
}

/// An inclusive run of pixel indices, never inverted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PixelRun(pub(super) RangeInclusive<u16>);
