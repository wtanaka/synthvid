//! The manifest objects that describe a flat camera's optics.
//!
//! An ideal medium ([`Medium::Off`]) and ideal optics ([`Optics::Off`]) write nothing, so a camera
//! made of them serializes exactly as it did before these objects existed. Every other medium and
//! optics writes one object under its own key. The medium object is written by the shared
//! [`medium_json`](crate::medium_json) module, which the deep shot manifest uses too.

use crate::keys::ManifestKey;
use crate::manifest::key;
use crate::medium_json::q8_8_int;
use crate::names::KindName;
use crate::optics::{Optics, VignetteStrength};
use crate::writer::JsonObject;

pub(super) use crate::medium_json::medium_object;

/// The object that describes the optics, or `None` when the optics are ideal.
#[must_use]
pub(super) fn optics_object(optics: Optics) -> Option<JsonObject> {
    match optics {
        Optics::Off => None,
        Optics::Vignette(strength) => Some(vignette_object(strength)),
    }
}

/// Describes a flat camera's vignette by the strength of its falloff toward the corners.
fn vignette_object(strength: VignetteStrength) -> JsonObject {
    let mut obj = JsonObject::new();
    obj.insert_int(key(ManifestKey::Strength), q8_8_int(strength.get()));
    obj.insert_name(key(ManifestKey::Kind), KindName::Vignette);
    obj
}
