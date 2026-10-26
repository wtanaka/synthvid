//! The manifest objects that describe a camera's recording medium: a sensor's noise or film's grain.
//!
//! An ideal medium ([`Medium::Off`]) writes nothing, so a camera made of it serializes without a
//! medium key. Every other medium writes one object. A seed is a `u64`, which the manifest's
//! integer type cannot hold, so it is written as a JSON string of its decimal digits.
//!
//! The flat camera manifest and the deep shot manifest both write their medium through these
//! functions, so a medium is spelled the same way in each.

use synthvid_scene::Seed;

use crate::keys::ManifestKey;
use crate::manifest::key;
use crate::medium::{FilmGrain, Medium, SensorNoise, Q8_8};
use crate::names::KindName;
use crate::writer::{JsonInt, JsonObject, RawJson};

/// Writes a seed as a JSON string of its decimal digits.
fn seed_json(seed: Seed) -> RawJson {
    RawJson(format!(r#""{}""#, seed.0))
}

/// Writes a Q8.8 value as its raw integer, in which 256 is 1.0.
pub(crate) fn q8_8_int(raw: Q8_8) -> JsonInt {
    JsonInt(i64::from(raw.0))
}

/// The object that describes the medium, or `None` when the medium is ideal.
#[must_use]
pub(crate) fn medium_object(medium: Medium) -> Option<JsonObject> {
    match medium {
        Medium::Off => None,
        Medium::Sensor(noise) => Some(sensor_object(noise)),
        Medium::Film(grain) => Some(film_object(grain)),
    }
}

/// Describes a digital sensor's noise terms and both of its seeds.
fn sensor_object(noise: SensorNoise) -> JsonObject {
    let mut obj = JsonObject::new();
    obj.insert_int(key(ManifestKey::Column), q8_8_int(noise.column().0));
    obj.insert_name(key(ManifestKey::Kind), KindName::Sensor);
    obj.insert_encoded(
        key(ManifestKey::PatternSeed),
        seed_json(noise.pattern_seed().get()),
    );
    obj.insert_int(key(ManifestKey::Prnu), JsonInt(i64::from(noise.prnu().0)));
    obj.insert_int(key(ManifestKey::Read), q8_8_int(noise.read().0));
    obj.insert_int(key(ManifestKey::Shot), q8_8_int(noise.shot().0));
    obj.insert_encoded(
        key(ManifestKey::TemporalSeed),
        seed_json(noise.temporal_seed().get()),
    );
    obj
}

/// Describes photographic film's grain.
fn film_object(grain: FilmGrain) -> JsonObject {
    let mut obj = JsonObject::new();
    obj.insert_int(key(ManifestKey::Blur), q8_8_int(grain.blur().0));
    obj.insert_int(key(ManifestKey::Density), q8_8_int(grain.density().get()));
    obj.insert_name(key(ManifestKey::Kind), KindName::Film);
    obj.insert_ratio(key(ManifestKey::Radius), grain.radius().get());
    obj.insert_encoded(key(ManifestKey::Seed), seed_json(grain.seed().get()));
    obj.insert_int(key(ManifestKey::Spread), q8_8_int(grain.spread().0));
    obj
}
