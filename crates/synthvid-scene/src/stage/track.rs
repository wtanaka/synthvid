//! Closed-form tracks that place a layer's position as a function of frame.
//!
//! [`track_at`] evaluates each component as `origin + velocity * frame` plus,
//! for a ballistic track, `half_acceleration * frame * frame`. Every product
//! and sum is exact in `i128`, so a frame never depends on any other frame.

use core::error::Error;
use core::fmt;
use core::num::TryFromIntError;

use super::coord::{Coord, CoordError, Position, RawCoord};
use crate::ratio::track_axis;
use crate::units::FrameIndex;

/// A change of coordinate per frame, in world units.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PerFrame(pub i32);

/// Half the change of per-frame change per frame squared, in world units.
///
/// The term contributes `half_acceleration * frame * frame` to a position, so
/// a value of `1` adds `frame^2` and never `frame^2 / 2`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PerFrameSquared(pub i32);

/// Change of position per frame along each axis.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Velocity {
    /// Change to the right per frame.
    pub across: PerFrame,
    /// Change downward per frame.
    pub down: PerFrame,
    /// Change forward along the optical axis per frame.
    pub depth: PerFrame,
}

/// Half the change of velocity per frame along each axis.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct HalfAcceleration {
    /// Half the change of rightward velocity per frame.
    pub across: PerFrameSquared,
    /// Half the change of downward velocity per frame.
    pub down: PerFrameSquared,
    /// Half the change of forward velocity per frame.
    pub depth: PerFrameSquared,
}

/// Closed-form motion of a layer's position.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Track {
    /// Fixed position, identical on every frame.
    Fixed(Position),
    /// Uniform straight-line motion from an origin along a velocity per frame.
    Linear {
        /// Position at frame zero.
        origin: Position,
        /// Change of position per frame.
        velocity: Velocity,
    },
    /// Constant-acceleration motion from an origin with an initial velocity.
    Ballistic {
        /// Position at frame zero.
        origin: Position,
        /// Change of position per frame at frame zero.
        velocity: Velocity,
        /// Half the change of velocity per frame.
        half_acceleration: HalfAcceleration,
    },
}

/// Error returned when a track cannot be evaluated at a frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TrackError {
    /// A component of the position lies outside the world bound.
    LeftWorld,
}

impl fmt::Display for TrackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LeftWorld => write!(f, "track position lies outside the world bound"),
        }
    }
}

impl Error for TrackError {}

/// A value that does not fit in an `i32` lies beyond the world bound, which is
/// far inside the `i32` range, so the conversion reports [`TrackError::LeftWorld`].
impl From<TryFromIntError> for TrackError {
    fn from(_: TryFromIntError) -> Self {
        Self::LeftWorld
    }
}

/// The acceleration term of a track that has none: exactly zero on every axis.
const NO_ACCELERATION: HalfAcceleration = HalfAcceleration {
    across: PerFrameSquared(0),
    down: PerFrameSquared(0),
    depth: PerFrameSquared(0),
};

/// Evaluates one axis as `origin + velocity * frame + half_acceleration * frame * frame`.
///
/// The exact sum must fit an `i32` and lie within the world bound, or the axis has left the world.
fn axis_at(
    origin: Coord,
    velocity: PerFrame,
    half_acceleration: PerFrameSquared,
    frame: FrameIndex,
) -> Result<Coord, TrackError> {
    let total = track_axis(origin, velocity, half_acceleration, frame);
    let raw = i32::try_from(total.0)?;
    match Coord::new(RawCoord(raw)) {
        Ok(coord) => Ok(coord),
        Err(CoordError::OutOfRange) => Err(TrackError::LeftWorld),
    }
}

/// Evaluates all three axes of a moving track at `frame`.
///
/// Axes are evaluated across, down, then depth, and the first failure is returned.
fn position_with(
    origin: &Position,
    velocity: &Velocity,
    half_acceleration: &HalfAcceleration,
    frame: FrameIndex,
) -> Result<Position, TrackError> {
    Ok(Position {
        across: axis_at(
            origin.across,
            velocity.across,
            half_acceleration.across,
            frame,
        )?,
        down: axis_at(origin.down, velocity.down, half_acceleration.down, frame)?,
        depth: axis_at(origin.depth, velocity.depth, half_acceleration.depth, frame)?,
    })
}

/// Returns the position of `track` at `frame`, in closed form.
///
/// The result depends only on the track and the frame index, so frames may be
/// evaluated in any order and repeated with the same answer.
///
/// # Errors
///
/// Returns `Err(TrackError::LeftWorld)` if any component lies outside the world bound.
pub fn track_at(track: &Track, frame: FrameIndex) -> Result<Position, TrackError> {
    match track {
        Track::Fixed(position) => Ok(*position),
        Track::Linear { origin, velocity } => {
            position_with(origin, velocity, &NO_ACCELERATION, frame)
        }
        Track::Ballistic {
            origin,
            velocity,
            half_acceleration,
        } => position_with(origin, velocity, half_acceleration, frame),
    }
}

#[cfg(test)]
mod tests;
