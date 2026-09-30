//! The declared corpus, its manifests, and their digests.
//!
//! This crate serializes rendered scenes to OLPC Canonical JSON manifests:
//! UTF-8, no whitespace outside string literals, object keys sorted by byte
//! value, rationals as `{"den":D,"num":N}` with `den` first, and no
//! floating-point numbers anywhere. Manifests record what was drawn in each
//! frame: objects' bounding boxes, centers in screen and world space,
//! visibility, and the camera transform. A manifest's digest is stable across
//! platforms, architectures, and time.
//!
//! # Layout
//!
//! | Module | Owns                                                   |
//! | ------ | ------------------------------------------------------ |
//! | `cover` | the pairwise covering array over the declared axes    |
//! | `generator` | rendering plus encoding one entry (`generate_entry`) |
//! | `manifest` | manifest document definition                         |
//! | `writer` | the canonical JSON writer manifests are built with    |
//! | `sha256` | the in-crate hash (`sha256`, `Sha256`, `Digest`)      |
//! | `lockfile` | the digest list (`generate_lockfile`, `parse_lockfile`, `verify`) |
//! | `keys`, `json_names`, `names` | closed key and name sets          |
//!
//! # Example
//!
//! ```rust
//! use synthvid_catalog::{generate_lockfile, parse_lockfile, sha256};
//!
//! let digest = sha256(b"").to_string();
//! let same_as_empty = digest
//!     == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
//! let text = generate_lockfile(Vec::new());
//! let parsed_len = match parse_lockfile(&text) {
//!     Ok(entries) => entries.len(),
//!     Err(_) => return,
//! };
//! (same_as_empty, parsed_len);
//! ```

#![forbid(unsafe_code)]

pub mod cover;
pub mod generator;
pub mod json_names;
pub mod keys;
pub mod lockfile;
pub mod manifest;
pub mod names;
pub mod object_state;
pub mod sha256;
pub mod writer;

pub use generator::{generate_entry, GeneratedEntry, GenerationError};
pub use keys::ManifestKey;
pub use lockfile::{
    generate_lockfile, parse_lockfile, verify, ContentLength, LockfileDifference, LockfileEntry,
    LockfileName, LockfileNameError, ParseLockfileError,
};
pub use manifest::{
    FrameObjectState, Manifest, ManifestAffine, ManifestBounds, ManifestFrame, ManifestHeader,
    ManifestObjectDecl, ManifestPoint, ManifestRatio, OnScreen,
};
pub use names::{BackgroundName, ShapeName};
pub use sha256::{sha256, DIGEST_SIZE};
pub use writer::{JsonArray, JsonEntryName, JsonKey, JsonObject};
