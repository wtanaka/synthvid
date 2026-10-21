//! Name fragments for catalogue entries.
//!
//! Every axis value renders as the fragment its manifest and media names use.
//! The fragments are `Display` implementations, so a name is assembled by
//! formatting the entry's own values rather than by passing strings around.

use super::cover_axes::{
    BackgroundAxis, CodingAxis, ContainerAxis, FrameCountValue, FrameRateValue, MotionAxis,
    ScaleAxis, ShapeAxis, TrackMatrixAxis,
};
use super::ManifestName;
use core::fmt;
use synthvid_encode::defects::Defect;

impl fmt::Display for FrameCountValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.get(), f)
    }
}

impl fmt::Display for FrameRateValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Thirty => "30fps",
            Self::Sixty => "60fps",
            Self::OneTwenty => "120fps",
            Self::TwoForty => "240fps",
            Self::Ntsc => "ntsc",
            Self::Absurd => "absurd",
        })
    }
}

impl fmt::Display for BackgroundAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Solid => "solid",
            Self::Checker => "checker",
            Self::Gradient => "gradient",
            Self::Blobs => "blobs",
            Self::Grid => "grid",
        })
    }
}

impl fmt::Display for ShapeAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Disc => "disc",
            Self::Rect => "rect",
            Self::Polygon => "polygon",
            Self::Cross => "cross",
        })
    }
}

impl fmt::Display for MotionAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Fixed => "fixed",
            Self::Linear => "linear",
            Self::Ballistic => "ballistic",
            Self::Circular => "circular",
            Self::Oscillating => "oscillating",
            Self::Walk => "walk",
        })
    }
}

impl fmt::Display for CodingAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Raw => "raw",
            Self::MotionJpeg => "mjpeg",
        })
    }
}

impl fmt::Display for ContainerAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Iso => "iso",
            Self::Avi => "avi",
        })
    }
}

impl fmt::Display for TrackMatrixAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Identity => "identity",
            Self::Rotate90 => "rot90",
            Self::Rotate180 => "rot180",
            Self::Rotate270 => "rot270",
        })
    }
}

impl fmt::Display for ScaleAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Declared => "scaled",
            Self::Undeclared => "unscaled",
        })
    }
}

/// The name fragment of an entry's defect: `nodef` when it has none.
#[derive(Debug, Clone, Copy)]
pub(super) struct DefectName<'defect>(
    /// The defect, if the entry has one.
    pub(super) Option<&'defect Defect>,
);

impl fmt::Display for DefectName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(defect) = self.0 else {
            return f.write_str("nodef");
        };
        f.write_str(match defect {
            Defect::TruncateBefore(_) => "trunc-before",
            Defect::TruncateWithin {
                address: _address,
                offset: _offset,
            } => "trunc-within",
            Defect::ZeroLengthPayload(_) => "zero-pay",
            Defect::OversizedBoxLength(_) => "over-len",
            Defect::UndersizedBoxLength(_) => "under-len",
            Defect::OffsetPastEnd(_) => "off-end",
            Defect::DuplicateBox(_) => "dup-box",
            Defect::UnknownBox {
                after: _after,
                tag: _tag,
            } => "unk-box",
            Defect::ZeroFrameRate => "zero-fps",
            Defect::ZeroDimension(_) => "zero-dim",
            Defect::AbsurdFrameRate(_) => "absurd-fps",
            Defect::NonMonotonicTimestamps => "non-mono",
            Defect::DeclaredCountMismatch {
                declared: _declared,
            } => "count-mis",
            Defect::ReorderedBoxes(_, _) => "reorder",
        })
    }
}

/// The media file name of an entry: its manifest name, then the extension its container gives.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaName {
    /// The manifest name the media file shares.
    manifest: ManifestName,
    /// The container, which chooses the extension.
    container: ContainerAxis,
}

impl MediaName {
    /// Pairs a manifest name with the container that picks its extension.
    #[must_use]
    pub(super) const fn new(manifest: ManifestName, container: ContainerAxis) -> Self {
        Self {
            manifest,
            container,
        }
    }
}

impl fmt::Display for MediaName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ext = match self.container {
            ContainerAxis::Iso => "mp4",
            ContainerAxis::Avi => "avi",
        };
        write!(f, "{}-media.{ext}", self.manifest)
    }
}
