//! Manifest types for serializing rendered scenes.
//!
//! A manifest describes the objects rendered, their bounding boxes, and camera
//! placement for each frame. It is serialized to canonical JSON and its digest
//! is recorded for reproducibility checking across architectures.
//!
//! Every type here keeps its fields private behind a constructor that takes
//! quantities already validated by `synthvid-scene`. A manifest is the last
//! place a value is written down before it is hashed and compared across
//! architectures, so a state that cannot occur in a scene must not become
//! expressible merely by being copied into one of these structs.

use core::num::NonZeroI64;

use crate::json_names::{JsonEntryName, JsonKey};
use crate::keys::ManifestKey;
use crate::manifest_error::ManifestError;
use crate::names::{BackgroundName, ShapeName};
use crate::render_output::ManifestJson;
use crate::writer::{JsonArray, JsonInt, JsonObject, RawJson};
use synthvid_scene::{
    CaptureCarrier, Dimensions, FrameCount, FrameIndex, FrameRates, FrameSpan, ObjectIndex, Ratio,
};

pub use crate::manifest_values::{
    ManifestAffine, ManifestBounds, ManifestPoint, ManifestRatio, OnScreen, OnScreenError,
};

/// Converts a manifest key to the writer's key type.
///
/// Infallible: the argument is one of a closed set, so there is no validation
/// left to fail and no panic path in this module.
pub(crate) fn key(manifest_key: ManifestKey) -> JsonKey {
    manifest_key.into()
}

/// The manifest's JSON string for the box that carries a distinct capture rate.
fn carrier_json(carrier: CaptureCarrier) -> RawJson {
    match carrier {
        CaptureCarrier::ProjectBox => RawJson(String::from(r#""cfps""#)),
        CaptureCarrier::QuickTimeMeta => RawJson(String::from(r#""meta""#)),
    }
}

/// What one object looked like in one frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct FrameObjectState {
    /// Bounding box in screen space.
    bbox: ManifestBounds,
    /// Centre in screen space.
    centre_screen: ManifestPoint,
    /// Centre in world units, when the scene declares a scale.
    centre_world: Option<ManifestPoint>,
    /// Which object this is, in scene declaration order.
    index: ObjectIndex,
    /// Fraction of the bounding box inside the frame.
    on_screen: OnScreen,
}

impl FrameObjectState {
    /// Records one object's state for one frame.
    #[must_use]
    pub const fn new(
        index: ObjectIndex,
        bbox: ManifestBounds,
        centre_screen: ManifestPoint,
        centre_world: Option<ManifestPoint>,
        on_screen: OnScreen,
    ) -> Self {
        Self {
            bbox,
            centre_screen,
            centre_world,
            index,
            on_screen,
        }
    }

    /// Serializes this state as a JSON object for inclusion in a manifest.
    pub(crate) fn to_json_object(self) -> JsonObject {
        let mut obj = JsonObject::new();
        obj.insert_object(key(ManifestKey::Bbox), self.bbox.to_json_object());
        obj.insert_object(
            key(ManifestKey::CentreScreen),
            self.centre_screen.to_json_object(),
        );
        if let Some(world) = self.centre_world {
            obj.insert_object(key(ManifestKey::CentreWorld), world.to_json_object());
        }
        obj.insert_int(
            key(ManifestKey::Index),
            JsonInt(i64::from(self.index.get())),
        );
        obj.insert_ratio(key(ManifestKey::OnScreen), self.on_screen.ratio());
        obj
    }
}

/// One frame of a manifest.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestFrame {
    /// Scene-to-screen transform for this frame.
    camera: ManifestAffine,
    /// Objects on screen this frame, in scene declaration order.
    objects: Vec<FrameObjectState>,
}

impl ManifestFrame {
    /// Records one frame.
    #[must_use]
    pub const fn new(camera: &ManifestAffine, objects: Vec<FrameObjectState>) -> Self {
        Self {
            camera: *camera,
            objects,
        }
    }

    /// Serializes this frame as a JSON object with the given frame index for inclusion in a manifest.
    pub(crate) fn to_json_object(&self, frame_index: FrameIndex) -> JsonObject {
        let mut obj = JsonObject::new();
        obj.insert_object(key(ManifestKey::Camera), self.camera.to_json_object());
        obj.insert_int(
            key(ManifestKey::Index),
            JsonInt(i64::from(frame_index.get())),
        );
        let mut array = JsonArray::new();
        for state in &self.objects {
            array.push_object(state.to_json_object());
        }
        obj.insert_array(key(ManifestKey::Objects), array);
        obj
    }
}

/// An invariant object declaration.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestObjectDecl {
    /// Index in the scene declaration.
    index: ObjectIndex,
    /// Which shape this object is.
    shape: ShapeName,
    /// Frames this object is drawn in.
    ///
    /// A [`FrameSpan`], not a pair of integers. Its constructor rejects an
    /// empty span and an inverted one; two public `u32` fields made both
    /// representable again at the only point where the value is written down
    /// permanently.
    visible: FrameSpan,
}

impl ManifestObjectDecl {
    /// Declares one object.
    #[must_use]
    pub const fn new(index: ObjectIndex, shape: ShapeName, visible: FrameSpan) -> Self {
        Self {
            index,
            shape,
            visible,
        }
    }

    /// Serializes this object declaration as a JSON object for inclusion in a manifest.
    pub(crate) fn to_json_object(self) -> JsonObject {
        let mut obj = JsonObject::new();
        obj.insert_int(
            key(ManifestKey::Index),
            JsonInt(i64::from(self.index.get())),
        );
        obj.insert_encoded(
            key(ManifestKey::Shape),
            RawJson(format!(r#""{}""#, self.shape)),
        );
        let mut visible = JsonObject::new();
        visible.insert_int(
            key(ManifestKey::End),
            JsonInt(i64::from(self.visible.end().get())),
        );
        visible.insert_int(
            key(ManifestKey::Start),
            JsonInt(i64::from(self.visible.start().get())),
        );
        obj.insert_object(key(ManifestKey::Visible), visible);
        obj
    }
}

/// The scene-level facts a manifest records, independent of any frame.
///
/// Grouped into their own type because positional parameters are a hazard:
/// it is a list a caller can misorder, and a signature no reader can check
/// at a glance. Frame count is derived from the manifest's frames rather than
/// stored here, making divergence unrepresentable.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestHeader {
    /// Which backdrop the scene declared.
    background: BackgroundName,
    /// Frame dimensions in pixels.
    dimensions: Dimensions,
    /// Playback frame rate, and the distinct capture rate when one is present.
    frame_rates: FrameRates,
    /// Catalog entry name.
    name: JsonEntryName,
    /// Scene units per world unit, if declared.
    scale: Option<ManifestRatio>,
}

impl ManifestHeader {
    /// Records the scene-level facts.
    ///
    /// Every parameter is a type whose constructor has already refused the
    /// states a manifest must never contain: a zero dimension, a frame rate
    /// that is not an exact positive rational, a backdrop outside its closed set.
    /// Frame count is derived from the manifest's frames at construction, not
    /// passed here.
    #[must_use]
    pub const fn new(
        name: JsonEntryName,
        dimensions: Dimensions,
        frame_rates: FrameRates,
        background: BackgroundName,
        scale: Option<Ratio>,
    ) -> Self {
        Self {
            background,
            dimensions,
            frame_rates,
            name,
            scale: match scale {
                Some(ratio) => Some(ManifestRatio::from_ratio(ratio)),
                None => None,
            },
        }
    }
}

/// A complete manifest for a rendered scene.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Manifest {
    /// Scene-level facts.
    header: ManifestHeader,
    /// Frames in order. Never empty; an empty vector cannot form a valid frame count.
    frames: Vec<ManifestFrame>,
    /// Invariant object declarations.
    objects: Vec<ManifestObjectDecl>,
    /// Number of frames, derived from `frames.len()` to ensure consistency.
    frame_count: FrameCount,
}

impl Manifest {
    /// Assembles a manifest from a header, its objects, and its frames.
    ///
    /// `FrameCount` wraps `NonZeroU32`, so the frame vector must be non-empty
    /// and must fit in a `u32`. Every other invariant is guaranteed by the
    /// parameter types.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::Empty`] if `frames` is empty, and
    /// [`ManifestError::TooMany`] if it holds more than `u32::MAX` frames.
    pub fn new(
        header: ManifestHeader,
        objects: Vec<ManifestObjectDecl>,
        frames: Vec<ManifestFrame>,
    ) -> Result<Self, ManifestError> {
        let count = u32::try_from(frames.len())?;
        let frame_count = FrameCount::new(count)?;
        Ok(Self {
            header,
            frames,
            objects,
            frame_count,
        })
    }

    /// Serializes this manifest to canonical JSON.
    ///
    /// The output contains no whitespace outside string literals, keys are
    /// sorted by UTF-8 byte value, and every byte is stable across platforms.
    #[must_use]
    pub fn to_canonical_json(&self) -> ManifestJson {
        let mut root = JsonObject::new();

        root.insert_encoded(
            key(ManifestKey::Background),
            RawJson(format!(r#""{}""#, self.header.background)),
        );

        let mut dimensions = JsonObject::new();
        dimensions.insert_int(
            key(ManifestKey::Height),
            JsonInt(NonZeroI64::from(self.header.dimensions.height.get()).get()),
        );
        dimensions.insert_int(
            key(ManifestKey::Width),
            JsonInt(NonZeroI64::from(self.header.dimensions.width.get()).get()),
        );
        root.insert_object(key(ManifestKey::Dimensions), dimensions);

        root.insert_int(
            key(ManifestKey::FrameCount),
            JsonInt(NonZeroI64::from(self.frame_count.get()).get()),
        );
        root.insert_ratio(
            key(ManifestKey::FrameRate),
            self.header.frame_rates.playback().get(),
        );
        if let Some(capture) = self.header.frame_rates.distinct_capture() {
            root.insert_ratio(key(ManifestKey::CaptureRate), capture.rate.get());
            root.insert_encoded(
                key(ManifestKey::CaptureCarrier),
                carrier_json(capture.carrier),
            );
        }

        let mut frames_array = JsonArray::new();
        // frame_count is derived from frames.len(), so zipping pairs each frame with its index.
        for (index, frame) in (0..self.frame_count.get().get()).zip(&self.frames) {
            frames_array.push_object(frame.to_json_object(FrameIndex::new(index)));
        }
        root.insert_array(key(ManifestKey::Frames), frames_array);

        root.insert_string(key(ManifestKey::Name), &self.header.name);

        let mut objects_array = JsonArray::new();
        for obj_decl in &self.objects {
            objects_array.push_object(obj_decl.to_json_object());
        }
        root.insert_array(key(ManifestKey::Objects), objects_array);

        if let Some(scale) = self.header.scale {
            root.insert_ratio(key(ManifestKey::Scale), scale.ratio());
        }

        root.insert_int(key(ManifestKey::SchemaVersion), JsonInt(1));

        ManifestJson(root.to_string())
    }
}

#[cfg(test)]
mod tests;
