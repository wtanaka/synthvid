//! One observer that turns the world into an image.
//!
//! A [`Camera`] stands at a [`View`], focuses light through its [`Optics`], and hands the focused
//! light to its [`Recorder`], which records it on a [`Medium`] as an image of its [`Dimensions`]
//! in sensels, at its [`Timing`]. Each part is a separate typed value, so the three parts cannot
//! be passed in each other's place. Construction cannot fail: every combination of the parts is a
//! camera that can be described.

use synthvid_scene::{Dimensions, FrameCount, FrameRate, View};

use crate::medium::Medium;
use crate::optics::Optics;
use crate::recorder::Recorder;

/// When a camera takes pictures: how often, and how many.
///
/// The rate and the count are separate typed values, so one cannot be given in place of the other.
/// Construction cannot fail: both terms are already known to be positive.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Timing {
    /// How often the camera takes a picture.
    rate: FrameRate,
    /// How many pictures the camera takes.
    count: FrameCount,
}

impl Timing {
    /// Creates a timing from a frame rate and a frame count.
    #[must_use]
    pub const fn new(rate: FrameRate, count: FrameCount) -> Self {
        Self { rate, count }
    }

    /// Returns how often the camera takes a picture.
    #[must_use]
    pub const fn rate(self) -> FrameRate {
        self.rate
    }

    /// Returns how many pictures the camera takes.
    #[must_use]
    pub const fn count(self) -> FrameCount {
        self.count
    }
}

/// One observer: where it stands and how it is oriented, how it focuses, and what records the
/// focused light.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Camera {
    /// Where the camera stands and how it is oriented.
    view: View,
    /// How the lens focuses light onto the image plane.
    optics: Optics,
    /// What records the light: its medium, output dimensions, and timing.
    recorder: Recorder,
}

impl Camera {
    /// Creates a camera from its view, optics, and recorder. The recorder is copied into the camera.
    #[must_use]
    pub const fn new(view: View, optics: Optics, recorder: &Recorder) -> Self {
        Self {
            view,
            optics,
            recorder: *recorder,
        }
    }

    /// Returns where the camera stands and how it is oriented.
    #[must_use]
    pub const fn view(&self) -> &View {
        &self.view
    }

    /// Returns how the lens focuses light.
    #[must_use]
    pub const fn optics(&self) -> Optics {
        self.optics
    }

    /// Returns what records the light.
    #[must_use]
    pub const fn recorder(&self) -> Recorder {
        self.recorder
    }

    /// Returns what records the light on the image plane.
    #[must_use]
    pub const fn medium(&self) -> Medium {
        self.recorder.medium()
    }

    /// Returns how many sensels wide and tall the output is.
    #[must_use]
    pub const fn dimensions(&self) -> Dimensions {
        self.recorder.dimensions()
    }

    /// Returns when the camera takes pictures.
    #[must_use]
    pub const fn timing(&self) -> Timing {
        self.recorder.timing()
    }
}

#[cfg(test)]
mod tests;
