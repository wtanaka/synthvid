//! A video specification, its rendering, and the manifest that records what was drawn.
//!
//! A [`VideoSpec`] is a `Scene` together with the choices that turn it into a file: its
//! encoding, its container, and an optional defect. [`render()`](fn@render) turns one into a manifest and
//! a media file. Manifests are OLPC Canonical JSON: UTF-8, no whitespace outside string
//! literals, object keys sorted by byte value, rationals as `{"den":D,"num":N}` with `den`
//! first, and no floating-point numbers anywhere. A manifest records what was drawn in each
//! frame: objects' bounding boxes, centers in screen and world space, visibility, and the
//! camera transform. A manifest's digest is stable across platforms and architectures within
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
//! | `keys`, `json_names`, `names` | closed key and name sets            |

#![forbid(unsafe_code)]

pub mod frame_bounds;
pub mod json_names;
pub mod keys;
pub mod manifest;
#[cfg(test)]
mod manifest_capture_tests;
pub mod manifest_error;
pub mod manifest_values;
pub mod names;
pub mod object_state;
pub mod render;
pub mod render_output;
#[cfg(test)]
pub mod testutil;
pub mod video_spec;
pub mod writer;

pub use keys::ManifestKey;
pub use manifest::{
    FrameObjectState, Manifest, ManifestAffine, ManifestBounds, ManifestFrame, ManifestHeader,
    ManifestObjectDecl, ManifestPoint, ManifestRatio, OnScreen,
};
pub use names::{BackgroundName, ShapeName};
pub use render::{render, CameraStep, GeneratedEntry, GenerationError, ObjectStep};
pub use render_output::{ManifestJson, MediaBytes};
pub use video_spec::{
    CaptureRate, Container, DisplayRotation, Encoding, VideoSpec, VideoSpecError,
};
pub use writer::{JsonArray, JsonEntryName, JsonInt, JsonKey, JsonObject};
