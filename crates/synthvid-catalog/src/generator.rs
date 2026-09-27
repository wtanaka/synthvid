//! Pipeline for generating video artifacts from catalog entries.
//!
//! Converts a catalog entry to a fully rendered scene with manifest and media bytes.
//! The pipeline is deterministic and produces byte-identical output for identical inputs.

use crate::cover::{CodingAxis, ContainerAxis, Entry};
use crate::manifest::{
    Manifest, ManifestAffine, ManifestFrame, ManifestHeader, ManifestObjectDecl,
};
use crate::names::{BackgroundName, ShapeName};
use crate::object_state::frame_objects;
use core::fmt;
use core::num::NonZeroU16;
use synthvid_encode::{
    encode_avi, encode_iso, encode_jpeg, AviCodec, ChromaSampling, IsoCodec, Quality, TrackMatrix,
};
use synthvid_scene::{
    render_into, Background, Camera, Direction, Frame, FrameCount, FrameIndex, FrameRate,
    FrameSpan, Magnification, Motion, ObjectIndex, Point, Ratio, Rgb8, Rotation, Scale, Scene,
    Seed, Shape, Turns, Vector, Zoom,
};

/// Error type for entry generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenerationError {
    /// Scene could not be constructed.
    SceneConstruction(String),
    /// Frame rendering failed.
    RenderError(String),
    /// Manifest could not be created.
    ManifestConstruction,
    /// Media encoding failed.
    EncodingError(String),
    /// Defect application failed.
    DefectError(String),
}

impl fmt::Display for GenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SceneConstruction(msg) => write!(f, "scene construction failed: {msg}"),
            Self::RenderError(msg) => write!(f, "render error: {msg}"),
            Self::ManifestConstruction => write!(f, "manifest construction failed"),
            Self::EncodingError(msg) => write!(f, "encoding error: {msg}"),
            Self::DefectError(msg) => write!(f, "defect application failed: {msg}"),
        }
    }
}

impl core::error::Error for GenerationError {}

/// Result of generating an entry: manifest JSON and media bytes.
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

/// Rejects raw (uncompressed) entries whose frame data would exceed a
/// classic container's 32-bit size limit, before any frame is rendered or
/// encoded, rather than discovering it only after that work is done.
fn check_raw_capacity(entry: &Entry) -> Result<(), GenerationError> {
    if entry.coding == CodingAxis::Raw
        && !synthvid_encode::raw_frames_fit_classic_container(
            entry.dimensions.width,
            entry.dimensions.height,
            entry.frame_count.get(),
        )
    {
        return Err(GenerationError::EncodingError(
            "raw frame data exceeds container capacity".to_owned(),
        ));
    }
    Ok(())
}

/// Generates a manifest and media file from a catalog entry.
///
/// This is the main entry point for the generation pipeline. It:
/// 1. Builds a scene from the entry's axis values
/// 2. Renders each frame and computes manifest data
/// 3. Encodes frames based on the coding axis
/// 4. Builds the appropriate container (AVI or ISO/MP4)
/// 5. Applies any defect if specified
/// 6. Returns the manifest JSON and media bytes
///
/// # Errors
///
/// Returns `GenerationError` if any step fails.
pub fn generate_entry(entry: &Entry) -> Result<GeneratedEntry, GenerationError> {
    // Build the scene from the entry
    let scene = build_scene(entry)?;

    check_raw_capacity(entry)?;

    // Render all frames and build manifest data
    let manifest_frames = Vec::new();
    let mut manifest_frames = manifest_frames;
    let mut encoded_frames = Vec::new();

    // Allocated once and reused across every frame of this entry: each
    // iteration's paint_background call overwrites every pixel before
    // anything else reads the buffer, so a stale previous frame's contents
    // never leak through (see render_into's own contract). Reusing the
    // buffer instead of calling render_frame per frame avoids one extra
    // full-frame allocation and zero-fill for every frame beyond the first.
    let mut frame_buffer = Frame::zeroed(scene.dimensions)
        .map_err(|_| GenerationError::RenderError("frame buffer allocation failed".to_owned()))?;

    for frame_idx in 0..scene.frame_count.get().get() {
        let frame_index = FrameIndex::new(frame_idx);

        // Render the frame into the reused buffer
        render_into(&scene, frame_index, &mut frame_buffer)
            .map_err(|e| GenerationError::RenderError(e.to_string()))?;
        let rendered_frame = &frame_buffer;

        // Encode the frame based on coding axis
        let encoded = match entry.coding {
            CodingAxis::Raw => rendered_frame.data().to_vec(),
            CodingAxis::MotionJpeg => {
                let quality = Quality::new(75).ok_or_else(|| {
                    GenerationError::EncodingError("invalid quality value".to_owned())
                })?;
                encode_jpeg(rendered_frame, quality, ChromaSampling::Yuv420)
            }
        };
        encoded_frames.push(encoded);

        // Build manifest frame data: the camera, and every object drawn
        let camera_affine = compute_camera_affine(&scene.camera, frame_index)?;
        let camera_manifest =
            ManifestAffine::from_affine(camera_affine.to_affine().map_err(|_| {
                GenerationError::RenderError("camera affine conversion failed".to_owned())
            })?);

        let objects = frame_objects(&scene, frame_index)?;
        let manifest_frame = ManifestFrame::new(camera_manifest, objects);
        manifest_frames.push(manifest_frame);
    }

    // Build manifest header
    let background_name = match entry.background {
        crate::cover::BackgroundAxis::Solid => BackgroundName::Solid,
        crate::cover::BackgroundAxis::Checker => BackgroundName::Checker,
        crate::cover::BackgroundAxis::Gradient => BackgroundName::Gradient,
        crate::cover::BackgroundAxis::Blobs => BackgroundName::Blobs,
        crate::cover::BackgroundAxis::Grid => BackgroundName::Grid,
    };

    let header = ManifestHeader::new(
        entry.manifest_name(),
        scene.dimensions,
        scene.frame_rate,
        background_name,
        scene.scale.map(Scale::get),
    );

    // Build manifest objects declaration
    let mut manifest_objects = Vec::new();
    for (idx, obj) in scene.objects.iter().enumerate() {
        let shape_name = match &obj.shape {
            Shape::Disc { .. } => ShapeName::Disc,
            Shape::Rect { .. } => ShapeName::Rect,
            Shape::Polygon { .. } => ShapeName::Polygon,
            Shape::Cross { .. } => ShapeName::Cross,
        };
        let obj_idx = ObjectIndex::new(u32::try_from(idx).map_err(|_| {
            GenerationError::SceneConstruction("object index out of range".to_owned())
        })?);
        if let Ok(manifest_obj) = FrameSpan::new(obj.visible.start, obj.visible.end) {
            manifest_objects.push(ManifestObjectDecl::new(obj_idx, shape_name, manifest_obj));
        }
    }

    // Create the manifest
    let manifest = Manifest::new(header, manifest_objects, manifest_frames)
        .ok_or(GenerationError::ManifestConstruction)?;
    let manifest_json = manifest.to_canonical_json();

    // Build the media container
    let mut media_bytes = match entry.container {
        ContainerAxis::Iso => encode_iso(
            scene.frame_rate,
            None,
            &encoded_frames,
            scene.dimensions.width,
            scene.dimensions.height,
            match entry.coding {
                CodingAxis::Raw => IsoCodec::UncompressedRgb,
                CodingAxis::MotionJpeg => IsoCodec::MotionJpeg,
            },
            build_track_matrix(entry)?,
        )
        .map_err(|e| GenerationError::EncodingError(e.to_string()))?,
        ContainerAxis::Avi => encode_avi(
            scene.frame_rate,
            &encoded_frames,
            scene.dimensions.width,
            scene.dimensions.height,
            match entry.coding {
                CodingAxis::Raw => AviCodec::UncompressedRgb,
                CodingAxis::MotionJpeg => AviCodec::MotionJpeg,
            },
        )
        .map_err(|e| GenerationError::EncodingError(e.to_string()))?,
    };

    // Apply defect if present
    if let Some(defect) = &entry.defect {
        media_bytes = synthvid_encode::apply(defect, &media_bytes)
            .map_err(|e| GenerationError::DefectError(e.to_string()))?;
    }

    Ok(GeneratedEntry::new(manifest_json, media_bytes))
}

/// Builds a scene from a catalog entry.
fn build_scene(entry: &Entry) -> Result<Scene, GenerationError> {
    // Build frame rate from FrameRateValue
    let frame_rate = frame_rate_from_axis(entry.frame_rate)?;

    // Build camera with fixed origin and identity rotation
    let camera = build_camera()?;

    // Build background
    let background = build_background(entry)?;

    // Build a single object based on entry's shape and motion
    let object = build_object_from_entry(entry)?;

    Ok(Scene::new(
        entry.dimensions,
        frame_rate,
        FrameCount::new(entry.frame_count.get())
            .map_err(|_| GenerationError::SceneConstruction("frame count invalid".to_owned()))?,
        match entry.scale {
            crate::cover::ScaleAxis::NoScale => None,
            crate::cover::ScaleAxis::WithScale => Some(declared_scale()?),
        },
        background,
        camera,
        vec![object],
    ))
}

/// Scene units per world unit declared by a scaled entry.
///
/// Deliberately not an integer, so that a world coordinate is never equal to
/// its scene coordinate by accident.
const SCALED_UNITS_PER_WORLD_UNIT: RatioParts = RatioParts {
    numerator: 5,
    denominator: 2,
};

/// Builds the scale a scaled entry declares.
fn declared_scale() -> Result<Scale, GenerationError> {
    Scale::new(make_ratio(SCALED_UNITS_PER_WORLD_UNIT)?)
        .map_err(|e| GenerationError::SceneConstruction(e.to_string()))
}

/// Builds a camera with fixed origin and identity rotation.
fn build_camera() -> Result<Camera, GenerationError> {
    let camera_origin = Point::new(Ratio::from_integer(0), Ratio::from_integer(0));
    let camera_motion = Motion::Fixed(camera_origin);
    let zero_ratio = Ratio::from_integer(0);
    let camera_rotation = Rotation::Fixed(Turns::new(zero_ratio));
    let one_ratio = Ratio::from_integer(1);
    let camera_zoom = Zoom::Fixed(Magnification::new(one_ratio).map_err(|_| {
        GenerationError::SceneConstruction("camera magnification failed".to_owned())
    })?);
    Ok(Camera::new(camera_motion, camera_rotation, camera_zoom))
}

/// Builds a background from the entry's background axis.
fn build_background(entry: &Entry) -> Result<Background, GenerationError> {
    Ok(match entry.background {
        crate::cover::BackgroundAxis::Solid => Background::Solid(Rgb8::new(128, 128, 128)),
        crate::cover::BackgroundAxis::Checker => Background::Checker {
            cell: NonZeroU16::new(16).ok_or_else(|| {
                GenerationError::SceneConstruction("checker cell failed".to_owned())
            })?,
            a: Rgb8::new(200, 200, 200),
            b: Rgb8::new(100, 100, 100),
        },
        crate::cover::BackgroundAxis::Gradient => Background::Gradient {
            from: Rgb8::new(50, 50, 50),
            to: Rgb8::new(200, 200, 200),
            direction: Direction::Horizontal,
        },
        crate::cover::BackgroundAxis::Blobs => Background::Blobs {
            seed: Seed::new(42),
            count: 10,
            min_radius: 4,
            max_radius: 20,
        },
        crate::cover::BackgroundAxis::Grid => Background::Grid {
            spacing: NonZeroU16::new(32).ok_or_else(|| {
                GenerationError::SceneConstruction("grid spacing failed".to_owned())
            })?,
            line: Rgb8::new(100, 100, 100),
            ground: Rgb8::new(180, 180, 180),
        },
    })
}

/// Builds an object from the entry's shape and motion axes.
fn build_object_from_entry(entry: &Entry) -> Result<synthvid_scene::Object, GenerationError> {
    let shape = match entry.shape {
        crate::cover::ShapeAxis::Disc => Shape::Disc {
            radius: Ratio::from_integer(20),
        },
        crate::cover::ShapeAxis::Rect => Shape::Rect {
            half_width: Ratio::from_integer(30),
            half_height: Ratio::from_integer(20),
        },
        crate::cover::ShapeAxis::Polygon => {
            let vertices = vec![
                Point::new(Ratio::from_integer(0), Ratio::from_integer(-30)),
                Point::new(Ratio::from_integer(30), Ratio::from_integer(0)),
                Point::new(Ratio::from_integer(0), Ratio::from_integer(30)),
                Point::new(Ratio::from_integer(-30), Ratio::from_integer(0)),
            ];
            Shape::Polygon { vertices }
        }
        crate::cover::ShapeAxis::Cross => Shape::Cross {
            arm: Ratio::from_integer(25),
            thickness: Ratio::from_integer(8),
        },
    };

    let motion = build_motion_from_entry(entry)?;

    let visible = FrameSpan::new(FrameIndex::new(0), FrameIndex::new(entry.frame_count.get()))
        .map_err(|_| GenerationError::SceneConstruction("visibility span failed".to_owned()))?;

    Ok(synthvid_scene::Object::new(
        shape,
        Rgb8::new(255, 100, 100),
        motion,
        visible,
    ))
}

/// Parameters for creating a ratio with named fields.
#[derive(Copy, Clone, Debug)]
struct RatioParts {
    /// The numerator of the ratio.
    numerator: i64,
    /// The denominator of the ratio.
    denominator: i64,
}

/// Helper to create a `Ratio` with error handling for `NonZeroI64` construction.
fn make_ratio(parts: RatioParts) -> Result<Ratio, GenerationError> {
    let denom = core::num::NonZeroI64::new(parts.denominator)
        .ok_or_else(|| GenerationError::SceneConstruction("NonZeroI64 failed".to_owned()))?;
    Ratio::new(parts.numerator, denom)
        .map_err(|_| GenerationError::SceneConstruction("ratio construction failed".to_owned()))
}

/// Extracts width and height from an entry's dimensions as i64 values.
fn dimensions_as_i64(entry: &Entry) -> (i64, i64) {
    let width_i64 = i64::from(entry.dimensions.width.get().get());
    let height_i64 = i64::from(entry.dimensions.height.get().get());
    (width_i64, height_i64)
}

/// Builds a fixed motion at the center of the screen.
fn build_fixed_motion(entry: &Entry) -> Result<Motion, GenerationError> {
    let (width, height) = dimensions_as_i64(entry);
    Ok(Motion::Fixed(Point::new(
        make_ratio(RatioParts {
            numerator: width,
            denominator: 2,
        })?,
        make_ratio(RatioParts {
            numerator: height,
            denominator: 2,
        })?,
    )))
}

/// Builds a linear motion with constant velocity.
fn build_linear_motion(entry: &Entry) -> Result<Motion, GenerationError> {
    let (width, height) = dimensions_as_i64(entry);
    Ok(Motion::Linear {
        origin: Point::new(
            make_ratio(RatioParts {
                numerator: width,
                denominator: 4,
            })?,
            make_ratio(RatioParts {
                numerator: height,
                denominator: 2,
            })?,
        ),
        velocity: Vector::new(
            make_ratio(RatioParts {
                numerator: 1,
                denominator: 2,
            })?,
            Ratio::from_integer(0),
        ),
    })
}

/// Builds a ballistic motion with constant acceleration.
fn build_ballistic_motion(entry: &Entry) -> Result<Motion, GenerationError> {
    let (width, height) = dimensions_as_i64(entry);
    Ok(Motion::Ballistic {
        origin: Point::new(
            make_ratio(RatioParts {
                numerator: width,
                denominator: 4,
            })?,
            make_ratio(RatioParts {
                numerator: height,
                denominator: 4,
            })?,
        ),
        velocity: Vector::new(
            make_ratio(RatioParts {
                numerator: 1,
                denominator: 1,
            })?,
            make_ratio(RatioParts {
                numerator: 1,
                denominator: 1,
            })?,
        ),
        acceleration: Vector::new(
            Ratio::from_integer(0),
            make_ratio(RatioParts {
                numerator: 1,
                denominator: 100,
            })?,
        ),
    })
}

/// Builds a circular motion around the center.
fn build_circular_motion(entry: &Entry) -> Result<Motion, GenerationError> {
    let (width, height) = dimensions_as_i64(entry);
    Ok(Motion::Circular {
        centre: Point::new(
            make_ratio(RatioParts {
                numerator: width,
                denominator: 2,
            })?,
            make_ratio(RatioParts {
                numerator: height,
                denominator: 2,
            })?,
        ),
        radius: Ratio::from_integer(50),
        turns_per_frame: make_ratio(RatioParts {
            numerator: 1,
            denominator: 360,
        })?,
        phase: Ratio::from_integer(0),
    })
}

/// Builds an oscillating motion along the x-axis.
fn build_oscillating_motion(entry: &Entry) -> Result<Motion, GenerationError> {
    let (width, height) = dimensions_as_i64(entry);
    Ok(Motion::Oscillating {
        origin: Point::new(
            make_ratio(RatioParts {
                numerator: width,
                denominator: 2,
            })?,
            make_ratio(RatioParts {
                numerator: height,
                denominator: 2,
            })?,
        ),
        amplitude: Vector::new(Ratio::from_integer(40), Ratio::from_integer(0)),
        period: FrameCount::new(120)
            .map_err(|_| GenerationError::SceneConstruction("period invalid".to_owned()))?,
        phase: Ratio::from_integer(0),
    })
}

/// Builds a walking motion with random steps.
fn build_walk_motion(entry: &Entry) -> Result<Motion, GenerationError> {
    let (width, height) = dimensions_as_i64(entry);
    Ok(Motion::Walk {
        origin: Point::new(
            make_ratio(RatioParts {
                numerator: width,
                denominator: 2,
            })?,
            make_ratio(RatioParts {
                numerator: height,
                denominator: 2,
            })?,
        ),
        step: make_ratio(RatioParts {
            numerator: 1,
            denominator: 2,
        })?,
        seed: Seed::new(123),
    })
}

/// Builds motion from the entry's motion axis.
fn build_motion_from_entry(entry: &Entry) -> Result<Motion, GenerationError> {
    match entry.motion {
        crate::cover::MotionAxis::Fixed => build_fixed_motion(entry),
        crate::cover::MotionAxis::Linear => build_linear_motion(entry),
        crate::cover::MotionAxis::Ballistic => build_ballistic_motion(entry),
        crate::cover::MotionAxis::Circular => build_circular_motion(entry),
        crate::cover::MotionAxis::Oscillating => build_oscillating_motion(entry),
        crate::cover::MotionAxis::Walk => build_walk_motion(entry),
    }
}

/// Computes a camera affine transform for a frame.
fn compute_camera_affine(
    camera: &Camera,
    frame: FrameIndex,
) -> Result<synthvid_scene::Similarity, GenerationError> {
    use synthvid_scene::{position_at, rotation_at, zoom_at};

    let centre = position_at(&camera.motion, frame)
        .map_err(|_| GenerationError::RenderError("camera motion evaluation failed".to_owned()))?;
    let angle = rotation_at(&camera.rotation, frame).map_err(|_| {
        GenerationError::RenderError("camera rotation evaluation failed".to_owned())
    })?;
    let magnification = zoom_at(&camera.zoom, frame)
        .map_err(|_| GenerationError::RenderError("camera zoom evaluation failed".to_owned()))?;

    let zoom = magnification.get();
    let angle_value = angle.get();
    let turn = angle_value
        .checked_neg()
        .map_err(|_| GenerationError::RenderError("angle negation overflow".to_owned()))?;
    let cos = synthvid_scene::cos_turns(turn)
        .map_err(|_| GenerationError::RenderError("cosine evaluation failed".to_owned()))?;
    let sin = synthvid_scene::sin_turns(turn)
        .map_err(|_| GenerationError::RenderError("sine evaluation failed".to_owned()))?;
    let neg_sin = sin
        .checked_neg()
        .map_err(|_| GenerationError::RenderError("sine negation overflow".to_owned()))?;

    let a = zoom
        .checked_mul(cos)
        .map_err(|_| GenerationError::RenderError("matrix a calculation overflow".to_owned()))?;
    let b = zoom
        .checked_mul(neg_sin)
        .map_err(|_| GenerationError::RenderError("matrix b calculation overflow".to_owned()))?;
    let c = zoom
        .checked_mul(sin)
        .map_err(|_| GenerationError::RenderError("matrix c calculation overflow".to_owned()))?;

    let ax = a.checked_mul(centre.x).map_err(|_| {
        GenerationError::RenderError("translation x1 calculation overflow".to_owned())
    })?;
    let bx = b.checked_mul(centre.y).map_err(|_| {
        GenerationError::RenderError("translation x2 calculation overflow".to_owned())
    })?;
    let sum_x = ax
        .checked_add(bx)
        .map_err(|_| GenerationError::RenderError("translation x sum overflow".to_owned()))?;
    let tx = sum_x
        .checked_neg()
        .map_err(|_| GenerationError::RenderError("translation x negation overflow".to_owned()))?;

    let cx = c.checked_mul(centre.x).map_err(|_| {
        GenerationError::RenderError("translation y1 calculation overflow".to_owned())
    })?;
    let dx = a.checked_mul(centre.y).map_err(|_| {
        GenerationError::RenderError("translation y2 calculation overflow".to_owned())
    })?;
    let sum_y = cx
        .checked_add(dx)
        .map_err(|_| GenerationError::RenderError("translation y sum overflow".to_owned()))?;
    let ty = sum_y
        .checked_neg()
        .map_err(|_| GenerationError::RenderError("translation y negation overflow".to_owned()))?;

    Ok(synthvid_scene::Similarity::new(a, b, tx, ty))
}

/// Builds the track matrix from the entry's `track_matrix` axis.
fn build_track_matrix(entry: &Entry) -> Result<TrackMatrix, GenerationError> {
    use crate::cover::TrackMatrixAxis;

    match entry.track_matrix {
        TrackMatrixAxis::Identity => Ok(TrackMatrix::identity()),
        TrackMatrixAxis::Rotate90 => TrackMatrix::rotate_90(entry.dimensions)
            .ok_or_else(|| GenerationError::EncodingError("rotate 90 failed".to_owned())),
        TrackMatrixAxis::Rotate180 => TrackMatrix::rotate_180(entry.dimensions)
            .ok_or_else(|| GenerationError::EncodingError("rotate 180 failed".to_owned())),
        TrackMatrixAxis::Rotate270 => TrackMatrix::rotate_270(entry.dimensions)
            .ok_or_else(|| GenerationError::EncodingError("rotate 270 failed".to_owned())),
    }
}

/// Converts a `FrameRateValue` to a `FrameRate`.
fn frame_rate_from_axis(fr: crate::cover::FrameRateValue) -> Result<FrameRate, GenerationError> {
    use crate::cover::FrameRateValue;

    match fr {
        FrameRateValue::Thirty => FrameRate::from_fps(30)
            .map_err(|_| GenerationError::SceneConstruction("frame rate 30fps failed".to_owned())),
        FrameRateValue::Sixty => FrameRate::from_fps(60)
            .map_err(|_| GenerationError::SceneConstruction("frame rate 60fps failed".to_owned())),
        FrameRateValue::OneTwenty => FrameRate::from_fps(120)
            .map_err(|_| GenerationError::SceneConstruction("frame rate 120fps failed".to_owned())),
        FrameRateValue::TwoForty => FrameRate::from_fps(240)
            .map_err(|_| GenerationError::SceneConstruction("frame rate 240fps failed".to_owned())),
        FrameRateValue::Ntsc => {
            let ntsc_fr = Ratio::new(
                30000,
                core::num::NonZeroI64::new(1001).ok_or_else(|| {
                    GenerationError::SceneConstruction("NonZeroI64 failed".to_owned())
                })?,
            )
            .map_err(|_| GenerationError::SceneConstruction("NTSC frame rate failed".to_owned()))?;
            FrameRate::new(ntsc_fr).map_err(|_| {
                GenerationError::SceneConstruction("NTSC frame rate construction failed".to_owned())
            })
        }
        FrameRateValue::Absurd => {
            let absurd_fr = Ratio::new(
                8000,
                core::num::NonZeroI64::new(1).ok_or_else(|| {
                    GenerationError::SceneConstruction("NonZeroI64 failed".to_owned())
                })?,
            )
            .map_err(|_| {
                GenerationError::SceneConstruction("Absurd frame rate failed".to_owned())
            })?;
            FrameRate::new(absurd_fr).map_err(|_| {
                GenerationError::SceneConstruction(
                    "Absurd frame rate construction failed".to_owned(),
                )
            })
        }
    }
}
