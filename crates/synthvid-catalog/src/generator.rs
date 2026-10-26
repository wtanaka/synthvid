//! Pipeline for generating video artifacts from catalog entries.
//!
//! Converts a catalog entry to a video specification and renders it. The pipeline is
//! deterministic and produces byte-identical output for identical inputs.

use crate::cover::{Entry, FlatScene, ScaleAxis};
use crate::scene_builder::SceneBuildError;
use core::error::Error;
use core::fmt;
use synthvid_scene::{FrameCount, Scene};
use synthvid_video::{render, Camera, GeneratedEntry, GenerationError, Optics, Recorder, Timing};

/// Why a catalogue entry could not be generated.
///
/// A variant that wraps another error shows it in its message.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EntryError {
    /// The entry's scene could not be built.
    Scene(SceneBuildError),
    /// The entry's video specification could not be rendered.
    Render(GenerationError),
}

impl fmt::Display for EntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scene(error) => write!(formatter, "scene could not be built: {error}"),
            Self::Render(error) => write!(formatter, "rendering failed: {error}"),
        }
    }
}

impl Error for EntryError {}

/// Generates a manifest and media file from a catalog entry.
///
/// This is the main entry point for the generation pipeline. It:
/// 1. Converts the entry to an explicit video specification
/// 2. Renders the specification to produce manifest and media bytes
///
/// # Errors
///
/// Returns `EntryError` if the scene cannot be built or the specification cannot be rendered.
pub fn generate_entry(entry: &Entry) -> Result<GeneratedEntry, EntryError> {
    let spec = entry.to_video_spec().map_err(EntryError::Scene)?;
    render(&spec).map_err(EntryError::Render)
}

/// Builds the scene a catalog entry shows: its scale, background, and object, where the
/// background and object come from the given flat scene.
pub(crate) fn build_scene(entry: &Entry, flat: FlatScene) -> Result<Scene, SceneBuildError> {
    use crate::scene_builder::{build_background, build_object_from_entry, declared_scale};

    let background = build_background(flat.background());
    let object = build_object_from_entry(entry, flat)?;

    Ok(Scene {
        scale: match entry.scale() {
            ScaleAxis::Undeclared => None,
            ScaleAxis::Declared => Some(declared_scale()?),
        },
        background,
        objects: vec![object],
    })
}

/// Builds the recorder of a catalog entry: the medium of the entry's recording, the entry's
/// dimensions, and the entry's timing.
pub(crate) fn build_recorder(entry: &Entry) -> Result<Recorder, SceneBuildError> {
    use crate::scene_builder::frame_rate_from_axis;

    let rate = frame_rate_from_axis(entry.frame_rate())?;
    let count = FrameCount::new(entry.frame_count().get())
        .map_err(|error| SceneBuildError::FrameCount(Box::new(error)))?;
    let medium = entry.recording().medium()?;

    Ok(Recorder::new(
        medium,
        entry.dimensions(),
        Timing::new(rate, count),
    ))
}

/// Builds the camera that records a catalog entry: a fixed view at the origin with no
/// optics, and the recorder of the entry.
pub(crate) fn build_camera(entry: &Entry) -> Result<Camera, SceneBuildError> {
    use crate::scene_builder::build_view;

    Ok(Camera::new(
        build_view()?,
        Optics::Off,
        &build_recorder(entry)?,
    ))
}
