//! Rendering of a flat shot's frames: the scene drawn through the camera's optics and medium,
//! with the view and the objects of each frame recorded for the manifest.
//!
//! `render_flat_frames` renders the frames in index order. Each frame depends only on its own
//! index, so no state is kept between frames.

use crate::manifest::{ManifestAffine, ManifestFrame};
use crate::object_state::frame_objects;
use crate::optics::{Optics, Vignette};
use crate::render::{apply_medium, encode_frame, GenerationError, ViewStep};
use crate::render_output::EncodedFrames;
use crate::shot::FlatShot;
use crate::video_spec::Encoding;
use crate::vignette::apply_vignette;
use synthvid_scene::{render_into, Frame, FrameIndex, Overflow, View};

/// Renders one frame of a flat shot into the frame buffer: the scene, then the camera's optics,
/// then its medium.
///
/// # Errors
///
/// Returns the errors of the scene renderer, the vignette, and the medium.
pub(crate) fn render_flat_frame(
    flat: &FlatShot,
    frame_buffer: &mut Frame,
    frame_index: FrameIndex,
) -> Result<(), GenerationError> {
    let camera = flat.camera();
    render_into(flat.scene(), camera.view(), frame_index, frame_buffer)
        .map_err(GenerationError::SceneFrameRender)?;
    match camera.optics() {
        Optics::Off => {}
        Optics::Vignette(strength) => {
            apply_vignette(frame_buffer, Vignette::Radial(strength))
                .map_err(GenerationError::Vignette)?;
        }
    }
    apply_medium(frame_buffer, camera.medium(), frame_index)
}

/// Renders every frame of a flat shot, returning the encoded frames and the manifest frames.
///
/// # Errors
///
/// Returns the errors of [`render_flat_frame`], the view arithmetic, the affine conversion, and
/// the object state of a frame.
pub(crate) fn render_flat_frames(
    flat: &FlatShot,
    encoding: Encoding,
) -> Result<(EncodedFrames, Vec<ManifestFrame>), GenerationError> {
    let scene = flat.scene();
    let camera = flat.camera();
    let mut manifest_frames = Vec::new();
    let mut encoded_frames = Vec::new();

    let mut frame_buffer =
        Frame::zeroed(camera.dimensions()).map_err(GenerationError::FrameBuffer)?;

    for frame_idx in 0..camera.timing().count().get().get() {
        let frame_index = FrameIndex::new(frame_idx);

        render_flat_frame(flat, &mut frame_buffer, frame_index)?;
        encoded_frames.push(encode_frame(encoding, &frame_buffer));

        let view_affine = compute_view_affine(camera.view(), frame_index)?;
        let view_manifest = ManifestAffine::from_affine(
            &view_affine
                .to_affine()
                .map_err(|Overflow| GenerationError::AffineConversion)?,
        );

        let objects = frame_objects(scene, camera, frame_index)?;
        let manifest_frame = ManifestFrame::new(&view_manifest, objects);
        manifest_frames.push(manifest_frame);
    }

    Ok((EncodedFrames(encoded_frames), manifest_frames))
}

/// Computes a view affine transform for a frame.
fn compute_view_affine(
    view: &View,
    frame: FrameIndex,
) -> Result<synthvid_scene::Similarity, GenerationError> {
    use synthvid_scene::{position_at, rotation_at, zoom_at};

    let center = position_at(&view.motion, frame)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::Center))?;
    let angle = rotation_at(&view.rotation, frame)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::Rotation))?;
    let magnification =
        zoom_at(&view.zoom, frame).map_err(|Overflow| GenerationError::ViewMath(ViewStep::Zoom))?;

    let zoom = magnification.get();
    let angle_value = angle.get();
    let turn = angle_value
        .checked_neg()
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::NegateAngle))?;
    let cos = synthvid_scene::cos_turns(turn)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::Cosine))?;
    let sin = synthvid_scene::sin_turns(turn)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::Sine))?;
    let neg_sin = sin
        .checked_neg()
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::LinearTerms))?;

    let scale_cos = zoom
        .checked_mul(cos)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::LinearTerms))?;
    let scale_neg_sin = zoom
        .checked_mul(neg_sin)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::LinearTerms))?;
    let scale_sin = zoom
        .checked_mul(sin)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::LinearTerms))?;

    let ax = scale_cos
        .checked_mul(center.across)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::TranslationX))?;
    let bx = scale_neg_sin
        .checked_mul(center.down)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::TranslationX))?;
    let sum_x = ax
        .checked_add(bx)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::TranslationX))?;
    let tx = sum_x
        .checked_neg()
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::TranslationX))?;

    let cx = scale_sin
        .checked_mul(center.across)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::TranslationY))?;
    let dx = scale_cos
        .checked_mul(center.down)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::TranslationY))?;
    let sum_y = cx
        .checked_add(dx)
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::TranslationY))?;
    let ty = sum_y
        .checked_neg()
        .map_err(|Overflow| GenerationError::ViewMath(ViewStep::TranslationY))?;

    Ok(synthvid_scene::Similarity {
        xx: scale_cos,
        xy: scale_neg_sin,
        tx,
        ty,
    })
}
