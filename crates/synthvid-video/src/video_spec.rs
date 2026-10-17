//! Video specification: the parameters that explicitly drive rendering.
//!
//! A `Scene` describes sensible content. A `VideoSpec` adds the choices that turn a scene
//! into a file, and its fields have the range of the file format's fields, so it can
//! describe a file that makes no sense. The manifest header's declared values are the
//! scene's computed values. A coded size or sample table that differs from the scene's has
//! no field here, because `Scene` cannot express it; such fields belong on `VideoSpec`,
//! defaulting to the scene's values. Byte-level corruption stays in the defect list.

use crate::json_names::JsonEntryName;
use core::error::Error;
use core::fmt;
use synthvid_encode::{ChromaSampling, Defect, Quality};
use synthvid_scene::Scene;

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

/// File container format for the encoded video.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Container {
    /// ISO base media format (MP4), with the display rotation its track matrix carries.
    Iso {
        /// The rotation recorded in the track matrix.
        rotation: DisplayRotation,
    },
    /// AVI format, which has no rotation to record.
    Avi,
}

/// An explicit video specification that drives rendering.
///
/// All parameters needed to render a video: the scene geometry, encoding parameters,
/// container format, and any defects to apply.
///
/// The encoder receives exactly the frames the scene produces. Fields for a coded size
/// or sample table differing from the scene's are absent because Scene cannot express them;
/// they belong on this type, defaulting to the scene's values. Byte-level corruption stays
/// in the defect list.
#[derive(Clone, Debug)]
pub struct VideoSpec {
    /// Manifest name: free text chosen by the spec builder, not derived from or checked
    /// against the codec or container.
    name: JsonEntryName,
    /// The scene to render: camera, background, objects, frame rate, dimensions.
    scene: Scene,
    /// How frames are encoded.
    encoding: Encoding,
    /// File container format.
    container: Container,
    /// Optional defect to apply to the media bytes after encoding.
    defect: Option<Defect>,
}

/// Why a combination of encoding, container and defect cannot be a specification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum VideoSpecError {
    /// The defect that leaves the size out of the container needs motion-JPEG frames in an
    /// ISO container: only then does the size survive in the frames for a reader to find.
    SizeOmissionNeedsMjpegInIso,
}

impl fmt::Display for VideoSpecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SizeOmissionNeedsMjpegInIso => formatter.write_str(
                "omitting the size from the container needs motion-JPEG frames in an ISO container",
            ),
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
            } => Ok(()),
            Container::Avi => Err(VideoSpecError::SizeOmissionNeedsMjpegInIso),
        },
        Encoding::Raw => Err(VideoSpecError::SizeOmissionNeedsMjpegInIso),
    }
}

impl VideoSpec {
    /// Creates a new video specification.
    ///
    /// # Errors
    ///
    /// Returns [`VideoSpecError`] when the defect cannot meet the encoding and container.
    pub fn new(
        name: JsonEntryName,
        scene: Scene,
        encoding: Encoding,
        container: Container,
        defect: Option<Defect>,
    ) -> Result<Self, VideoSpecError> {
        check_defect(encoding, container, defect.as_ref())?;
        Ok(Self {
            name,
            scene,
            encoding,
            container,
            defect,
        })
    }

    /// Returns the manifest name.
    #[must_use]
    pub const fn name(&self) -> &JsonEntryName {
        &self.name
    }

    /// Returns a reference to the scene.
    #[must_use]
    pub const fn scene(&self) -> &Scene {
        &self.scene
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

    /// Returns this specification with a different container.
    ///
    /// A combination the defect cannot meet is not refused here; [`render()`](fn@crate::render)
    /// refuses it.
    #[must_use]
    pub fn with_container(self, container: Container) -> Self {
        Self { container, ..self }
    }
}
