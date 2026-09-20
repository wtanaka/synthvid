//! Validation and testing oracle for synthvid-catalog's canonical JSON writer.
//!
//! This crate uses external tools (`jq` and `python3`) as independent oracles to
//! validate that synthvid-catalog's canonical JSON output is:
//! - RFC 8259 compliant
//! - Properly canonicalized (sorted keys, no whitespace)
//! - Free of floating-point numbers
//!
//! Tests skip gracefully if external tools are not available, but report clearly
//! when they are used and what validation they performed.

#![deny(missing_docs)]

#[cfg(test)]
mod tests;
