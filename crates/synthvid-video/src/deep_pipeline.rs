//! Rendering of a deep shot's frames: each frame is sampled from the shot's stage, darkened by
//! the rig's lens vignette, and then recorded on the rig's medium.
//!
//! The frames are produced without a manifest: the manifest of a deep shot is written by
//! [`manifest_deep`](crate::manifest_deep) from the shot itself. Each frame depends only on its
//! own index, so no state is kept between frames.

use crate::deep::render_stage_into;
use crate::render::{apply_medium, GenerationError};
use crate::rig::DeepShot;
use crate::vignette::apply_vignette;
use synthvid_scene::{Frame, FrameIndex};

/// Renders one frame of a deep shot, as the frame at the given index.
///
/// The frame is a fresh buffer of the rig's recorder dimensions. The stage is sampled into it,
/// the lens vignette is applied, and then the recorder's medium.
///
/// # Errors
///
/// Returns the errors of the frame buffer, the sampling, the vignette, and the medium.
fn render_deep_frame(shot: &DeepShot, index: FrameIndex) -> Result<Frame, GenerationError> {
    let recorder = shot.rig().recorder();
    let mut frame = Frame::zeroed(recorder.dimensions()).map_err(GenerationError::FrameBuffer)?;
    render_stage_into(shot, index, &mut frame)?;
    apply_vignette(&mut frame, shot.rig().lens().vignette()).map_err(GenerationError::Vignette)?;
    apply_medium(&mut frame, recorder.medium(), index)?;
    Ok(frame)
}

/// Renders every frame of a deep shot, one for each frame index of the rig's recorder.
///
/// # Errors
///
/// Returns the errors of [`render_deep_frame`] for the first frame that fails.
pub(crate) fn render_deep_shot_frames(shot: &DeepShot) -> Result<Vec<Frame>, GenerationError> {
    let count = shot.rig().recorder().timing().count().get().get();
    let mut frames = Vec::new();
    for frame_idx in 0..count {
        frames.push(render_deep_frame(shot, FrameIndex::new(frame_idx))?);
    }
    Ok(frames)
}
