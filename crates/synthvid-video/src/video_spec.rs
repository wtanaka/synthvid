//! Video specification: the parameters that explicitly drive rendering.
//!
//! A `Scene` describes sensible content, and a `Camera` records it. A `VideoSpec` adds the
//! choices that turn them into a file, and its fields have the range of the file format's
//! fields, so it can describe a file that makes no sense. The manifest header's declared
//! values are the camera's computed values. A coded size or sample table that differs from
//! the camera's has no field here, because `Camera` cannot express it; such fields belong on
//! `VideoSpec`, defaulting to the camera's values. Byte-level corruption stays in the defect
//! list.

use crate::camera::Camera;
use crate::json_names::JsonEntryName;
use crate::recorder::Recorder;
use crate::rig::DeepShot;
use crate::shot::{FlatShot, Shot};
use core::error::Error;
use core::fmt;
use synthvid_encode::{ChromaSampling, Defect, Quality};
use synthvid_scene::{DistinctCapture, FrameRate, FrameRates, FrameRatesError, Scene};

/// A display rotation, independent of video content.
///
/// Only an ISO container can carry one, so only [`Container::Iso`] holds it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DisplayRotation {
    /// Identity: no rotation.
    Identity,
    /// 90 degrees clockwise.
    Rot90,
    /// 180 degrees.
    Rot180,
    /// 270 degrees clockwise.
    Rot270,
}

/// Writes the rotation as a phrase.
impl fmt::Display for DisplayRotation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Identity => "identity",
            Self::Rot90 => "90 degrees clockwise",
            Self::Rot180 => "180 degrees",
            Self::Rot270 => "270 degrees clockwise",
        })
    }
}

/// How video frames are encoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Encoding {
    /// Raw uncompressed RGB frames.
    Raw,
    /// Motion JPEG frames.
    Mjpeg {
        /// JPEG quality value in the range 1-100.
        quality: Quality,
        /// Chroma subsampling mode.
        chroma: ChromaSampling,
    },
}

/// The capture rate an ISO container declares beside its playback rate.
///
/// Only the ISO container has one, because AVI cannot carry a capture rate. A distinct
/// rate names the box that carries it, so the rate is held in exactly one form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureRate {
    /// No distinct capture rate: the media runs at the playback rate.
    PlaybackOnly,
    /// Captured at this rate, in the box the carrier names. A specification refuses the
    /// rate when it equals the playback rate.
    Distinct(DistinctCapture),
}

impl CaptureRate {
    /// The distinct capture rate and its carrier, or `None` when the media runs at the
    /// playback rate.
    #[must_use]
    pub const fn distinct(self) -> Option<DistinctCapture> {
        match self {
            Self::PlaybackOnly => None,
            Self::Distinct(capture) => Some(capture),
        }
    }
}

/// File container format for the encoded video.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Container {
    /// ISO base media format (MP4), with the display rotation its track matrix carries and
    /// the capture rate its metadata declares.
    Iso {
        /// The rotation recorded in the track matrix.
        rotation: DisplayRotation,
        /// The capture rate recorded in the metadata.
        capture: CaptureRate,
    },
    /// AVI format, which has no rotation to record.
    Avi,
}

/// How a video is written: its encoding, its container, and an optional defect to apply.
///
/// The parts are private, so a format is built only through [`Format::new`]. Whether a
/// defect can meet the encoding and container is decided when the format is given to
/// [`VideoSpec::new`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Format {
    /// How frames are encoded.
    encoding: Encoding,
    /// The container the encoded frames are written in.
    container: Container,
    /// Optional defect to apply to the media bytes after encoding.
    defect: Option<Defect>,
}

impl Format {
    /// Creates a format from its encoding, container, and optional defect.
    #[must_use]
    pub const fn new(encoding: Encoding, container: Container, defect: Option<Defect>) -> Self {
        Self {
            encoding,
            container,
            defect,
        }
    }

    /// Returns the encoding.
    #[must_use]
    pub const fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Returns the container format.
    #[must_use]
    pub const fn container(&self) -> Container {
        self.container
    }

    /// Returns a reference to the optional defect.
    #[must_use]
    pub const fn defect(&self) -> Option<&Defect> {
        self.defect.as_ref()
    }
}

/// An explicit video specification that drives rendering.
///
/// All parameters needed to render a video: the shot to record, encoding parameters,
/// container format, and any defects to apply.
///
/// The shot is either a flat scene recorded by a camera or a deep shot seen through a rig.
/// The encoder receives exactly the frames the shot's recorder records. Fields for a coded size
/// or sample table differing from the recorder's are absent because the shot cannot express
/// them; they belong on this type, defaulting to the recorder's values. Byte-level corruption
/// stays in the defect list.
#[derive(Clone, Debug)]
pub struct VideoSpec {
    /// Manifest name: free text chosen by the spec builder, not derived from or checked
    /// against the codec or container.
    name: JsonEntryName,
    /// The shot to record: a flat scene and its camera, or a deep shot and its rig.
    shot: Shot,
    /// How frames are encoded.
    encoding: Encoding,
    /// The container's shape, with its validated capture part when it is ISO. The
    /// playback rate is the shot's recorder timing rate and is not stored a second time.
    shape: ContainerShape,
    /// Optional defect to apply to the media bytes after encoding.
    defect: Option<Defect>,
}

/// The container's shape. Only an ISO shape can hold a capture part, so an AVI shape
/// cannot carry one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContainerShape {
    /// ISO, with the rotation its track matrix carries and the distinct capture part
    /// validated when the specification was built.
    Iso {
        /// The rotation recorded in the track matrix.
        rotation: DisplayRotation,
        /// The distinct capture rate and its carrier, or `None` for playback only.
        capture: Option<DistinctCapture>,
    },
    /// AVI, which has no rotation to record and no capture rate.
    Avi,
}

/// Why a combination of encoding, container and defect cannot be a specification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum VideoSpecError {
    /// The defect that leaves the size out of the container needs motion-JPEG frames in an
    /// ISO container: only then does the size survive in the frames for a reader to find.
    SizeOmissionNeedsMjpegInIso,
    /// The playback and capture rates were refused by the camera's frame rates.
    FrameRates(FrameRatesError),
}

impl fmt::Display for VideoSpecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SizeOmissionNeedsMjpegInIso => formatter.write_str(
                "omitting the size from the container needs motion-JPEG frames in an ISO container",
            ),
            Self::FrameRates(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl Error for VideoSpecError {}

/// Refuses a defect that cannot meet this encoding and container.
pub(crate) const fn check_defect(
    encoding: Encoding,
    container: Container,
    defect: Option<&Defect>,
) -> Result<(), VideoSpecError> {
    match defect {
        Some(Defect::NoContainerSize) => check_size_omission(encoding, container),
        Some(Defect::TruncateWithin {
            address: _address,
            offset: _offset,
        }) => Ok(()),
        Some(Defect::UnknownBox {
            after: _after,
            tag: _tag,
        }) => Ok(()),
        Some(Defect::DeclaredCountMismatch {
            declared: _declared,
        }) => Ok(()),
        Some(
            Defect::TruncateBefore(_)
            | Defect::ZeroLengthPayload(_)
            | Defect::OversizedBoxLength(_)
            | Defect::UndersizedBoxLength(_)
            | Defect::OffsetPastEnd(_)
            | Defect::DuplicateBox(_)
            | Defect::ZeroFrameRate
            | Defect::ZeroDimension(_)
            | Defect::AbsurdFrameRate(_)
            | Defect::NonMonotonicTimestamps
            | Defect::ReorderedBoxes(_, _),
        )
        | None => Ok(()),
    }
}

/// Whether the size omitted from the container survives in the frames: only motion-JPEG
/// frames in an ISO container carry it for a reader to find.
const fn check_size_omission(
    encoding: Encoding,
    container: Container,
) -> Result<(), VideoSpecError> {
    match encoding {
        Encoding::Mjpeg {
            quality: _quality,
            chroma: _chroma,
        } => match container {
            Container::Iso {
                rotation: _rotation,
                capture: _capture,
            } => Ok(()),
            Container::Avi => Err(VideoSpecError::SizeOmissionNeedsMjpegInIso),
        },
        Encoding::Raw => Err(VideoSpecError::SizeOmissionNeedsMjpegInIso),
    }
}

/// Splits a container into the shape it is written in, refusing a capture rate equal to
/// the playback rate.
///
/// The equality rule is applied by [`FrameRates::from_playback_and_capture`] here, at
/// construction. [`VideoSpec::frame_rates`] applies it again when the rates are read; that
/// second call cannot fail while the shot is immutable. The playback rate is the shot's.
///
/// # Errors
///
/// Returns [`VideoSpecError::FrameRates`] wrapping [`FrameRatesError::CaptureEqualsPlayback`]
/// when an ISO container's capture rate equals `frame_rate`.
fn split_container(
    frame_rate: FrameRate,
    container: Container,
) -> Result<ContainerShape, VideoSpecError> {
    match container {
        Container::Iso { rotation, capture } => {
            let frame_rates = FrameRates::from_playback_and_capture(frame_rate, capture.distinct())
                .map_err(VideoSpecError::FrameRates)?;
            Ok(ContainerShape::Iso {
                rotation,
                capture: frame_rates.distinct_capture(),
            })
        }
        Container::Avi => Ok(ContainerShape::Avi),
    }
}

/// The capture rate a stored capture part declares, in the form a container carries.
#[must_use]
const fn capture_rate_of(capture: Option<DistinctCapture>) -> CaptureRate {
    match capture {
        Some(distinct) => CaptureRate::Distinct(distinct),
        None => CaptureRate::PlaybackOnly,
    }
}

impl VideoSpec {
    /// Creates a new video specification.
    ///
    /// # Errors
    ///
    /// Returns [`VideoSpecError`] when the defect cannot meet the encoding and container, or
    /// when the capture rate equals the camera's playback rate.
    pub fn new(
        name: JsonEntryName,
        scene: Scene,
        camera: Camera,
        format: Format,
    ) -> Result<Self, VideoSpecError> {
        let shot = Shot::Flat(Box::new(FlatShot::from_camera(scene, camera)));
        Self::from_shot(name, shot, format)
    }

    /// Creates a new video specification for a deep shot seen through its rig.
    ///
    /// The playback rate is the rig's recorder timing rate.
    ///
    /// # Errors
    ///
    /// Returns [`VideoSpecError`] when the defect cannot meet the encoding and container, or
    /// when the capture rate equals the rig's playback rate.
    pub fn deep(
        name: JsonEntryName,
        deep: DeepShot,
        format: Format,
    ) -> Result<Self, VideoSpecError> {
        Self::from_shot(name, Shot::Deep(Box::new(deep)), format)
    }

    /// Creates a specification around a shot, checking the defect and splitting the container
    /// against the shot's playback rate.
    fn from_shot(name: JsonEntryName, shot: Shot, format: Format) -> Result<Self, VideoSpecError> {
        let Format {
            encoding,
            container,
            defect,
        } = format;
        check_defect(encoding, container, defect.as_ref())?;
        let shape = split_container(shot.recorder().timing().rate(), container)?;
        Ok(Self {
            name,
            shot,
            encoding,
            shape,
            defect,
        })
    }

    /// Returns the manifest name.
    #[must_use]
    pub const fn name(&self) -> &JsonEntryName {
        &self.name
    }

    /// Returns the shot to record.
    #[must_use]
    pub const fn shot(&self) -> &Shot {
        &self.shot
    }

    /// Returns what records the shot's light: the camera's recorder for a flat shot, and the
    /// rig's recorder for a deep shot.
    #[must_use]
    pub fn recorder(&self) -> Recorder {
        self.shot.recorder()
    }

    /// Returns a reference to the scene of a flat shot, or `None` for a deep shot.
    #[must_use]
    pub fn scene(&self) -> Option<&Scene> {
        match &self.shot {
            Shot::Flat(flat) => Some(flat.scene()),
            Shot::Deep(_) => None,
        }
    }

    /// Returns a reference to the camera that records a flat shot, or `None` for a deep shot.
    #[must_use]
    pub fn camera(&self) -> Option<&Camera> {
        match &self.shot {
            Shot::Flat(flat) => Some(flat.camera()),
            Shot::Deep(_) => None,
        }
    }

    /// Returns the encoding.
    #[must_use]
    pub const fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Returns the container format.
    #[must_use]
    pub const fn container(&self) -> Container {
        match self.shape {
            ContainerShape::Iso { rotation, capture } => Container::Iso {
                rotation,
                capture: capture_rate_of(capture),
            },
            ContainerShape::Avi => Container::Avi,
        }
    }

    /// Returns the validated playback and capture rates. The playback rate is the
    /// shot's, and the capture part is the one validated at construction.
    ///
    /// # Errors
    ///
    /// Returns [`VideoSpecError::FrameRates`] when the stored capture rate equals the
    /// shot's playback rate.
    pub fn frame_rates(&self) -> Result<FrameRates, VideoSpecError> {
        let capture = match self.shape {
            ContainerShape::Iso {
                rotation: _rotation,
                capture,
            } => capture,
            ContainerShape::Avi => None,
        };
        FrameRates::from_playback_and_capture(self.shot.recorder().timing().rate(), capture)
            .map_err(VideoSpecError::FrameRates)
    }

    /// Returns a reference to the optional defect.
    #[must_use]
    pub const fn defect(&self) -> Option<&Defect> {
        self.defect.as_ref()
    }

    /// Returns this specification with a different container.
    ///
    /// A combination the defect cannot meet is not refused here; [`render()`](fn@crate::render)
    /// refuses it.
    ///
    /// # Errors
    ///
    /// Returns [`VideoSpecError::FrameRates`] when the new container's capture rate equals
    /// the shot's playback rate.
    pub fn with_container(self, container: Container) -> Result<Self, VideoSpecError> {
        let shape = split_container(self.shot.recorder().timing().rate(), container)?;
        Ok(Self {
            name: self.name,
            shot: self.shot,
            encoding: self.encoding,
            shape,
            defect: self.defect,
        })
    }
}

#[cfg(test)]
mod tests;
