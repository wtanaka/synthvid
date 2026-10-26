//! Pipeline for rendering a video specification.
//!
//! Turns a [`VideoSpec`] into a manifest and media bytes. The pipeline is deterministic and
//! produces byte-identical output for identical inputs. The frames of a flat shot are produced
//! by the flat pipeline; the manifest is built here from those frames.

use crate::deep::DeepRenderError;
use crate::deep_pipeline::render_deep_shot_frames;
use crate::flat_pipeline::render_flat_frames;
use crate::grain::{apply_film_grain, GrainError};
use crate::manifest::{Manifest, ManifestCamera, ManifestHeader, ManifestObjectDecl};
use crate::manifest_deep::{deep_manifest_json, DeepManifestError};
use crate::manifest_error::ManifestError;
use crate::medium::Medium;
use crate::names::{BackgroundName, ShapeName};
use crate::render_output::{EncodedFrames, ManifestJson, MediaBytes};
use crate::rig::DeepShot;
use crate::sensor::{apply_sensor_noise, SensorError};
use crate::shot::{FlatShot, Shot};
use crate::video_spec::{
    check_defect, Container, DisplayRotation, Encoding, VideoSpec, VideoSpecError,
};
use crate::vignette::VignetteError;
use core::error::Error;
use core::fmt;
use core::num::TryFromIntError;
use synthvid_encode::{
    encode_avi, encode_iso, encode_jpeg, AviCodec, ContainerFile, ContainerFit, EncodedFrame,
    IsoCodec, TrackMatrix,
};
use synthvid_scene::{
    Background, Dimensions, Frame, FrameIndex, FrameRates, ObjectIndex, Scale, Shape,
};

/// A step of the exact view arithmetic that failed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ViewStep {
    /// Evaluating the view center.
    Center,
    /// Evaluating the view rotation.
    Rotation,
    /// Evaluating the view magnification.
    Zoom,
    /// Negating the rotation angle.
    NegateAngle,
    /// Taking the cosine of the rotation.
    Cosine,
    /// Taking the sine of the rotation.
    Sine,
    /// Forming the linear terms: the magnification times the cosine and sine.
    LinearTerms,
    /// Forming the horizontal translation.
    TranslationX,
    /// Forming the vertical translation.
    TranslationY,
}

/// Writes the step as a phrase.
impl fmt::Display for ViewStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Center => "evaluating the center",
            Self::Rotation => "evaluating the rotation",
            Self::Zoom => "evaluating the magnification",
            Self::NegateAngle => "negating the angle",
            Self::Cosine => "taking the cosine",
            Self::Sine => "taking the sine",
            Self::LinearTerms => "forming the linear terms",
            Self::TranslationX => "forming the horizontal translation",
            Self::TranslationY => "forming the vertical translation",
        })
    }
}

/// A step of the exact per-object arithmetic that failed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ObjectStep {
    /// Building the frame's bounds.
    FrameBounds,
    /// Taking the area of an object's box.
    BoxArea,
    /// Taking the area of the part of the box inside the frame.
    VisibleArea,
    /// Dividing the visible area by the box area.
    VisibleFraction,
    /// Converting a coordinate from scene to world units.
    WorldCoordinate,
    /// Recording the visible fraction for the manifest.
    OnScreenValue,
}

/// Writes the step as a phrase.
impl fmt::Display for ObjectStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::FrameBounds => "building the frame bounds",
            Self::BoxArea => "taking the box area",
            Self::VisibleArea => "taking the visible area",
            Self::VisibleFraction => "dividing the visible area by the box area",
            Self::WorldCoordinate => "converting a coordinate to world units",
            Self::OnScreenValue => "recording the visible fraction",
        })
    }
}

/// Error type for rendering a video specification.
///
/// A variant that wraps another error shows it in its message.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GenerationError {
    /// The specification's defect cannot meet its encoding and container.
    Specification(VideoSpecError),
    /// The frame buffer could not be allocated.
    FrameBuffer(synthvid_scene::FrameError),
    /// A step of the view arithmetic failed.
    ViewMath(ViewStep),
    /// There are more objects than an object index can number.
    ObjectIndexOutOfRange(TryFromIntError),
    /// The view similarity could not be widened to an affine without overflow.
    AffineConversion,
    /// Scene frame rendering failed.
    SceneFrameRender(synthvid_scene::RenderError),
    /// Manifest could not be created.
    ManifestConstruction(ManifestError),
    /// A step of the per-object arithmetic failed.
    ObjectState(ObjectStep),
    /// Raw frame data exceeds container capacity.
    RawCapacityExceeded,
    /// The track matrix for a display rotation could not be built.
    TrackMatrix(DisplayRotation),
    /// ISO encoding failed.
    IsoEncodingFailed(synthvid_encode::IsoError),
    /// AVI encoding failed.
    AviEncodingFailed(synthvid_encode::AviError),
    /// Defect application failed.
    DefectApplicationFailed(synthvid_encode::DefectError),
    /// The sensor noise of a frame could not be applied.
    SensorNoise(SensorError),
    /// The film grain of a frame could not be applied.
    FilmGrain(GrainError),
    /// The vignette of a frame could not be applied.
    Vignette(VignetteError),
    /// A deep shot's frame could not be sampled from its stage.
    DeepRender(DeepRenderError),
    /// A deep shot's manifest could not be written from its stage and rig.
    DeepManifest(DeepManifestError),
}

impl fmt::Display for GenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameBuffer(error) => {
                write!(formatter, "frame buffer allocation failed: {error}")
            }
            Self::ViewMath(step) => write!(formatter, "view arithmetic failed {step}"),
            Self::ObjectIndexOutOfRange(error) => {
                write!(formatter, "object index out of range: {error}")
            }
            Self::Specification(error) => write!(formatter, "specification invalid: {error}"),
            Self::AffineConversion => write!(formatter, "affine conversion failed"),
            Self::SceneFrameRender(error) => {
                write!(formatter, "scene frame rendering failed: {error}")
            }
            Self::ManifestConstruction(error) => {
                write!(formatter, "manifest construction failed: {error}")
            }
            Self::ObjectState(step) => {
                write!(formatter, "object state arithmetic failed {step}")
            }
            Self::RawCapacityExceeded => {
                write!(formatter, "raw frame data exceeds container capacity")
            }
            Self::TrackMatrix(rotation) => {
                write!(
                    formatter,
                    "track matrix for rotation {rotation} could not be built"
                )
            }
            Self::IsoEncodingFailed(error) => write!(formatter, "ISO encoding failed: {error}"),
            Self::AviEncodingFailed(error) => write!(formatter, "AVI encoding failed: {error}"),
            Self::DefectApplicationFailed(error) => {
                write!(formatter, "defect application failed: {error}")
            }
            Self::SensorNoise(error) => write!(formatter, "sensor noise failed: {error}"),
            Self::FilmGrain(error) => write!(formatter, "film grain failed: {error}"),
            Self::Vignette(error) => write!(formatter, "vignette failed: {error}"),
            Self::DeepRender(error) => write!(formatter, "deep shot rendering failed: {error}"),
            Self::DeepManifest(error) => write!(formatter, "deep manifest failed: {error}"),
        }
    }
}

impl Error for GenerationError {}

impl From<DeepRenderError> for GenerationError {
    fn from(error: DeepRenderError) -> Self {
        Self::DeepRender(error)
    }
}

/// Result of rendering a video specification: manifest JSON and media bytes.
#[derive(Clone, Debug)]
pub struct GeneratedEntry {
    /// Canonical JSON manifest bytes.
    manifest_json: ManifestJson,
    /// Media file bytes (video container).
    media_bytes: MediaBytes,
}

impl GeneratedEntry {
    /// Creates a new entry with the given manifest JSON and media bytes.
    #[must_use]
    pub(crate) const fn new(manifest_json: ManifestJson, media_bytes: MediaBytes) -> Self {
        Self {
            manifest_json,
            media_bytes,
        }
    }

    /// Returns the canonical JSON manifest.
    #[must_use]
    pub const fn manifest_json(&self) -> &ManifestJson {
        &self.manifest_json
    }

    /// Returns the media container's bytes.
    #[must_use]
    pub const fn media_bytes(&self) -> &MediaBytes {
        &self.media_bytes
    }
}

/// Applies a medium to a rendered frame: nothing, sensor noise, or film grain, as the medium
/// records the light.
///
/// # Errors
///
/// Returns `Err(GenerationError::SensorNoise)` or `Err(GenerationError::FilmGrain)` when the
/// medium cannot be applied to the frame.
pub(crate) fn apply_medium(
    frame: &mut Frame,
    medium: Medium,
    index: FrameIndex,
) -> Result<(), GenerationError> {
    match medium {
        Medium::Off => Ok(()),
        Medium::Sensor(noise) => {
            apply_sensor_noise(frame, &noise, index).map_err(GenerationError::SensorNoise)
        }
        Medium::Film(grain) => {
            apply_film_grain(frame, &grain, index).map_err(GenerationError::FilmGrain)
        }
    }
}

/// Encodes one rendered frame in the given encoding.
pub(crate) fn encode_frame(encoding: Encoding, frame: &Frame) -> EncodedFrame {
    match encoding {
        Encoding::Raw => EncodedFrame::raw(frame),
        Encoding::Mjpeg { quality, chroma } => encode_jpeg(frame, quality, chroma),
    }
}

/// The manifest name of a background.
const fn background_name_of(background: &Background) -> BackgroundName {
    match background {
        Background::Solid(_) => BackgroundName::Solid,
        Background::Checker {
            cell: _cell,
            even: _a,
            odd: _b,
        } => BackgroundName::Checker,
        Background::Gradient {
            from: _from,
            to: _to,
            direction: _direction,
        } => BackgroundName::Gradient,
        Background::Blobs {
            seed: _seed,
            count: _count,
            radii: _radii,
        } => BackgroundName::Blobs,
        Background::Grid {
            spacing: _spacing,
            line: _line,
            ground: _ground,
        } => BackgroundName::Grid,
    }
}

/// The manifest name of a shape.
const fn shape_name_of(shape: &Shape) -> ShapeName {
    match shape {
        Shape::Disc { radius: _radius } => ShapeName::Disc,
        Shape::Rect {
            half_width: _half_width,
            half_height: _half_height,
        } => ShapeName::Rect,
        Shape::Polygon {
            vertices: _vertices,
        } => ShapeName::Polygon,
        Shape::Cross {
            arm: _arm,
            thickness: _thickness,
        } => ShapeName::Cross,
    }
}

/// The manifest and encoded frames of a flat shot, with the header built from its scene and camera.
///
/// # Errors
///
/// Returns the errors of the frame rendering, the object declarations, and the manifest.
fn render_flat_shot(
    spec: &VideoSpec,
    flat: &FlatShot,
    frame_rates: FrameRates,
) -> Result<(EncodedFrames, ManifestJson), GenerationError> {
    let scene = flat.scene();
    let camera = flat.camera();
    let (encoded_frames, manifest_frames) = render_flat_frames(flat, spec.encoding())?;

    // Build manifest header
    let background_name = background_name_of(&scene.background);

    let header = ManifestHeader::new(
        spec.name().clone(),
        &ManifestCamera::new(camera.dimensions(), camera.medium(), camera.optics()),
        frame_rates,
        background_name,
        scene.scale.map(Scale::get),
    );

    // Build manifest objects declaration with proper error handling
    let mut manifest_objects = Vec::new();
    for (idx, obj) in scene.objects.iter().enumerate() {
        let shape_name = shape_name_of(&obj.shape);
        let obj_idx =
            ObjectIndex::new(u32::try_from(idx).map_err(GenerationError::ObjectIndexOutOfRange)?);

        manifest_objects.push(ManifestObjectDecl::new(obj_idx, shape_name, obj.visible));
    }

    // Create the manifest
    let manifest = Manifest::new(header, manifest_objects, manifest_frames)
        .map_err(GenerationError::ManifestConstruction)?;
    Ok((encoded_frames, manifest.to_canonical_json()))
}

/// The manifest and encoded frames of a deep shot. The frames are rendered from the stage, and
/// the manifest is written from the shot itself.
///
/// # Errors
///
/// Returns the errors of the frame rendering and of the deep manifest.
fn render_deep_shot(
    spec: &VideoSpec,
    deep: &DeepShot,
) -> Result<(EncodedFrames, ManifestJson), GenerationError> {
    let frames = render_deep_shot_frames(deep)?;
    let encoded: Vec<EncodedFrame> = frames
        .iter()
        .map(|frame| encode_frame(spec.encoding(), frame))
        .collect();
    let manifest_json =
        deep_manifest_json(spec.name(), deep).map_err(GenerationError::DeepManifest)?;
    Ok((EncodedFrames(encoded), manifest_json))
}

/// Renders a video specification to produce manifest and media bytes.
///
/// This function takes an explicit specification (`VideoSpec`) and performs:
/// 1. Specification checks
/// 2. Frame rendering and encoding
/// 3. Manifest construction
/// 4. Container creation
/// 5. Defect application
///
/// A flat shot and a deep shot share every step after the frames and the manifest.
///
/// # Errors
///
/// Returns `GenerationError` if any step fails.
pub fn render(spec: &VideoSpec) -> Result<GeneratedEntry, GenerationError> {
    check_defect(spec.encoding(), spec.container(), spec.defect())
        .map_err(GenerationError::Specification)?;
    let frame_rates = spec.frame_rates().map_err(GenerationError::Specification)?;
    let recorder = spec.recorder();

    // Check raw capacity for raw coding
    if spec.encoding() == Encoding::Raw
        && synthvid_encode::raw_frames_fit_classic_container(
            recorder.dimensions(),
            recorder.timing().count(),
        ) == ContainerFit::TooLarge
    {
        return Err(GenerationError::RawCapacityExceeded);
    }

    let (encoded_frames, manifest_json) = match spec.shot() {
        Shot::Flat(flat) => render_flat_shot(spec, flat, frame_rates)?,
        Shot::Deep(deep) => render_deep_shot(spec, deep)?,
    };

    let mut media_bytes = build_container(spec, frame_rates, &encoded_frames)?;

    // Apply defect if present
    if let Some(defect) = spec.defect() {
        media_bytes = synthvid_encode::apply(defect, &media_bytes)
            .map_err(GenerationError::DefectApplicationFailed)?;
    }

    Ok(GeneratedEntry::new(manifest_json, MediaBytes(media_bytes)))
}

/// Builds the media container around the encoded frames.
fn build_container(
    spec: &VideoSpec,
    frame_rates: FrameRates,
    encoded_frames: &EncodedFrames,
) -> Result<ContainerFile, GenerationError> {
    let dims = spec.recorder().dimensions();
    Ok(match spec.container() {
        Container::Iso {
            rotation,
            capture: _capture,
        } => {
            let track_matrix = build_track_matrix_from_rotation(rotation, dims)?;
            encode_iso(
                frame_rates,
                &encoded_frames.0,
                synthvid_encode::TrackParams {
                    width: dims.width,
                    height: dims.height,
                    codec: match spec.encoding() {
                        Encoding::Raw => IsoCodec::UncompressedRgb,
                        Encoding::Mjpeg {
                            quality: _quality,
                            chroma: _chroma,
                        } => IsoCodec::MotionJpeg,
                    },
                    track_matrix,
                },
            )
            .map_err(GenerationError::IsoEncodingFailed)?
        }
        Container::Avi => encode_avi(
            frame_rates.playback(),
            &encoded_frames.0,
            dims.width,
            dims.height,
            match spec.encoding() {
                Encoding::Raw => AviCodec::UncompressedRgb,
                Encoding::Mjpeg {
                    quality: _quality,
                    chroma: _chroma,
                } => AviCodec::MotionJpeg,
            },
        )
        .map_err(GenerationError::AviEncodingFailed)?,
    })
}

/// Builds the track matrix from a display rotation and dimensions.
fn build_track_matrix_from_rotation(
    rotation: DisplayRotation,
    dimensions: Dimensions,
) -> Result<TrackMatrix, GenerationError> {
    match rotation {
        DisplayRotation::Identity => Ok(TrackMatrix::identity()),
        DisplayRotation::Rot90 => {
            TrackMatrix::rotate_90(dimensions).ok_or(GenerationError::TrackMatrix(rotation))
        }
        DisplayRotation::Rot180 => {
            TrackMatrix::rotate_180(dimensions).ok_or(GenerationError::TrackMatrix(rotation))
        }
        DisplayRotation::Rot270 => {
            TrackMatrix::rotate_270(dimensions).ok_or(GenerationError::TrackMatrix(rotation))
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod deep_tests;
