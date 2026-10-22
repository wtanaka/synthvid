//! The error type for catalogue generation.

use core::error::Error;
use core::fmt;

/// Error type for catalog generation.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CatalogError {
    /// A dimension value was zero (rejected by Width or Height constructor).
    ZeroDimension,
    /// A frame rate value was invalid.
    InvalidFrameRate,
    /// A resolved axis index was out of range.
    AxisIndexOutOfRange,
    /// No entry can hold both values of a required pair, or an axis has no
    /// value compatible with the values already chosen. Some compatibility
    /// rule rules out a pair without excluding it from the pairs required.
    UnsatisfiableObligation,
    /// A truncation offset was zero, which would cut nothing.
    ZeroTruncationOffset,
    /// A frame count axis value was zero.
    ZeroFrameCount,
    /// A capture singleton's capture rate equals its playback rate, so it declares nothing.
    CaptureEqualsPlayback,
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDimension => write!(f, "dimension value was zero"),
            Self::InvalidFrameRate => write!(f, "frame rate was invalid"),
            Self::AxisIndexOutOfRange => write!(f, "resolved axis index was out of range"),
            Self::UnsatisfiableObligation => {
                write!(f, "a required pair of axis values cannot be combined")
            }
            Self::ZeroTruncationOffset => write!(f, "truncation offset was zero"),
            Self::ZeroFrameCount => write!(f, "frame count was zero"),
            Self::CaptureEqualsPlayback => {
                write!(f, "capture rate must differ from playback rate")
            }
        }
    }
}

impl Error for CatalogError {}
