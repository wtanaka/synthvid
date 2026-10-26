//! The failures of preparing a deep shot's layers for a frame.

use core::error::Error;
use core::fmt;

use synthvid_scene::{FrameIndex, LatticeError, ObjectIndex, Overflow, TrackError};

use super::{ApertureError, ProjectionError};

/// Why a deep shot could not be prepared for a frame.
///
/// Each variant names the stage of preparation that failed, and a projection failure names the
/// frame and the layer it concerns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeepRenderError {
    /// The camera or a layer track could not be evaluated at the frame.
    Track(TrackError),
    /// The camera's roll could not be turned into its cosine and sine.
    Roll(ProjectionError),
    /// A layer could not be projected onto the lattice at the frame.
    Projection {
        /// The frame at which the projection failed.
        frame: FrameIndex,
        /// The layer whose projection failed, counted from zero in declaration order.
        layer: ObjectIndex,
        /// The reason the projection failed.
        error: ProjectionError,
    },
    /// The lens aperture could not be sampled. The error is boxed so that the sizes of the
    /// variants stay within the workspace limit on how far apart they may be.
    Aperture(Box<ApertureError>),
    /// Exact lattice arithmetic could not represent a result.
    Lattice(LatticeError),
    /// Exact arithmetic could not represent an intermediate value.
    Overflow,
    /// A sample lies outside the sensels of the frame.
    PixelOutOfBounds,
    /// A frame's dimensions differ from the dimensions of the rig's recorder.
    FrameSizeMismatch,
}

impl fmt::Display for DeepRenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Track(error) => write!(f, "track could not be evaluated: {error}"),
            Self::Roll(error) => write!(f, "camera roll could not be placed: {error}"),
            Self::Projection {
                frame,
                layer,
                error,
            } => write!(
                f,
                "layer {} could not be projected at frame {}: {error}",
                layer.get(),
                frame.get()
            ),
            Self::Aperture(error) => write!(f, "lens aperture could not be sampled: {error}"),
            Self::Lattice(error) => write!(f, "lattice arithmetic failed: {error}"),
            Self::Overflow => write!(f, "deep render arithmetic could not represent a result"),
            Self::PixelOutOfBounds => write!(f, "sample lies outside the sensels of the frame"),
            Self::FrameSizeMismatch => {
                write!(f, "frame dimensions differ from the rig's recorder")
            }
        }
    }
}

impl Error for DeepRenderError {}

impl From<TrackError> for DeepRenderError {
    fn from(error: TrackError) -> Self {
        Self::Track(error)
    }
}

impl From<ApertureError> for DeepRenderError {
    fn from(error: ApertureError) -> Self {
        Self::Aperture(Box::new(error))
    }
}

impl From<LatticeError> for DeepRenderError {
    fn from(error: LatticeError) -> Self {
        Self::Lattice(error)
    }
}

/// Every overflow of preparation arithmetic is the same failure, so the variant alone carries the
/// fact and the overflow itself has nothing further to report.
impl From<Overflow> for DeepRenderError {
    fn from(_: Overflow) -> Self {
        Self::Overflow
    }
}
