//! The error returned when a manifest cannot be assembled.

use core::error::Error;
use core::fmt;
use core::num::TryFromIntError;
use synthvid_scene::FrameCountError;

/// Error returned when a [`crate::manifest::Manifest`] cannot be assembled from its frames.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ManifestError {
    /// The manifest has no frames, so no frame count can describe it.
    Empty(FrameCountError),
    /// The number of frames does not fit in the `u32` a frame count holds.
    TooMany(TryFromIntError),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty(error) => write!(formatter, "manifest has no frames: {error}"),
            Self::TooMany(error) => {
                write!(formatter, "manifest has too many frames: {error}")
            }
        }
    }
}

impl Error for ManifestError {}

impl From<FrameCountError> for ManifestError {
    fn from(error: FrameCountError) -> Self {
        Self::Empty(error)
    }
}

impl From<TryFromIntError> for ManifestError {
    fn from(error: TryFromIntError) -> Self {
        Self::TooMany(error)
    }
}
