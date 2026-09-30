//! Validation oracle for this workspace's other crates.
//!
//! This crate generates nothing. It checks what the other crates produced,
//! using external tools (`jq`, `python3`, `djpeg`) as independent oracles:
//!
//! - `synthvid-catalog`'s canonical JSON output: valid JSON as specified by
//!   RFC 8259, keys sorted by byte value, no whitespace outside string
//!   literals, and no floating-point numbers anywhere.
//! - `synthvid-encode`'s JPEG output: decodable by a real, independently
//!   implemented JPEG decoder.
//!
//! External decoders are verification only; they never produce a file.
//! Tests skip gracefully when a tool is absent and report which oracle ran
//! when one is present.

#![deny(missing_docs)]

#[cfg(test)]
pub mod support;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod jpeg_tests;
