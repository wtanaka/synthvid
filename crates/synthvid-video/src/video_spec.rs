//! Video specification: the parameters that explicitly drive rendering.
//!
//! A `Scene` describes sensible content. A `VideoSpec` adds the choices that turn a scene
//! into a file, and its fields have the range of the file format's fields, so it can
//! describe a file that makes no sense. The manifest header's declared values are the
//! scene's computed values. A coded size or sample table that differs from the scene's has
//! no field here, because `Scene` cannot express it; such fields belong on `VideoSpec`,
//! defaulting to the scene's values. Byte-level corruption stays in the defect list.

use crate::json_names::JsonEntryName;
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

impl VideoSpec {
    /// Creates a new video specification.
    #[must_use]
    pub const fn new(
        name: JsonEntryName,
        scene: Scene,
        encoding: Encoding,
        container: Container,
        defect: Option<Defect>,
    ) -> Self {
        Self {
            name,
            scene,
            encoding,
            container,
            defect,
        }
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
    #[must_use]
    pub fn with_container(self, container: Container) -> Self {
        Self { container, ..self }
    }
}
