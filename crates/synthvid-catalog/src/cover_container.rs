//! The container of a catalogue entry, with the capture rate only ISO can record.

use super::cover_axes::{AxisArrays, CodingAxis, TrackMatrixAxis};
use super::cover_greedy::AxisSelection;
use super::{CatalogError, ContainerAxis};
use synthvid_video::{CaptureRate, DisplayRotation, Encoding};

/// The container an entry takes from its selected axis value. A selected ISO container
/// starts with no capture rate beyond the playback rate.
pub(super) fn entry_container(
    axes: &AxisArrays,
    sel: &AxisSelection,
) -> Result<EntryContainer, CatalogError> {
    let value = axes
        .containers
        .get(sel.container.0)
        .copied()
        .ok_or(CatalogError::AxisIndexOutOfRange)?;
    Ok(match value {
        ContainerAxis::Iso => EntryContainer::Iso {
            capture: CaptureRate::PlaybackOnly,
        },
        ContainerAxis::Avi => EntryContainer::Avi,
    })
}

/// The container of a catalogue entry.
///
/// Only ISO records a capture rate, so the rate is a field of the ISO variant
/// and an AVI entry has no way to hold one.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum EntryContainer {
    /// ISO, with the capture rate the metadata declares.
    Iso {
        /// Capture rate declared in the metadata, or none beyond the playback rate.
        capture: CaptureRate,
    },
    /// AVI, which records no capture rate.
    Avi,
}

impl EntryContainer {
    /// The container axis value this container takes.
    #[must_use]
    pub(super) const fn axis(self) -> ContainerAxis {
        match self {
            Self::Iso { capture: _capture } => ContainerAxis::Iso,
            Self::Avi => ContainerAxis::Avi,
        }
    }

    /// The capture rate declared for this container. An AVI container declares
    /// none, so it reports the playback-only rate.
    #[must_use]
    pub(super) const fn capture(self) -> CaptureRate {
        match self {
            Self::Iso { capture } => capture,
            Self::Avi => CaptureRate::PlaybackOnly,
        }
    }

    /// The container that takes `axis` in place of this one. An ISO container keeps the
    /// capture rate it had; an AVI container, which records none, takes the axis alone.
    #[cfg(test)]
    pub(super) const fn with_axis(self, axis: ContainerAxis) -> Self {
        match axis {
            ContainerAxis::Iso => Self::Iso {
                capture: self.capture(),
            },
            ContainerAxis::Avi => Self::Avi,
        }
    }
}

impl CodingAxis {
    /// The pixel encoding this coding axis value writes into a video specification.
    pub(super) const fn encoding(self) -> Encoding {
        match self {
            Self::Raw => Encoding::Raw,
            Self::MotionJpeg => Encoding::Mjpeg {
                quality: synthvid_encode::Quality::Q75,
                chroma: synthvid_encode::ChromaSampling::Yuv420,
            },
        }
    }
}

impl TrackMatrixAxis {
    /// The display rotation this track matrix value records in a video specification.
    pub(super) const fn rotation(self) -> DisplayRotation {
        match self {
            Self::Identity => DisplayRotation::Identity,
            Self::Rotate90 => DisplayRotation::Rot90,
            Self::Rotate180 => DisplayRotation::Rot180,
            Self::Rotate270 => DisplayRotation::Rot270,
        }
    }
}
