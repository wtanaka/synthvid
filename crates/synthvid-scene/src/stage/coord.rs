//! World coordinates, each validated against the world bound.

use core::error::Error;
use core::fmt;

/// Largest magnitude a world coordinate or extent may take, in world units.
pub(super) const WORLD_LIMIT: u32 = 0x0010_0000;

/// Error returned when attempting to construct a [`Coord`] beyond the world bound.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CoordError {
    /// Coordinate magnitude must not exceed the world bound.
    OutOfRange,
}

impl fmt::Display for CoordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange => write!(f, "coordinate lies outside the world bound"),
        }
    }
}

impl Error for CoordError {}

/// A coordinate in world units, before it is checked against the world bound.
///
/// Every `i32` is a legal raw value; [`Coord::new`] decides which are in range.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RawCoord(pub i32);

/// A world coordinate whose magnitude is at most the world bound.
///
/// The field is private, so every `Coord` in existence has passed [`Coord::new`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Coord(RawCoord);

impl Coord {
    /// Creates a world coordinate whose magnitude is at most `1_048_576`.
    ///
    /// # Errors
    ///
    /// Returns `Err(CoordError::OutOfRange)` if the magnitude of `raw` exceeds `1_048_576`.
    pub const fn new(raw: RawCoord) -> Result<Self, CoordError> {
        if raw.0.unsigned_abs() <= WORLD_LIMIT {
            Ok(Self(raw))
        } else {
            Err(CoordError::OutOfRange)
        }
    }

    /// Returns the coordinate in world units.
    #[must_use]
    pub const fn get(self) -> RawCoord {
        self.0
    }
}

/// A point in world space: across to the right, down downward, and depth forward.
///
/// Each component is already validated, so a position cannot lie outside the world.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Position {
    /// Distance to the right of the origin.
    pub across: Coord,
    /// Distance below the origin.
    pub down: Coord,
    /// Distance forward along the optical axis.
    pub depth: Coord,
}
