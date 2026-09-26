//! Lockfile format for verifying catalog integrity.
//!
//! The lockfile maps every artifact name (media files and manifests) to its
//! SHA-256 digest and byte length. It is line-oriented, not JSON: one artifact
//! per line, name, digest, and length separated by a single space, sorted by
//! name bytewise ascending. Every line is terminated by `\n` including the last.

use crate::sha256::Digest;
use core::cmp::Ordering;
use core::fmt;

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

impl core::error::Error for LockfileNameError {}

/// A validated artifact name for use in a lockfile.
///
/// Holds both manifest names (e.g., `0256x0256-1-30fps-...-nodef.json`) and
/// media file names (e.g., `0256x0256-1-30fps-...-nodef-media.mp4`).
/// Must match `[a-z0-9-]+\.[a-z]+` (lowercase letters, digits, hyphens, one dot,
/// and a lowercase extension).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct LockfileName(String);

impl LockfileName {
    /// Creates a validated lockfile name matching `[a-z0-9-]+\.[a-z]+`.
    ///
    /// # Errors
    ///
    /// Returns `Err(LockfileNameError::InvalidAlphabet)` if the name does not
    /// match the required format.
    pub fn new(s: impl AsRef<str>) -> Result<Self, LockfileNameError> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(LockfileNameError::InvalidAlphabet);
        }

        // Must contain exactly one dot
        let dot_count = s.bytes().filter(|&b| b == b'.').count();
        if dot_count != 1 {
            return Err(LockfileNameError::InvalidAlphabet);
        }

        let parts: Vec<&str> = s.split('.').collect();
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
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(LockfileNameError::InvalidAlphabet);
        }

        // Extension must be [a-z0-9]+ (not empty, can contain digits for extensions like mp4)
        if ext.is_empty()
            || !ext
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        {
            return Err(LockfileNameError::InvalidAlphabet);
        }

        Ok(Self(s.to_owned()))
    }

    /// Returns the validated lockfile name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Ord for LockfileName {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for LockfileName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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

    /// Serializes this entry to its line-oriented form: `name digest length\n`.
    #[must_use]
    pub fn to_line(&self) -> String {
        format!(
            "{} {} {}\n",
            self.name.as_str(),
            self.digest,
            self.length.get()
        )
    }
}

impl Ord for LockfileEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for LockfileEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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
        name: String,
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
                    "digest mismatch in {}: expected {}, got {}",
                    name.as_str(),
                    expected,
                    actual
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

impl core::error::Error for LockfileDifference {}

/// Error returned when parsing a lockfile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseLockfileError {
    /// A line has the wrong number of fields (expected 3).
    MalformedLine {
        /// Lockfile line.
        line: String,
    },
    /// An artifact name does not validate as a lockfile name.
    InvalidName {
        /// Invalid name.
        name: String,
    },
    /// A digest is not exactly 64 hex characters.
    InvalidDigest {
        /// Invalid digest.
        digest: String,
    },
    /// A digest contains invalid hex characters.
    InvalidDigestHex {
        /// Digest with invalid hex.
        digest: String,
    },
    /// A length field cannot be parsed as a u64.
    InvalidLength {
        /// Unparseable length.
        length: String,
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
            Self::InvalidLength { length } => {
                write!(f, "length must be a valid u64: {length}")
            }
        }
    }
}

impl core::error::Error for ParseLockfileError {}

/// Generates a lockfile from a collection of entries.
///
/// Sorts entries by name, then serializes each to the line-oriented format.
/// The result is a single string with each line ending in `\n`.
#[must_use]
pub fn generate_lockfile(mut entries: Vec<LockfileEntry>) -> String {
    entries.sort();
    let mut result = String::new();
    for entry in entries {
        result.push_str(&entry.to_line());
    }
    result
}

/// Parses a lockfile from its line-oriented format.
///
/// # Errors
///
/// Returns an error if any line is malformed or if a name, digest, or length
/// is invalid.
pub fn parse_lockfile(content: &str) -> Result<Vec<LockfileEntry>, ParseLockfileError> {
    let mut entries = Vec::new();

    for line in content.lines() {
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 3 {
            return Err(ParseLockfileError::MalformedLine {
                line: line.to_owned(),
            });
        }

        let Some(&name_str) = parts.first() else {
            return Err(ParseLockfileError::MalformedLine {
                line: line.to_owned(),
            });
        };
        let Some(&digest_str) = parts.get(1) else {
            return Err(ParseLockfileError::MalformedLine {
                line: line.to_owned(),
            });
        };
        let Some(&length_str) = parts.get(2) else {
            return Err(ParseLockfileError::MalformedLine {
                line: line.to_owned(),
            });
        };

        let name = LockfileName::new(name_str).map_err(|_| ParseLockfileError::InvalidName {
            name: name_str.to_owned(),
        })?;

        // Parse digest (must be 64 hex characters)
        if digest_str.len() != 64 {
            return Err(ParseLockfileError::InvalidDigest {
                digest: digest_str.to_owned(),
            });
        }

        let mut digest_bytes = [0u8; 32];
        for (i, byte_pair) in digest_str.as_bytes().chunks(2).enumerate() {
            if i >= 32 {
                break;
            }
            if let Ok(s) = core::str::from_utf8(byte_pair) {
                if let Ok(b) = u8::from_str_radix(s, 16) {
                    if let Some(target) = digest_bytes.get_mut(i) {
                        *target = b;
                    }
                } else {
                    return Err(ParseLockfileError::InvalidDigestHex {
                        digest: digest_str.to_owned(),
                    });
                }
            } else {
                return Err(ParseLockfileError::InvalidDigestHex {
                    digest: digest_str.to_owned(),
                });
            }
        }
        let digest = Digest::new(digest_bytes);

        // Parse length
        let length_u64: u64 =
            length_str
                .parse()
                .map_err(|_| ParseLockfileError::InvalidLength {
                    length: length_str.to_owned(),
                })?;
        let length = ContentLength::new(length_u64);

        entries.push(LockfileEntry::new(name, digest, length));
    }

    Ok(entries)
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
/// * `corpus` - Iterator of (name, content bytes) tuples representing the
///   generated artifacts
pub fn verify(
    lockfile_entries: &[LockfileEntry],
    corpus: impl IntoIterator<Item = (String, Vec<u8>)>,
) -> Vec<LockfileDifference> {
    use crate::sha256::sha256;
    use std::collections::BTreeMap;
    use std::collections::BTreeSet;

    let mut differences = Vec::new();

    // Build a map of lockfile entries by name for efficient lookup
    let mut lockfile_map: BTreeMap<LockfileName, &LockfileEntry> = BTreeMap::new();
    for entry in lockfile_entries {
        lockfile_map.insert(entry.name.clone(), entry);
    }

    // Track which lockfile entries we've seen
    let mut seen = BTreeSet::new();

    // Process each corpus artifact
    for (name, content) in corpus {
        let Ok(lockfile_name) = LockfileName::new(&name) else {
            // Report artifacts with invalid names
            differences.push(LockfileDifference::InvalidName { name });
            continue;
        };

        let content_digest = sha256(&content);
        let Ok(content_length_u64) = u64::try_from(content.len()) else {
            differences.push(LockfileDifference::LengthOverflow {
                name: lockfile_name,
            });
            continue;
        };
        let content_length = ContentLength::new(content_length_u64);

        seen.insert(lockfile_name.clone());

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
