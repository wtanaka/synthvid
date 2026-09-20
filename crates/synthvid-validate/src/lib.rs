//! Validation and testing oracle for this workspace's other crates.
//!
//! This crate uses external tools (`jq`, `python3`, `djpeg`) as independent
//! oracles to validate:
//! - synthvid-catalog's canonical JSON output: RFC 8259 compliant, properly
//!   canonicalized (sorted keys, no whitespace), and free of floating-point
//!   numbers.
//! - synthvid-encode's JPEG output: decodable by a real, independently
//!   implemented JPEG decoder.
//!
//! Tests skip gracefully if external tools are not available, but report clearly
//! when they are used and what validation they performed.

#![deny(missing_docs)]

#[cfg(test)]
pub mod support;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod jpeg_tests;
