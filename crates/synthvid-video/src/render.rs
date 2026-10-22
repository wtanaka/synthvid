//! Pipeline for rendering a video specification.
//!
//! Turns a [`VideoSpec`] into a manifest and media bytes. The pipeline is deterministic and
//! produces byte-identical output for identical inputs.

use crate::manifest::{
    Manifest, ManifestAffine, ManifestFrame, ManifestHeader, ManifestObjectDecl,
};
use crate::manifest_error::ManifestError;
use crate::names::{BackgroundName, ShapeName};
use crate::object_state::frame_objects;
use crate::render_output::{EncodedFrames, ManifestJson, MediaBytes};
use crate::video_spec::{
    check_defect, Container, DisplayRotation, Encoding, VideoSpec, VideoSpecError,
};
use core::error::Error;
use core::fmt;
use core::num::TryFromIntError;
use synthvid_encode::{
    encode_avi, encode_iso, encode_jpeg, AviCodec, ContainerFile, ContainerFit, EncodedFrame,
    IsoCodec, TrackMatrix,
};
use synthvid_scene::{
    render_into, Background, Camera, Dimensions, Frame, FrameIndex, FrameRates, ObjectIndex,
    Overflow, Scale, Scene, Shape,
};

/// A step of the exact camera arithmetic that failed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CameraStep {
    /// Evaluating the camera centre.
    Centre,
    /// Evaluating the camera rotation.
    Rotation,
    /// Evaluating the camera magnification.
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
impl fmt::Display for CameraStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Centre => "evaluating the centre",
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
    /// A step of the camera arithmetic failed.
    CameraMath(CameraStep),
    /// There are more objects than an object index can number.
    ObjectIndexOutOfRange(TryFromIntError),
    /// The camera similarity could not be widened to an affine without overflow.
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
}

impl fmt::Display for GenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameBuffer(error) => {
                write!(formatter, "frame buffer allocation failed: {error}")
            }
            Self::CameraMath(step) => write!(formatter, "camera arithmetic failed {step}"),
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
        }
    }
}

impl Error for GenerationError {}

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

/// Renders frames and builds encoded frames and manifest frames.
fn render_frames_to_manifest(
    spec: &VideoSpec,
    scene: &Scene,
) -> Result<(EncodedFrames, Vec<ManifestFrame>), GenerationError> {
    let mut manifest_frames = Vec::new();
    let mut encoded_frames = Vec::new();

    let mut frame_buffer = Frame::zeroed(scene.dimensions).map_err(GenerationError::FrameBuffer)?;

    for frame_idx in 0..scene.frame_count.get().get() {
        let frame_index = FrameIndex::new(frame_idx);

        render_into(scene, frame_index, &mut frame_buffer)
            .map_err(GenerationError::SceneFrameRender)?;
        let rendered_frame = &frame_buffer;

        let encoded = match spec.encoding() {
            Encoding::Raw => EncodedFrame::raw(rendered_frame),
            Encoding::Mjpeg { quality, chroma } => encode_jpeg(rendered_frame, quality, chroma),
        };
        encoded_frames.push(encoded);

        let camera_affine = compute_camera_affine(&scene.camera, frame_index)?;
        let camera_manifest = ManifestAffine::from_affine(
            &camera_affine
                .to_affine()
                .map_err(|Overflow| GenerationError::AffineConversion)?,
        );

        let objects = frame_objects(scene, frame_index)?;
        let manifest_frame = ManifestFrame::new(&camera_manifest, objects);
        manifest_frames.push(manifest_frame);
    }

    Ok((EncodedFrames(encoded_frames), manifest_frames))
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

/// Renders a video specification to produce manifest and media bytes.
///
/// This function takes an explicit specification (`VideoSpec`) and performs:
/// 1. Scene validation
/// 2. Frame rendering and encoding
/// 3. Manifest construction
/// 4. Container creation
/// 5. Defect application
///
/// # Errors
///
/// Returns `GenerationError` if any step fails.
pub fn render(spec: &VideoSpec) -> Result<GeneratedEntry, GenerationError> {
    let scene = spec.scene();

    check_defect(spec.encoding(), spec.container(), spec.defect())
        .map_err(GenerationError::Specification)?;
    let frame_rates = spec.frame_rates().map_err(GenerationError::Specification)?;

    // Check raw capacity for raw coding
    if spec.encoding() == Encoding::Raw
        && synthvid_encode::raw_frames_fit_classic_container(scene.dimensions, scene.frame_count)
            == ContainerFit::TooLarge
    {
        return Err(GenerationError::RawCapacityExceeded);
    }

    let (encoded_frames, manifest_frames) = render_frames_to_manifest(spec, scene)?;

    // Build manifest header
    let background_name = background_name_of(&scene.background);

    let header = ManifestHeader::new(
        spec.name().clone(),
        scene.dimensions,
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
    let manifest_json = manifest.to_canonical_json();

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
    let scene = spec.scene();
    Ok(match spec.container() {
        Container::Iso {
            rotation,
            capture: _capture,
        } => {
            let dims = scene.dimensions;
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
            scene.dimensions.width,
            scene.dimensions.height,
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

/// Computes a camera affine transform for a frame.
fn compute_camera_affine(
    camera: &Camera,
    frame: FrameIndex,
) -> Result<synthvid_scene::Similarity, GenerationError> {
    use synthvid_scene::{position_at, rotation_at, zoom_at};

    let centre = position_at(&camera.motion, frame)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::Centre))?;
    let angle = rotation_at(&camera.rotation, frame)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::Rotation))?;
    let magnification = zoom_at(&camera.zoom, frame)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::Zoom))?;

    let zoom = magnification.get();
    let angle_value = angle.get();
    let turn = angle_value
        .checked_neg()
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::NegateAngle))?;
    let cos = synthvid_scene::cos_turns(turn)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::Cosine))?;
    let sin = synthvid_scene::sin_turns(turn)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::Sine))?;
    let neg_sin = sin
        .checked_neg()
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::LinearTerms))?;

    let scale_cos = zoom
        .checked_mul(cos)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::LinearTerms))?;
    let scale_neg_sin = zoom
        .checked_mul(neg_sin)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::LinearTerms))?;
    let scale_sin = zoom
        .checked_mul(sin)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::LinearTerms))?;

    let ax = scale_cos
        .checked_mul(centre.across)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::TranslationX))?;
    let bx = scale_neg_sin
        .checked_mul(centre.down)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::TranslationX))?;
    let sum_x = ax
        .checked_add(bx)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::TranslationX))?;
    let tx = sum_x
        .checked_neg()
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::TranslationX))?;

    let cx = scale_sin
        .checked_mul(centre.across)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::TranslationY))?;
    let dx = scale_cos
        .checked_mul(centre.down)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::TranslationY))?;
    let sum_y = cx
        .checked_add(dx)
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::TranslationY))?;
    let ty = sum_y
        .checked_neg()
        .map_err(|Overflow| GenerationError::CameraMath(CameraStep::TranslationY))?;

    Ok(synthvid_scene::Similarity {
        xx: scale_cos,
        xy: scale_neg_sin,
        tx,
        ty,
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
