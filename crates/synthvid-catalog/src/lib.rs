//! The declared corpus and its digests.
//!
//! The catalogue is a pairwise covering array over declared axes. [`cover::Entry`] is one
//! point of it; [`cover::Entry::to_video_spec`] turns an entry into a `synthvid_video::VideoSpec`,
//! and [`generate_entry`] renders it. The lockfile lists the SHA-256 digest and length of every
//! file the catalogue produces, so any change to the corpus shows up as a change to that file.
//! The crate is pre-release (version `0.0.x`): its public API, the catalogue and
//! the digests may change in any release without notice.
//!
//! # Layout
//!
//! | Module | Owns                                                   |
//! | ------ | ------------------------------------------------------ |
//! | `cover` | the pairwise covering array over the declared axes    |
//! | `scene_builder` | the scene each entry describes                |
//! | `generator` | generating one entry (`generate_entry`)           |
//! | `sha256` | the in-crate hash (`sha256`, `Sha256`, `Digest`)      |
//! | `lockfile` | the digest list (`generate_lockfile`, `parse_lockfile`, `verify`) |
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
pub mod lockfile;
pub mod scene_builder;
pub mod sha256;

pub use generator::{generate_entry, EntryError};
pub use lockfile::{
    generate_lockfile, parse_lockfile, verify, ArtifactBytes, ContentLength, CorpusArtifact,
    LockfileDifference, LockfileEntry, LockfileName, LockfileNameError, LockfileText,
    ParseLockfileError, RawName,
};
pub use scene_builder::SceneBuildError;
pub use sha256::{sha256, DIGEST_SIZE};

#[cfg(test)]
mod capture_tests;
#[cfg(test)]
mod object_state_tests;
#[cfg(test)]
mod video_spec_tests;
