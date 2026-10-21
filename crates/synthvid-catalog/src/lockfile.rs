//! Lockfile format for verifying catalog integrity.
//!
//! The lockfile maps every artifact name (media files and manifests) to its
//! SHA-256 digest and byte length. It is line-oriented, not JSON: one artifact
//! per line, name, digest, and length separated by a single space, sorted by
//! name bytewise ascending. Every line is terminated by `\n` including the last.

use crate::sha256::Digest;
use core::error::Error;
use core::fmt;
use core::num::ParseIntError;

#[path = "lockfile_fields.rs"]
mod lockfile_fields;

pub use lockfile_fields::{
    ArtifactBytes, CorpusArtifact, LockfileText, RawDigest, RawLength, RawLine, RawName,
};

/// Error returned when attempting to construct an invalid [`LockfileName`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LockfileNameError {
    /// Lockfile name must match [a-z0-9-]+\.[a-z]+.
    InvalidAlphabet,
}

impl fmt::Display for LockfileNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAlphabet => {
                write!(
                    f,
                    "lockfile name must contain only lowercase letters, digits, hyphens, and a single dot"
                )
            }
        }
    }
}

impl Error for LockfileNameError {}

/// A validated artifact name for use in a lockfile.
///
/// Holds both manifest names (e.g., `0256x0256-1-30fps-...-nodef.json`) and
/// media file names (e.g., `0256x0256-1-30fps-...-nodef-media.mp4`).
/// Must match `[a-z0-9-]+\.[a-z]+` (lowercase letters, digits, hyphens, one dot,
/// and a lowercase extension).
///
/// Ordered bytewise by the name, which is the order a lockfile lists them in.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct LockfileName(String);

impl LockfileName {
    /// Creates a validated lockfile name matching `[a-z0-9-]+\.[a-z]+`.
    ///
    /// # Errors
    ///
    /// Returns `Err(LockfileNameError::InvalidAlphabet)` if the name does not
    /// match the required format.
    pub fn new<Name>(name: Name) -> Result<Self, LockfileNameError>
    where
        Name: AsRef<str>,
    {
        let text = name.as_ref();
        if text.is_empty() {
            return Err(LockfileNameError::InvalidAlphabet);
        }

        // Must contain exactly one dot
        let dot_count = text.bytes().filter(|&byte| byte == b'.').count();
        if dot_count != 1 {
            return Err(LockfileNameError::InvalidAlphabet);
        }

        let parts: Vec<&str> = text.split('.').collect();
        if parts.len() != 2 {
            return Err(LockfileNameError::InvalidAlphabet);
        }

        let Some(stem) = parts.first() else {
            return Err(LockfileNameError::InvalidAlphabet);
        };
        let Some(ext) = parts.get(1) else {
            return Err(LockfileNameError::InvalidAlphabet);
        };

        // Stem must be [a-z0-9-]+ (not empty)
        if stem.is_empty()
            || !stem
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(LockfileNameError::InvalidAlphabet);
        }

        // Extension must be [a-z0-9]+ (not empty, can contain digits for extensions like mp4)
        if ext.is_empty()
            || !ext
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            return Err(LockfileNameError::InvalidAlphabet);
        }

        Ok(Self(text.to_owned()))
    }

    /// Returns the validated lockfile name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The byte length of an artifact in the lockfile.
///
/// A newtype wrapper around `u64` representing the byte length of any artifact,
/// including zero-length files.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ContentLength(u64);

impl ContentLength {
    /// Creates a content length from a byte count.
    ///
    /// Accepts any `u64` value, including zero.
    #[must_use]
    pub const fn new(len: u64) -> Self {
        Self(len)
    }

    /// Returns the length as a `u64`.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One entry in a lockfile: an artifact's name, digest, and byte length.
///
/// Entries are sorted by name. The lockfile format is line-oriented:
/// `name digest length\n` (single space separators).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockfileEntry {
    /// The artifact name (manifest or media file).
    pub name: LockfileName,
    /// The SHA-256 digest of the artifact's content.
    pub digest: Digest,
    /// The byte length of the artifact.
    pub length: ContentLength,
}

impl LockfileEntry {
    /// Creates a lockfile entry.
    #[must_use]
    pub const fn new(name: LockfileName, digest: Digest, length: ContentLength) -> Self {
        Self {
            name,
            digest,
            length,
        }
    }
}

/// Writes this entry in its line-oriented form: `name digest length\n`.
impl fmt::Display for LockfileEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} {} {}",
            self.name.as_str(),
            self.digest,
            self.length.get()
        )
    }
}

/// A difference found during lockfile verification.
///
/// Reports every discrepancy between a freshly generated corpus and a
/// parsed lockfile, not just the first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LockfileDifference {
    /// The artifact exists in the lockfile but not in the corpus.
    Missing {
        /// The name of the missing artifact.
        name: LockfileName,
    },
    /// The artifact exists in the corpus but not in the lockfile.
    Extra {
        /// The name of the extra artifact.
        name: LockfileName,
    },
    /// A corpus artifact name does not validate as a lockfile name.
    InvalidName {
        /// The raw, unvalidated artifact name.
        name: RawName,
    },
    /// The artifact exists in both but the digest differs.
    DigestMismatch {
        /// The name of the artifact with mismatched digest.
        name: LockfileName,
        /// The expected digest from the lockfile.
        expected: Digest,
        /// The actual digest from the corpus.
        actual: Digest,
    },
    /// The artifact exists in both but the length differs.
    LengthMismatch {
        /// The name of the artifact with mismatched length.
        name: LockfileName,
        /// The expected length from the lockfile.
        expected: ContentLength,
        /// The actual length from the corpus.
        actual: ContentLength,
    },
    /// A corpus artifact's byte length cannot be represented as a `u64`.
    LengthOverflow {
        /// The name of the artifact whose length could not be converted.
        name: LockfileName,
    },
}

impl fmt::Display for LockfileDifference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { name } => {
                write!(f, "missing artifact: {}", name.as_str())
            }
            Self::Extra { name } => {
                write!(f, "extra artifact: {}", name.as_str())
            }
            Self::InvalidName { name } => {
                write!(f, "invalid artifact name: {name}")
            }
            Self::DigestMismatch {
                name,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "digest mismatch in {}: expected {expected}, got {actual}",
                    name.as_str()
                )
            }
            Self::LengthMismatch {
                name,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "length mismatch in {}: expected {}, got {}",
                    name.as_str(),
                    expected.get(),
                    actual.get()
                )
            }
            Self::LengthOverflow { name } => {
                write!(
                    f,
                    "length overflow in {}: content too large to represent as u64",
                    name.as_str()
                )
            }
        }
    }
}

impl Error for LockfileDifference {}

/// Error returned when parsing a lockfile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseLockfileError {
    /// A line has the wrong number of fields (expected 3).
    MalformedLine {
        /// Lockfile line.
        line: RawLine,
    },
    /// An artifact name does not validate as a lockfile name.
    InvalidName {
        /// Invalid name.
        name: RawName,
    },
    /// A digest is not exactly 64 hex characters.
    InvalidDigest {
        /// Invalid digest.
        digest: RawDigest,
    },
    /// A digest contains invalid hex characters.
    InvalidDigestHex {
        /// Digest with invalid hex.
        digest: RawDigest,
    },
    /// A length field cannot be parsed as a u64.
    InvalidLength {
        /// Unparseable length.
        length: RawLength,
        /// Why the length did not parse, shown in the message.
        source: ParseIntError,
    },
}

impl fmt::Display for ParseLockfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedLine { line } => {
                write!(f, "malformed lockfile line (expected 3 fields): {line}")
            }
            Self::InvalidName { name } => {
                write!(f, "invalid artifact name: {name}")
            }
            Self::InvalidDigest { digest } => {
                write!(f, "digest must be 64 hex characters: {digest}")
            }
            Self::InvalidDigestHex { digest } => {
                write!(f, "digest contains invalid hex characters: {digest}")
            }
            Self::InvalidLength { length, source } => {
                write!(f, "length must be a valid u64: {length}: {source}")
            }
        }
    }
}

impl Error for ParseLockfileError {}

/// Generates a lockfile from a collection of entries.
///
/// Sorts entries by name, keeping the given order among entries that share
/// a name, then serializes each to the line-oriented format. Every line of
/// the result ends in `\n`.
///
/// # Example
///
/// ```rust
/// use synthvid_catalog::{
///     ContentLength, LockfileEntry, LockfileName, generate_lockfile, sha256,
/// };
///
/// let name = match LockfileName::new("b.json") {
///     Ok(name) => name,
///     Err(_) => return,
/// };
/// let entry = LockfileEntry::new(name, sha256(b"b"), ContentLength::new(1));
/// let text = generate_lockfile(vec![entry]);
/// text.0.len();
/// ```
#[must_use]
pub fn generate_lockfile(mut entries: Vec<LockfileEntry>) -> LockfileText {
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    let mut result = String::new();
    for entry in entries {
        result.push_str(&entry.to_string());
    }
    LockfileText(result)
}

/// Parses a lockfile from its line-oriented format.
///
/// # Errors
///
/// Returns an error if any line is malformed or if a name, digest, or length
/// is invalid.
pub fn parse_lockfile(content: &LockfileText) -> Result<Vec<LockfileEntry>, ParseLockfileError> {
    content
        .0
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| parse_line(LineText(line)))
        .collect()
}

/// One non-empty line of a lockfile's text.
#[derive(Copy, Clone, Debug)]
struct LineText<'text>(&'text str);

/// The digest field of a lockfile line, as written.
#[derive(Copy, Clone, Debug)]
struct DigestText<'text>(&'text str);

/// Parses one non-empty lockfile line: a name, a digest and a length,
/// separated by single spaces.
fn parse_line(line: LineText<'_>) -> Result<LockfileEntry, ParseLockfileError> {
    let LineText(text) = line;
    let parts: Vec<&str> = text.split(' ').collect();
    let [name_str, digest_str, length_str] = parts.as_slice() else {
        return Err(ParseLockfileError::MalformedLine {
            line: RawLine(text.to_owned()),
        });
    };

    let name = LockfileName::new(name_str).map_err(|LockfileNameError::InvalidAlphabet| {
        ParseLockfileError::InvalidName {
            name: RawName((*name_str).to_owned()),
        }
    })?;

    let digest = parse_digest(DigestText(digest_str))?;

    let length_u64: u64 =
        length_str
            .parse()
            .map_err(|source| ParseLockfileError::InvalidLength {
                length: RawLength((*length_str).to_owned()),
                source,
            })?;
    let length = ContentLength::new(length_u64);

    Ok(LockfileEntry::new(name, digest, length))
}

/// Parses a digest written as exactly 64 hexadecimal digits.
fn parse_digest(digest: DigestText<'_>) -> Result<Digest, ParseLockfileError> {
    let DigestText(text) = digest;
    if text.len() != 64 {
        return Err(ParseLockfileError::InvalidDigest {
            digest: RawDigest(text.to_owned()),
        });
    }
    let invalid_hex = || ParseLockfileError::InvalidDigestHex {
        digest: RawDigest(text.to_owned()),
    };
    let mut digest_bytes = [0_u8; 32];
    let mut rest = text;
    for target in &mut digest_bytes {
        let Some((pair, tail)) = rest.split_at_checked(2) else {
            return Err(invalid_hex());
        };
        let Ok(byte) = u8::from_str_radix(pair, 16) else {
            return Err(invalid_hex());
        };
        *target = byte;
        rest = tail;
    }
    Ok(Digest::new(digest_bytes))
}

/// Verifies a corpus against a lockfile.
///
/// Compares a freshly generated set of artifacts (names and content bytes)
/// against a parsed lockfile and returns all differences found, not just the
/// first one.
///
/// # Arguments
///
/// * `lockfile_entries` - The entries parsed from a lockfile
/// * `corpus` - The generated artifacts, each a name and its content
pub fn verify<Corpus>(lockfile_entries: &[LockfileEntry], corpus: Corpus) -> Vec<LockfileDifference>
where
    Corpus: IntoIterator<Item = CorpusArtifact>,
{
    use crate::sha256::sha256;
    extern crate alloc;
    use alloc::collections::BTreeMap;
    use alloc::collections::BTreeSet;

    let mut differences = Vec::new();

    // Build a map of lockfile entries by name for efficient lookup
    let lockfile_map: BTreeMap<LockfileName, &LockfileEntry> = lockfile_entries
        .iter()
        .map(|entry| (entry.name.clone(), entry))
        .collect();

    // Track which lockfile entries we've seen
    let mut seen = BTreeSet::new();

    // Process each corpus artifact
    for CorpusArtifact { name, bytes } in corpus {
        let Ok(lockfile_name) = LockfileName::new(&name.0) else {
            // Report artifacts with invalid names
            differences.push(LockfileDifference::InvalidName { name });
            continue;
        };

        let content_digest = sha256(&bytes.0);
        let Ok(content_length_u64) = u64::try_from(bytes.0.len()) else {
            differences.push(LockfileDifference::LengthOverflow {
                name: lockfile_name,
            });
            continue;
        };
        let content_length = ContentLength::new(content_length_u64);

        seen.extend([lockfile_name.clone()]);

        if let Some(lockfile_entry) = lockfile_map.get(&lockfile_name) {
            // Entry exists in lockfile, compare digest and length
            if content_digest != lockfile_entry.digest {
                differences.push(LockfileDifference::DigestMismatch {
                    name: lockfile_name.clone(),
                    expected: lockfile_entry.digest,
                    actual: content_digest,
                });
            }
            if content_length != lockfile_entry.length {
                differences.push(LockfileDifference::LengthMismatch {
                    name: lockfile_name.clone(),
                    expected: lockfile_entry.length,
                    actual: content_length,
                });
            }
        } else {
            // Entry exists in corpus but not in lockfile
            differences.push(LockfileDifference::Extra {
                name: lockfile_name,
            });
        }
    }

    // Check for entries in lockfile that are not in corpus
    for entry in lockfile_entries {
        if !seen.contains(&entry.name) {
            differences.push(LockfileDifference::Missing {
                name: entry.name.clone(),
            });
        }
    }

    differences
}

#[cfg(test)]
#[path = "lockfile_tests.rs"]
mod tests;
