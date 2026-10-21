//! Unvalidated text and bytes a lockfile is read from or checked against, one
//! type per quantity.
//!
//! Each type holds text exactly as it was read, before any check has run on
//! it, so the error that reports it cannot be confused with another field.

use core::fmt;

/// An artifact name as read from a corpus or a lockfile, before validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawName(pub String);

impl fmt::Display for RawName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A line of a lockfile as read, before it is split into fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawLine(pub String);

impl fmt::Display for RawLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A digest field as read from a lockfile, before it is decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawDigest(pub String);

impl fmt::Display for RawDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A length field as read from a lockfile, before it is parsed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawLength(pub String);

impl fmt::Display for RawLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The text of a lockfile: one `name digest length` line per artifact.
///
/// Held as read or as generated; [`parse_lockfile`](super::parse_lockfile)
/// is what checks it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockfileText(pub String);

/// The content of one artifact of a corpus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactBytes(pub Vec<u8>);

/// One artifact of a corpus, as handed to [`verify`](super::verify): its name
/// before validation, and its content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusArtifact {
    /// The artifact's name, not yet checked against the lockfile alphabet.
    pub name: RawName,
    /// The artifact's content.
    pub bytes: ArtifactBytes,
}
