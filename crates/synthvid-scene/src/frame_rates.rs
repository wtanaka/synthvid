//! Playback and capture frame rates.
//!
//! A video may be captured at one rate and played back at another. The capture
//! rate is optional, and when it is present it must differ from the playback
//! rate: an equal pair would describe no distinction at all. The representation
//! is private, so the only way to hold a distinct capture rate is through
//! [`FrameRates::distinct_in`], which is the single place that checks equality.

use crate::units::FrameRate;
use core::error::Error;
use core::fmt;

/// Error returned when a capture rate offered alongside a playback rate is
/// equal to it.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FrameRatesError {
    /// The capture rate equals the playback rate, so there is no distinct
    /// capture rate to record.
    CaptureEqualsPlayback,
}

impl fmt::Display for FrameRatesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CaptureEqualsPlayback => {
                write!(f, "capture rate must differ from playback rate")
            }
        }
    }
}

impl Error for FrameRatesError {}

/// Where a distinct capture rate is written in a container. The two forms carry the
/// same rate in different boxes, so a container writes exactly one of them.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum CaptureCarrier {
    /// The project-specific `moov/udta/cfps` box, which holds the exact reduced rate.
    ProjectBox,
    /// The `QuickTime`-style `moov/meta` box, which holds the rate as a 32-bit float.
    QuickTimeMeta,
}

/// A declared capture rate and the box that carries it. Distinctness from playback is
/// checked by [`FrameRates::distinct_in`], which is the only way to obtain a
/// `DistinctCapture` inside a [`FrameRates`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct DistinctCapture {
    /// The rate the media was captured at.
    pub rate: FrameRate,
    /// The box the rate is written in.
    pub carrier: CaptureCarrier,
}

/// Whether a capture rate distinct from playback is present.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum Capture {
    /// No distinct capture rate: the media runs at the playback rate.
    PlaybackOnly,
    /// A capture rate that is known to differ from playback.
    Distinct(DistinctCapture),
}

/// A playback rate and the capture rate offered alongside it.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct RatePair {
    /// The rate the media is played back at.
    pub playback: FrameRate,
    /// The rate the media was captured at.
    pub capture: FrameRate,
}

/// A playback frame rate with an optional, necessarily distinct, capture rate.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct FrameRates {
    /// The playback rate.
    playback: FrameRate,
    /// Whether a distinct capture rate is present.
    capture: Capture,
}

impl FrameRates {
    /// Frame rates with no distinct capture rate.
    #[must_use]
    pub const fn playback_only(playback: FrameRate) -> Self {
        Self {
            playback,
            capture: Capture::PlaybackOnly,
        }
    }

    /// Frame rates from a playback rate and an optional capture part, such as one
    /// returned by [`FrameRates::distinct_capture`].
    ///
    /// A present capture part is passed through [`FrameRates::distinct_in`], so the
    /// equality check is the same one every other constructor uses.
    ///
    /// # Errors
    ///
    /// Returns [`FrameRatesError::CaptureEqualsPlayback`] when the capture rate equals
    /// `playback`.
    pub fn from_playback_and_capture(
        playback: FrameRate,
        capture: Option<DistinctCapture>,
    ) -> Result<Self, FrameRatesError> {
        match capture {
            None => Ok(Self::playback_only(playback)),
            Some(DistinctCapture { rate, carrier }) => Self::distinct_in(
                RatePair {
                    playback,
                    capture: rate,
                },
                carrier,
            ),
        }
    }

    /// Frame rates with a capture rate that differs from playback, carried in the
    /// box `carrier` names.
    ///
    /// # Errors
    ///
    /// Returns [`FrameRatesError::CaptureEqualsPlayback`] when `capture` equals
    /// `playback`.
    pub fn distinct_in(pair: RatePair, carrier: CaptureCarrier) -> Result<Self, FrameRatesError> {
        if pair.playback == pair.capture {
            return Err(FrameRatesError::CaptureEqualsPlayback);
        }
        Ok(Self {
            playback: pair.playback,
            capture: Capture::Distinct(DistinctCapture {
                rate: pair.capture,
                carrier,
            }),
        })
    }

    /// The playback frame rate.
    #[must_use]
    pub const fn playback(self) -> FrameRate {
        self.playback
    }

    /// The distinct capture rate, or `None` when there is none.
    #[must_use]
    pub const fn capture(self) -> Option<FrameRate> {
        match self.capture {
            Capture::PlaybackOnly => None,
            Capture::Distinct(capture) => Some(capture.rate),
        }
    }

    /// The distinct capture rate with the box that carries it, or `None` when there
    /// is none.
    #[must_use]
    pub const fn distinct_capture(self) -> Option<DistinctCapture> {
        match self.capture {
            Capture::PlaybackOnly => None,
            Capture::Distinct(capture) => Some(capture),
        }
    }
}

#[cfg(test)]
mod tests;
