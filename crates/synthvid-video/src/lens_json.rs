//! The manifest object that describes a lens: its aperture, focal length, and vignette.
//!
//! A lens is written as one object, the optics of a deep shot, with its kind set to `lens`. Its
//! focal length is an exact ratio of frame heights. An iris writes its stop, outline, focus, frame
//! height, and sample grid, and a pinhole writes only its kind.

use core::num::NonZeroI64;

use synthvid_scene::Overflow;

use crate::deep_keys::DeepKey;
use crate::json_names::JsonKey;
use crate::keys::ManifestKey;
use crate::manifest::key;
use crate::medium_json::q8_8_int;
use crate::names::{
    ApertureKindName, ApertureName, ApertureShapeKindName, FocusName, KindName, VignetteKindName,
};
use crate::optics::{Aperture, ApertureGrid, ApertureShape, FStop, Focus, Iris, Lens, Vignette};
use crate::writer::{JsonInt, JsonObject};

/// The object that describes a lens.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the focal length cannot be written as an exact ratio.
pub(crate) fn lens_object(lens: Lens) -> Result<JsonObject, Overflow> {
    let mut obj = JsonObject::new();
    obj.insert_object(key(ManifestKey::Aperture), aperture_object(lens.aperture()));
    obj.insert_ratio(key(ManifestKey::FocalLength), lens.focal_length().ratio()?);
    obj.insert_name(key(ManifestKey::Kind), KindName::Lens);
    obj.insert_object(key(ManifestKey::Vignette), vignette_object(lens.vignette()));
    Ok(obj)
}

/// Describes the aperture: a pinhole, or an iris with its stop, outline, and focus.
fn aperture_object(aperture: Aperture) -> JsonObject {
    match aperture {
        Aperture::Pinhole => {
            let mut obj = JsonObject::new();
            obj.insert_name(key(ManifestKey::Kind), ApertureKindName::Pinhole);
            obj
        }
        Aperture::Iris(iris) => iris_object(iris),
    }
}

/// Describes an iris by its focus, frame height, kind, sample grid, outline, and stop.
fn iris_object(iris: Iris) -> JsonObject {
    let mut obj = JsonObject::new();
    match iris.focus() {
        Focus::Infinity => obj.insert_name(key(ManifestKey::Focus), FocusName::Infinity),
        Focus::At(distance) => obj.insert_int(
            key(ManifestKey::Focus),
            JsonInt(NonZeroI64::from(distance.get()).get()),
        ),
    }
    obj.insert_int(
        JsonKey::from(DeepKey::FrameHeight),
        JsonInt(NonZeroI64::from(iris.frame_height().0).get()),
    );
    obj.insert_name(key(ManifestKey::Kind), ApertureKindName::Iris);
    obj.insert_int(JsonKey::from(DeepKey::Samples), grid_count(iris.samples()));
    obj.insert_object(key(ManifestKey::Shape), shape_object(iris.shape()));
    obj.insert_name(JsonKey::from(DeepKey::Stop), stop_name(iris.stop()));
    obj
}

/// Describes an iris outline: a round iris, or a bladed one with its blade count.
fn shape_object(shape: ApertureShape) -> JsonObject {
    let mut obj = JsonObject::new();
    match shape {
        ApertureShape::Disc => {
            obj.insert_name(key(ManifestKey::Kind), ApertureShapeKindName::Disc);
        }
        ApertureShape::Polygon(blades) => {
            obj.insert_int(
                key(ManifestKey::Blades),
                JsonInt(NonZeroI64::from(blades.get()).get()),
            );
            obj.insert_name(key(ManifestKey::Kind), ApertureShapeKindName::Polygon);
        }
    }
    obj
}

/// Describes the lens vignette: none, or a vignette with its strength.
fn vignette_object(vignette: Vignette) -> JsonObject {
    let mut obj = JsonObject::new();
    match vignette {
        Vignette::None => {
            obj.insert_name(key(ManifestKey::Kind), VignetteKindName::None);
        }
        Vignette::Radial(strength) => {
            obj.insert_int(key(ManifestKey::Strength), q8_8_int(strength.get()));
            obj.insert_name(key(ManifestKey::Kind), KindName::Vignette);
        }
    }
    obj
}

/// The number of lens samples along each axis of an iris's grid, as the manifest writes it.
const fn grid_count(grid: ApertureGrid) -> JsonInt {
    match grid {
        ApertureGrid::Two => JsonInt(2),
        ApertureGrid::Four => JsonInt(4),
        ApertureGrid::Eight => JsonInt(8),
    }
}

/// The name of an f-number as the manifest writes it.
const fn stop_name(stop: FStop) -> ApertureName {
    match stop {
        FStop::F1_4 => ApertureName::F1_4,
        FStop::F2 => ApertureName::F2,
        FStop::F2_8 => ApertureName::F2_8,
        FStop::F4 => ApertureName::F4,
        FStop::F5_6 => ApertureName::F5_6,
        FStop::F8 => ApertureName::F8,
        FStop::F11 => ApertureName::F11,
        FStop::F16 => ApertureName::F16,
        FStop::F22 => ApertureName::F22,
    }
}
