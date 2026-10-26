//! What records the light a camera focuses.
//!
//! A [`Recorder`] is the part of a camera that turns focused light into pictures: the [`Medium`]
//! that records the light, the [`Dimensions`] of the output in sensels, and the [`Timing`] of
//! when pictures are taken. Construction cannot fail: every combination of the three parts is a
//! recorder that can be described.

use synthvid_scene::Dimensions;

use crate::camera::Timing;
use crate::medium::Medium;

/// What records the light: the medium, the output size, and when pictures are taken.
///
/// The three parts are separate typed values, so they cannot be passed in each other's place.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Recorder {
    /// What records the light on the image plane.
    medium: Medium,
    /// How many sensels wide and tall the output is.
    dimensions: Dimensions,
    /// When the recorder takes pictures.
    timing: Timing,
}

impl Recorder {
    /// Creates a recorder from its medium, output dimensions, and timing.
    #[must_use]
    pub const fn new(medium: Medium, dimensions: Dimensions, timing: Timing) -> Self {
        Self {
            medium,
            dimensions,
            timing,
        }
    }

    /// Returns what records the light.
    #[must_use]
    pub const fn medium(self) -> Medium {
        self.medium
    }

    /// Returns how many sensels wide and tall the output is.
    #[must_use]
    pub const fn dimensions(self) -> Dimensions {
        self.dimensions
    }

    /// Returns when the recorder takes pictures.
    #[must_use]
    pub const fn timing(self) -> Timing {
        self.timing
    }
}

#[cfg(test)]
mod tests;
