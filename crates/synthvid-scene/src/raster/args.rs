//! Typed arguments for the drawing entry points and their extent twins.
//!
//! Each struct bundles parameters that would otherwise sit side by side with
//! the same type, so the compiler, not the argument order, tells them apart.

use crate::geom::Point;
use crate::ratio::Ratio;

/// A directed segment from `start` to `end`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    /// The first endpoint.
    pub start: Point,
    /// The second endpoint.
    pub end: Point,
}

/// An axis-aligned rectangle given by two opposite corners.
///
/// The rectangle is the set `{ p : min.x <= p.x <= max.x, min.y <= p.y <= max.y }`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Corners {
    /// The corner with the smaller coordinates.
    pub min: Point,
    /// The corner with the larger coordinates.
    pub max: Point,
}

/// An axis-aligned cross centred on `centre`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Cross {
    /// The centre of the cross.
    pub centre: Point,
    /// The half-length from the centre to the tip of each bar along its axis.
    pub arm: Ratio,
    /// The full width of each bar.
    pub thickness: Ratio,
}

/// Half the width and half the height of an axis-aligned rectangle.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct HalfSize {
    /// Half the width, in scene units.
    pub width: Ratio,
    /// Half the height, in scene units.
    pub height: Ratio,
}
