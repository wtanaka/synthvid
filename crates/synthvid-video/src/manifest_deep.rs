//! The manifest of a deep shot: its backdrop, layers, lens, and, on every frame, the position of
//! the camera and of each visible layer.
//!
//! A deep shot is not projected, so its manifest records no bounding boxes and no screen
//! positions. Each frame lists the camera's roll and track position, then every layer whose
//! visible span contains that frame, at its track position. Layers behind the camera are listed
//! too. The frame count is the recorder's timing count, so it is never stored separately from the
//! frames.

use core::error::Error;
use core::fmt;
use core::num::{NonZeroI64, TryFromIntError};

use synthvid_scene::scene::Membership;
use synthvid_scene::{
    rotation_at, track_at, FrameIndex, ObjectIndex, Overflow, Position, Stage, TrackError,
};

use crate::deep_keys::DeepKey;
use crate::json_names::{JsonEntryName, JsonKey};
use crate::keys::ManifestKey;
use crate::lens_json::lens_object;
use crate::manifest::key;
use crate::medium_json::medium_object;
use crate::names::ProjectionName;
use crate::render_output::ManifestJson;
use crate::rig::DeepShot;
use crate::writer::{JsonArray, JsonInt, JsonObject};

/// The objects that describe the stage's colors and the declaration of each layer.
mod stage_json;

use self::stage_json::{layer_object, rgb_object};

/// Error returned when a deep shot's manifest cannot be written.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DeepManifestError {
    /// A track cannot be evaluated at a frame: its position leaves the world, or exact arithmetic
    /// cannot represent it.
    TrackPosition(TrackError),
    /// The camera's roll cannot be evaluated exactly at a frame.
    Roll(Overflow),
    /// The focal length cannot be written as an exact ratio.
    FocalLength(Overflow),
    /// A layer's place in the stage does not fit an object index.
    LayerIndex(TryFromIntError),
}

impl fmt::Display for DeepManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TrackPosition(error) => {
                write!(
                    formatter,
                    "a track position could not be evaluated: {error}"
                )
            }
            Self::Roll(error) => {
                write!(formatter, "the camera roll could not be evaluated: {error}")
            }
            Self::FocalLength(error) => {
                write!(formatter, "the focal length could not be written: {error}")
            }
            Self::LayerIndex(error) => {
                write!(
                    formatter,
                    "a layer index does not fit an object index: {error}"
                )
            }
        }
    }
}

impl Error for DeepManifestError {}

/// Builds the canonical JSON manifest of a deep shot, under the given entry name.
///
/// # Errors
///
/// Returns the error of the first track, roll, focal length, or layer index that cannot be
/// written.
pub(crate) fn deep_manifest_json(
    name: &JsonEntryName,
    shot: &DeepShot,
) -> Result<ManifestJson, DeepManifestError> {
    let recorder = shot.rig().recorder();
    let dimensions = recorder.dimensions();
    let stage = shot.stage();
    let count = recorder.timing().count().get().get();

    let mut frames = JsonArray::new();
    for frame_idx in 0..count {
        frames.push_object(frame_object(shot, FrameIndex::new(frame_idx))?);
    }

    let mut root = JsonObject::new();
    root.insert_object(JsonKey::from(DeepKey::Backdrop), rgb_object(stage.backdrop));
    let mut dimensions_obj = JsonObject::new();
    dimensions_obj.insert_int(
        key(ManifestKey::Height),
        JsonInt(NonZeroI64::from(dimensions.height.get()).get()),
    );
    dimensions_obj.insert_int(
        key(ManifestKey::Width),
        JsonInt(NonZeroI64::from(dimensions.width.get()).get()),
    );
    root.insert_object(key(ManifestKey::Dimensions), dimensions_obj);
    root.insert_int(key(ManifestKey::FrameCount), JsonInt(i64::from(count)));
    root.insert_ratio(key(ManifestKey::FrameRate), recorder.timing().rate().get());
    root.insert_array(key(ManifestKey::Frames), frames);
    if let Some(medium) = medium_object(recorder.medium()) {
        root.insert_object(key(ManifestKey::Medium), medium);
    }
    root.insert_string(key(ManifestKey::Name), name);
    root.insert_array(key(ManifestKey::Objects), layer_declarations(stage)?);
    let optics = lens_object(shot.rig().lens()).map_err(DeepManifestError::FocalLength)?;
    root.insert_object(key(ManifestKey::Optics), optics);
    root.insert_int(
        JsonKey::from(DeepKey::PixelSamples),
        JsonInt(NonZeroI64::from(shot.pixels().per_axis()).get()),
    );
    root.insert_name(
        JsonKey::from(DeepKey::Projection),
        ProjectionName::Perspective,
    );
    root.insert_int(key(ManifestKey::SchemaVersion), JsonInt(2));
    Ok(ManifestJson(root.to_string()))
}

/// Declares every layer of the stage, in declaration order.
fn layer_declarations(stage: &Stage) -> Result<JsonArray, DeepManifestError> {
    let mut array = JsonArray::new();
    for (position, layer) in stage.layers.iter().enumerate() {
        let index = u32::try_from(position).map_err(DeepManifestError::LayerIndex)?;
        array.push_object(layer_object(ObjectIndex::new(index), layer));
    }
    Ok(array)
}

/// Writes one frame: the camera's roll and track position, then every visible layer's position.
fn frame_object(shot: &DeepShot, frame: FrameIndex) -> Result<JsonObject, DeepManifestError> {
    let pose = shot.rig().pose();
    let roll = rotation_at(&pose.roll(), frame).map_err(DeepManifestError::Roll)?;
    let camera_position =
        track_at(&pose.track(), frame).map_err(DeepManifestError::TrackPosition)?;

    let mut camera = JsonObject::new();
    camera.insert_ratio(JsonKey::from(DeepKey::Roll), roll.get());
    insert_position(&mut camera, camera_position);

    let mut objects = JsonArray::new();
    for (position, layer) in shot.stage().layers.iter().enumerate() {
        match layer.visible.contains(frame) {
            Membership::Inside => {
                let index = u32::try_from(position).map_err(DeepManifestError::LayerIndex)?;
                let layer_position =
                    track_at(&layer.track, frame).map_err(DeepManifestError::TrackPosition)?;
                let mut object = JsonObject::new();
                object.insert_int(key(ManifestKey::Index), JsonInt(i64::from(index)));
                insert_position(&mut object, layer_position);
                objects.push_object(object);
            }
            Membership::Outside => {}
        }
    }

    let mut obj = JsonObject::new();
    obj.insert_object(key(ManifestKey::Camera), camera);
    obj.insert_int(key(ManifestKey::Index), JsonInt(i64::from(frame.get())));
    obj.insert_array(key(ManifestKey::Objects), objects);
    Ok(obj)
}

/// Writes a position's across, down, and depth offsets as the `x`, `y`, and `z` keys.
fn insert_position(obj: &mut JsonObject, position: Position) {
    obj.insert_int(
        key(ManifestKey::PointX),
        JsonInt(i64::from(position.across.get().0)),
    );
    obj.insert_int(
        key(ManifestKey::PointY),
        JsonInt(i64::from(position.down.get().0)),
    );
    obj.insert_int(
        JsonKey::from(DeepKey::PointZ),
        JsonInt(i64::from(position.depth.get().0)),
    );
}

#[cfg(test)]
mod tests;
