//! Flat cut-outs lying on planes at fixed depths.
//!
//! A [`Stage`] is a backdrop and an ordered list of [`Layer`]s. Each layer is a
//! flat [`Cutout`] lying on a plane perpendicular to the camera's optical axis,
//! placed at the depth its [`Track`] gives on every frame. Across is to the
//! right, down is downward, and depth is forward along the optical axis.
//! [`track_at`] evaluates a track in closed form, so a frame never depends on
//! any other frame.

mod coord;
mod track;

#[cfg(test)]
mod tests;

use core::error::Error;
use core::fmt;
use core::num::NonZeroU32;

use self::coord::WORLD_LIMIT;
use crate::color::Rgb8;
use crate::scene::FrameSpan;

pub use coord::{Coord, CoordError, Position, RawCoord};
pub use track::{
    track_at, HalfAcceleration, PerFrame, PerFrameSquared, Track, TrackError, Velocity,
};

/// Error returned when attempting to construct an [`Extent`] longer than the world bound.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ExtentError {
    /// Extent must not exceed the world bound.
    TooLong,
}

impl fmt::Display for ExtentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong => write!(f, "extent exceeds the world bound"),
        }
    }
}

impl Error for ExtentError {}

/// A length in world units, strictly positive and at most the world bound.
///
/// The field is private, so the only way to build an extent is [`Extent::new`],
/// which refuses a length beyond the bound.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Extent(NonZeroU32);

impl Extent {
    /// Creates an extent from a non-zero length no larger than the world bound.
    ///
    /// # Errors
    ///
    /// Returns `Err(ExtentError::TooLong)` if `length` exceeds `1_048_576`.
    pub const fn new(length: NonZeroU32) -> Result<Self, ExtentError> {
        if length.get() <= WORLD_LIMIT {
            Ok(Self(length))
        } else {
            Err(ExtentError::TooLong)
        }
    }

    /// Returns the length in world units.
    #[must_use]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }
}

/// A vertex offset from a layer's position, in the layer's own plane.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct LocalPoint {
    /// Offset to the right of the layer's position.
    pub across: Coord,
    /// Offset below the layer's position.
    pub down: Coord,
}

/// A closed polygon outline with at least three vertices.
///
/// The first three vertices are stored apart from the rest, so an outline with
/// fewer than three cannot be built. Vertices keep their declaration order.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Outline {
    /// The first three vertices, in order.
    first: [LocalPoint; 3],
    /// Every vertex after the first three, in order.
    rest: Vec<LocalPoint>,
}

impl Outline {
    /// Creates an outline from its first three vertices and the vertices that follow them.
    #[must_use]
    pub const fn new(first: [LocalPoint; 3], rest: Vec<LocalPoint>) -> Self {
        Self { first, rest }
    }

    /// Returns every vertex in declaration order: the first three, then the rest.
    pub fn vertices(&self) -> impl Iterator<Item = &LocalPoint> + '_ {
        self.first.iter().chain(self.rest.iter())
    }

    /// Returns the first three vertices as an array, and the vertices after them as a slice.
    ///
    /// The array's length is the type's, so the head always holds exactly three vertices.
    #[must_use]
    pub const fn head_and_tail(&self) -> (&[LocalPoint; 3], &[LocalPoint]) {
        (&self.first, self.rest.as_slice())
    }
}

/// The flat shape of a layer, in the layer's own plane.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Cutout {
    /// Disc of the given radius about the layer's position.
    Disc {
        /// Radius in world units.
        radius: Extent,
    },
    /// Axis-aligned rectangle about the layer's position.
    Rect {
        /// Half the width in world units.
        half_width: Extent,
        /// Half the height in world units.
        half_height: Extent,
    },
    /// Closed polygon through the outline's vertices, decided by the even-odd rule.
    Polygon(Outline),
}

/// A flat cut-out drawn at its track's position on every frame it is visible.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Layer {
    /// The flat shape of the layer.
    pub cutout: Cutout,
    /// Fill color of the layer.
    pub fill: Rgb8,
    /// Closed-form position of the layer as a function of frame.
    pub track: Track,
    /// Frames during which the layer is drawn.
    pub visible: FrameSpan,
}

/// A backdrop and the layers stacked over it in declaration order.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Stage {
    /// Color drawn before any layer.
    pub backdrop: Rgb8,
    /// Layers drawn in declaration order.
    pub layers: Vec<Layer>,
}
