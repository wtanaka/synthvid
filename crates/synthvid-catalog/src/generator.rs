//! Pipeline for generating video artifacts from catalog entries.
//!
//! Converts a catalog entry to a video specification and renders it. The pipeline is
//! deterministic and produces byte-identical output for identical inputs.

use crate::cover::Entry;
use crate::cover::ScaleAxis;
use crate::scene_builder::SceneBuildError;
use core::error::Error;
use core::fmt;
use synthvid_scene::{FrameCount, Scene};
use synthvid_video::{render, GeneratedEntry, GenerationError};

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

/// Builds a scene from a catalog entry.
pub(crate) fn build_scene(entry: &Entry) -> Result<Scene, SceneBuildError> {
    use crate::scene_builder::{
        build_background, build_camera, build_object_from_entry, declared_scale,
        frame_rate_from_axis,
    };

    let frame_rate = frame_rate_from_axis(entry.frame_rate())?;
    let camera = build_camera()?;
    let background = build_background(entry);
    let object = build_object_from_entry(entry)?;

    Ok(Scene {
        dimensions: entry.dimensions(),
        frame_rate,
        frame_count: FrameCount::new(entry.frame_count().get())
            .map_err(SceneBuildError::FrameCount)?,
        scale: match entry.scale() {
            ScaleAxis::Undeclared => None,
            ScaleAxis::Declared => Some(declared_scale()?),
        },
        background,
        camera,
        objects: vec![object],
    })
}
