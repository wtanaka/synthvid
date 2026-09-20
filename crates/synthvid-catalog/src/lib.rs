//! The declared corpus, its manifests, and their digests.
//!
//! This crate serializes rendered scenes to OLPC Canonical JSON manifests.
//! Manifests record what was drawn in each frame: objects' bounding boxes,
//! centres in screen and world space, visibility, and the camera transform.
//! A manifest's digest is stable across platforms, architectures, and time,
//! making it ground truth for reproducibility.

#![forbid(unsafe_code)]

pub mod keys;
pub mod manifest;
pub mod names;
pub mod writer;

pub use keys::ManifestKey;
pub use manifest::{
    FrameObjectState, Manifest, ManifestAffine, ManifestBounds, ManifestFrame, ManifestHeader,
    ManifestObjectDecl, ManifestPoint, ManifestRatio, OnScreen,
};
pub use names::{BackgroundName, ShapeName};
pub use writer::{JsonArray, JsonEntryName, JsonKey, JsonObject};
