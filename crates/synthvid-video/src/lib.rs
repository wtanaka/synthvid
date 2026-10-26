//! A video specification, its rendering, and the manifest that records what was drawn.
//!
//! A [`VideoSpec`] is a `Scene` and the [`Camera`] that records it, together with the choices
//! that turn them into a file: a [`Format`] of encoding, container, and an optional defect.
//! [`render()`](fn@render) turns one into a manifest and
//! a media file. Manifests are OLPC Canonical JSON: UTF-8, no whitespace outside string
//! literals, object keys sorted by byte value, rationals as `{"den":D,"num":N}` with `den`
//! first, and no floating-point numbers anywhere. A manifest records what was drawn in each
//! frame: objects' bounding boxes, centers in screen and world space, visibility, and the
//! view transform. A manifest's digest is stable across platforms and architectures within
//! one release. The crate is pre-release (version `0.0.x`): its public API, the manifest
//! format and the digests may change in any release without notice.
//!
//! # Layout
//!
//! | Module | Owns                                                      |
//! | ------ | --------------------------------------------------------- |
//! | `video_spec` | the video specification and its encoding and container |
//! | `render` | rendering one specification (`render`)                    |
//! | `manifest` | manifest document definition                            |
//! | `manifest_values` | the exact values a manifest records              |
//! | `writer` | the canonical JSON writer manifests are built with       |
//! | `object_state` | the per-frame state of every object, for the manifest |
//! | `camera` | a camera: its view, optics, and recorder |
//! | `recorder` | what records a camera's light: its medium, output dimensions, and timing |
//! | `medium` | how a camera's recording medium records light: off, sensor, or film |
//! | `sensor` | the noise a digital sensor adds: per-sensel gain, column offsets, shot and read noise |
//! | `grain` | the grain photographic film adds: a correlated noise field per channel and frame |
//! | `optics` | how a camera's optics focus light: off, or a vignette; and the lens and aperture types |
//! | `vignette` | the radial vignette: how brightness falls off from the frame center toward the corners |
//! | `deep` | projection of depth layers onto the sensel lattice: vertices, radii, and lens shifts |
//! | `rig` | the three-dimensional camera rig: its pose, lens, and recorder, and the deep shot it looks at |
//! | `shot` | the shot a video specification records: a flat scene and camera, or a deep shot |
//! | `flat_pipeline` | rendering a flat shot's frames: the scene through optics and medium |
//! | `deep_pipeline` | rendering a deep shot's frames: sampled stage, vignette, and medium |
//! | `manifest_deep` | the manifest of a deep shot: its stage, lens, and per-frame positions |
//! | `lens_json`, `medium_json` | the lens and medium objects, shared by the flat and deep manifests |
//! | `deep_keys` | the keys a deep shot's manifest adds to those of `keys`                  |
//! | `keys`, `json_names`, `names` | closed key and name sets            |

#![forbid(unsafe_code)]

pub mod camera;
pub mod deep;
pub mod deep_keys;
pub mod deep_pipeline;
pub mod flat_pipeline;
pub mod frame_bounds;
pub mod grain;
pub mod json_names;
pub mod keys;
pub mod lens_json;
pub mod manifest;
#[cfg(test)]
mod manifest_capture_tests;
pub mod manifest_deep;
pub mod manifest_error;
pub mod manifest_values;
pub mod medium;
pub mod medium_json;
pub mod names;
pub mod object_state;
pub mod optics;
pub mod recorder;
pub mod render;
pub mod render_output;
pub mod rig;
pub mod sensor;
pub mod shot;
#[cfg(test)]
pub mod testutil;
pub mod video_spec;
pub mod vignette;
pub mod writer;

pub use camera::{Camera, Timing};
pub use deep_keys::DeepKey;
pub use grain::{apply_film_grain, grain_sigma, grain_window, GrainError, GrainWindow};
pub use keys::ManifestKey;
pub use manifest::{
    FrameObjectState, Manifest, ManifestAffine, ManifestBounds, ManifestFrame, ManifestHeader,
    ManifestObjectDecl, ManifestPoint, ManifestRatio, OnScreen,
};
pub use medium::{
    ColumnSigma, FilmGrain, GrainBlur, GrainDensity, GrainDensityError, GrainRadius,
    GrainRadiusError, GrainSeed, GrainSpread, Medium, PatternSeed, PrnuSpread, ReadSigma,
    SensorNoise, SensorNoiseError, ShotGain, TemporalSeed, Q8_8,
};
pub use names::{BackgroundName, ShapeName};
pub use optics::{
    Aperture, ApertureGrid, ApertureShape, BladeCount, BladeCountError, FStop, FocalLength, Focus,
    FocusDistance, FocusDistanceError, FrameHeight, Iris, Lens, Optics, Vignette, VignetteStrength,
    VignetteStrengthError,
};
pub use recorder::Recorder;
pub use render::{render, GeneratedEntry, GenerationError, ObjectStep, ViewStep};
pub use render_output::{ManifestJson, MediaBytes};
pub use rig::{DeepShot, DeepShotError, Pose, Rig};
pub use sensor::{
    apply_sensor_noise, combine_level, shot_sigma, Amplitude, ColumnOffset, Level, Light,
    ReadNoise, SensorError, ShotNoise,
};
pub use shot::{FlatShot, Shot};
pub use video_spec::{
    CaptureRate, Container, DisplayRotation, Encoding, Format, VideoSpec, VideoSpecError,
};
pub use vignette::{
    apply_vignette, vignette_factor, DoubledColumnOffset, DoubledExtent, DoubledRowOffset,
    FrameCenterOffsets, VignetteError, VignetteFactor,
};
pub use writer::{JsonArray, JsonEntryName, JsonInt, JsonKey, JsonObject};
