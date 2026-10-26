//! The manifest objects that describe a deep shot's stage: its colors and the declaration of
//! each layer.
//!
//! A layer is declared once, in the stage's order, with its fill, its outline, and the span of
//! frames in which it is visible. Its position is written frame by frame by the deep manifest,
//! not here.

use core::num::NonZeroI64;

use synthvid_scene::{Cutout, Extent, FrameSpan, Layer, LocalPoint, ObjectIndex, Rgb8};

use crate::deep_keys::DeepKey;
use crate::json_names::JsonKey;
use crate::keys::ManifestKey;
use crate::manifest::key;
use crate::names::CutoutName;
use crate::writer::{JsonArray, JsonInt, JsonObject};

/// Describes a color as its blue, green, and red channels.
pub(super) fn rgb_object(color: Rgb8) -> JsonObject {
    let mut obj = JsonObject::new();
    obj.insert_int(
        JsonKey::from(DeepKey::Blue),
        JsonInt(i64::from(color.blue.0)),
    );
    obj.insert_int(
        JsonKey::from(DeepKey::Green),
        JsonInt(i64::from(color.green.0)),
    );
    obj.insert_int(JsonKey::from(DeepKey::Red), JsonInt(i64::from(color.red.0)));
    obj
}

/// Declares one layer: its index, its fill, its outline, and its visible span.
pub(super) fn layer_object(index: ObjectIndex, layer: &Layer) -> JsonObject {
    let mut obj = JsonObject::new();
    obj.insert_object(JsonKey::from(DeepKey::Fill), rgb_object(layer.fill));
    obj.insert_int(key(ManifestKey::Index), JsonInt(i64::from(index.get())));
    match &layer.cutout {
        Cutout::Disc { radius } => {
            obj.insert_int(key(ManifestKey::Radius), extent_int(*radius));
            obj.insert_name(key(ManifestKey::Shape), CutoutName::Disc);
        }
        Cutout::Rect {
            half_width,
            half_height,
        } => {
            obj.insert_int(JsonKey::from(DeepKey::HalfWidth), extent_int(*half_width));
            obj.insert_int(JsonKey::from(DeepKey::HalfHeight), extent_int(*half_height));
            obj.insert_name(key(ManifestKey::Shape), CutoutName::Rect);
        }
        Cutout::Polygon(outline) => {
            let mut vertices = JsonArray::new();
            for vertex in outline.vertices() {
                vertices.push_object(point_object(*vertex));
            }
            obj.insert_array(JsonKey::from(DeepKey::Vertices), vertices);
            obj.insert_name(key(ManifestKey::Shape), CutoutName::Polygon);
        }
    }
    obj.insert_object(key(ManifestKey::Visible), visible_object(layer.visible));
    obj
}

/// Writes a length in world units as an integer.
fn extent_int(extent: Extent) -> JsonInt {
    JsonInt(NonZeroI64::from(extent.get()).get())
}

/// Describes a vertex offset from the layer's position as its across and down offsets.
fn point_object(point: LocalPoint) -> JsonObject {
    let mut obj = JsonObject::new();
    obj.insert_int(
        key(ManifestKey::PointX),
        JsonInt(i64::from(point.across.get().0)),
    );
    obj.insert_int(
        key(ManifestKey::PointY),
        JsonInt(i64::from(point.down.get().0)),
    );
    obj
}

/// Describes the frames in which a layer is visible, as a half-open span.
fn visible_object(span: FrameSpan) -> JsonObject {
    let mut obj = JsonObject::new();
    obj.insert_int(key(ManifestKey::End), JsonInt(i64::from(span.end().get())));
    obj.insert_int(
        key(ManifestKey::Start),
        JsonInt(i64::from(span.start().get())),
    );
    obj
}
