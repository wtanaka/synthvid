//! Pipeline for rendering a video specification.
//!
//! Turns a [`VideoSpec`] into a manifest and media bytes. The pipeline is deterministic and
//! produces byte-identical output for identical inputs.

use crate::manifest::{
    Manifest, ManifestAffine, ManifestFrame, ManifestHeader, ManifestObjectDecl,
};
use crate::names::{BackgroundName, ShapeName};
use crate::object_state::frame_objects;
use crate::video_spec::{Container, DisplayRotation, Encoding, VideoSpec};
use core::fmt;
use synthvid_encode::{encode_avi, encode_iso, encode_jpeg, AviCodec, IsoCodec, TrackMatrix};
use synthvid_scene::{
    render_into, Background, Camera, Dimensions, Frame, FrameIndex, ObjectIndex, Scale, Scene,
    Shape,
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

impl CameraStep {
    /// The step as a phrase.
    const fn describe(self) -> &'static str {
        match self {
            Self::Centre => "evaluating the centre",
            Self::Rotation => "evaluating the rotation",
            Self::Zoom => "evaluating the magnification",
            Self::NegateAngle => "negating the angle",
            Self::Cosine => "taking the cosine",
            Self::Sine => "taking the sine",
            Self::LinearTerms => "forming the linear terms",
            Self::TranslationX => "forming the horizontal translation",
            Self::TranslationY => "forming the vertical translation",
        }
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

impl ObjectStep {
    /// The step as a phrase.
    const fn describe(self) -> &'static str {
        match self {
            Self::FrameBounds => "building the frame bounds",
            Self::BoxArea => "taking the box area",
            Self::VisibleArea => "taking the visible area",
            Self::VisibleFraction => "dividing the visible area by the box area",
            Self::WorldCoordinate => "converting a coordinate to world units",
            Self::OnScreenValue => "recording the visible fraction",
        }
    }
}

/// Error type for rendering a video specification.
///
/// A variant that wraps another error shows it in its message.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GenerationError {
    /// The frame buffer could not be allocated.
    FrameBuffer(synthvid_scene::FrameError),
    /// A step of the camera arithmetic failed.
    CameraMath(CameraStep),
    /// There are more objects than an object index can number.
    ObjectIndexOutOfRange(core::num::TryFromIntError),
    /// The camera similarity could not be widened to an affine without overflow.
    AffineConversion,
    /// Scene frame rendering failed.
    SceneFrameRender(synthvid_scene::RenderError),
    /// Manifest could not be created.
    ManifestConstruction,
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameBuffer(e) => write!(f, "frame buffer allocation failed: {e}"),
            Self::CameraMath(step) => write!(f, "camera arithmetic failed {}", step.describe()),
            Self::ObjectIndexOutOfRange(e) => write!(f, "object index out of range: {e}"),
            Self::AffineConversion => write!(f, "affine conversion failed"),
            Self::SceneFrameRender(e) => write!(f, "scene frame rendering failed: {e}"),
            Self::ManifestConstruction => write!(f, "manifest construction failed"),
            Self::ObjectState(step) => {
                write!(f, "object state arithmetic failed {}", step.describe())
            }
            Self::RawCapacityExceeded => write!(f, "raw frame data exceeds container capacity"),
            Self::TrackMatrix(rotation) => {
                write!(
                    f,
                    "track matrix for rotation {rotation:?} could not be built"
                )
            }
            Self::IsoEncodingFailed(e) => write!(f, "ISO encoding failed: {e}"),
            Self::AviEncodingFailed(e) => write!(f, "AVI encoding failed: {e}"),
            Self::DefectApplicationFailed(e) => write!(f, "defect application failed: {e}"),
        }
    }
}

impl core::error::Error for GenerationError {}

/// Result of rendering a video specification: manifest JSON and media bytes.
#[derive(Clone, Debug)]
pub struct GeneratedEntry {
    /// Canonical JSON manifest bytes.
    manifest_json: String,
    /// Media file bytes (video container).
    media_bytes: Vec<u8>,
}

impl GeneratedEntry {
    /// Creates a new entry with the given manifest JSON and media bytes.
    #[must_use]
    pub(crate) const fn new(manifest_json: String, media_bytes: Vec<u8>) -> Self {
        Self {
            manifest_json,
            media_bytes,
        }
    }

    /// Returns the manifest JSON as a string slice.
    #[must_use]
    pub fn manifest_json(&self) -> &str {
        &self.manifest_json
    }

    /// Returns the media bytes as a slice.
    #[must_use]
    pub fn media_bytes(&self) -> &[u8] {
        &self.media_bytes
    }
}

/// Renders frames and builds encoded frames and manifest frames.
fn render_frames_to_manifest(
    spec: &VideoSpec,
    scene: &Scene,
) -> Result<(Vec<Vec<u8>>, Vec<ManifestFrame>), GenerationError> {
    let mut manifest_frames = Vec::new();
    let mut encoded_frames = Vec::new();

    let mut frame_buffer = Frame::zeroed(scene.dimensions).map_err(GenerationError::FrameBuffer)?;

    for frame_idx in 0..scene.frame_count.get().get() {
        let frame_index = FrameIndex::new(frame_idx);

        render_into(scene, frame_index, &mut frame_buffer)
            .map_err(GenerationError::SceneFrameRender)?;
        let rendered_frame = &frame_buffer;

        let encoded = match spec.encoding() {
            Encoding::Raw => rendered_frame.data().to_vec(),
            Encoding::Mjpeg { quality, chroma } => encode_jpeg(rendered_frame, quality, chroma),
        };
        encoded_frames.push(encoded);

        let camera_affine = compute_camera_affine(&scene.camera, frame_index)?;
        let camera_manifest = ManifestAffine::from_affine(
            camera_affine
                .to_affine()
                .map_err(|_| GenerationError::AffineConversion)?,
        );

        let objects = frame_objects(scene, frame_index)?;
        let manifest_frame = ManifestFrame::new(camera_manifest, objects);
        manifest_frames.push(manifest_frame);
    }

    Ok((encoded_frames, manifest_frames))
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

    // Check raw capacity for raw coding
    if matches!(spec.encoding(), Encoding::Raw)
        && !synthvid_encode::raw_frames_fit_classic_container(
            scene.dimensions.width,
            scene.dimensions.height,
            scene.frame_count.get().get(),
        )
    {
        return Err(GenerationError::RawCapacityExceeded);
    }

    let (encoded_frames, manifest_frames) = render_frames_to_manifest(spec, scene)?;

    // Build manifest header
    let background_name = match scene.background {
        Background::Solid(_) => BackgroundName::Solid,
        Background::Checker { .. } => BackgroundName::Checker,
        Background::Gradient { .. } => BackgroundName::Gradient,
        Background::Blobs { .. } => BackgroundName::Blobs,
        Background::Grid { .. } => BackgroundName::Grid,
    };

    let header = ManifestHeader::new(
        spec.name().clone(),
        scene.dimensions,
        scene.frame_rate,
        background_name,
        scene.scale.map(Scale::get),
    );

    // Build manifest objects declaration with proper error handling
    let mut manifest_objects = Vec::new();
    for (idx, obj) in scene.objects.iter().enumerate() {
        let shape_name = match &obj.shape {
            Shape::Disc { .. } => ShapeName::Disc,
            Shape::Rect { .. } => ShapeName::Rect,
            Shape::Polygon { .. } => ShapeName::Polygon,
            Shape::Cross { .. } => ShapeName::Cross,
        };
        let obj_idx =
            ObjectIndex::new(u32::try_from(idx).map_err(GenerationError::ObjectIndexOutOfRange)?);

        manifest_objects.push(ManifestObjectDecl::new(obj_idx, shape_name, obj.visible));
    }

    // Create the manifest
    let manifest = Manifest::new(header, manifest_objects, manifest_frames)
        .ok_or(GenerationError::ManifestConstruction)?;
    let manifest_json = manifest.to_canonical_json();

    // Build the media container
    let mut media_bytes = match spec.container() {
        Container::Iso { rotation } => {
            let dims = scene.dimensions;
            let track_matrix = build_track_matrix_from_rotation(rotation, dims)?;
            encode_iso(
                scene.frame_rate,
                None,
                &encoded_frames,
                dims.width,
                dims.height,
                match spec.encoding() {
                    Encoding::Raw => IsoCodec::UncompressedRgb,
                    Encoding::Mjpeg { .. } => IsoCodec::MotionJpeg,
                },
                track_matrix,
            )
            .map_err(GenerationError::IsoEncodingFailed)?
        }
        Container::Avi => encode_avi(
            scene.frame_rate,
            &encoded_frames,
            scene.dimensions.width,
            scene.dimensions.height,
            match spec.encoding() {
                Encoding::Raw => AviCodec::UncompressedRgb,
                Encoding::Mjpeg { .. } => AviCodec::MotionJpeg,
            },
        )
        .map_err(GenerationError::AviEncodingFailed)?,
    };

    // Apply defect if present
    if let Some(defect) = spec.defect() {
        media_bytes = synthvid_encode::apply(defect, &media_bytes)
            .map_err(GenerationError::DefectApplicationFailed)?;
    }

    Ok(GeneratedEntry::new(manifest_json, media_bytes))
}

/// Computes a camera affine transform for a frame.
fn compute_camera_affine(
    camera: &Camera,
    frame: FrameIndex,
) -> Result<synthvid_scene::Similarity, GenerationError> {
    use synthvid_scene::{position_at, rotation_at, zoom_at};

    let centre = position_at(&camera.motion, frame)
        .map_err(|_| GenerationError::CameraMath(CameraStep::Centre))?;
    let angle = rotation_at(&camera.rotation, frame)
        .map_err(|_| GenerationError::CameraMath(CameraStep::Rotation))?;
    let magnification =
        zoom_at(&camera.zoom, frame).map_err(|_| GenerationError::CameraMath(CameraStep::Zoom))?;

    let zoom = magnification.get();
    let angle_value = angle.get();
    let turn = angle_value
        .checked_neg()
        .map_err(|_| GenerationError::CameraMath(CameraStep::NegateAngle))?;
    let cos = synthvid_scene::cos_turns(turn)
        .map_err(|_| GenerationError::CameraMath(CameraStep::Cosine))?;
    let sin = synthvid_scene::sin_turns(turn)
        .map_err(|_| GenerationError::CameraMath(CameraStep::Sine))?;
    let neg_sin = sin
        .checked_neg()
        .map_err(|_| GenerationError::CameraMath(CameraStep::LinearTerms))?;

    let a = zoom
        .checked_mul(cos)
        .map_err(|_| GenerationError::CameraMath(CameraStep::LinearTerms))?;
    let b = zoom
        .checked_mul(neg_sin)
        .map_err(|_| GenerationError::CameraMath(CameraStep::LinearTerms))?;
    let c = zoom
        .checked_mul(sin)
        .map_err(|_| GenerationError::CameraMath(CameraStep::LinearTerms))?;

    let ax = a
        .checked_mul(centre.x)
        .map_err(|_| GenerationError::CameraMath(CameraStep::TranslationX))?;
    let bx = b
        .checked_mul(centre.y)
        .map_err(|_| GenerationError::CameraMath(CameraStep::TranslationX))?;
    let sum_x = ax
        .checked_add(bx)
        .map_err(|_| GenerationError::CameraMath(CameraStep::TranslationX))?;
    let tx = sum_x
        .checked_neg()
        .map_err(|_| GenerationError::CameraMath(CameraStep::TranslationX))?;

    let cx = c
        .checked_mul(centre.x)
        .map_err(|_| GenerationError::CameraMath(CameraStep::TranslationY))?;
    let dx = a
        .checked_mul(centre.y)
        .map_err(|_| GenerationError::CameraMath(CameraStep::TranslationY))?;
    let sum_y = cx
        .checked_add(dx)
        .map_err(|_| GenerationError::CameraMath(CameraStep::TranslationY))?;
    let ty = sum_y
        .checked_neg()
        .map_err(|_| GenerationError::CameraMath(CameraStep::TranslationY))?;

    Ok(synthvid_scene::Similarity::new(a, b, tx, ty))
}

/// Builds the track matrix from a display rotation and dimensions.
fn build_track_matrix_from_rotation(
    rotation: DisplayRotation,
    dimensions: Dimensions,
) -> Result<TrackMatrix, GenerationError> {
    use DisplayRotation;

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
