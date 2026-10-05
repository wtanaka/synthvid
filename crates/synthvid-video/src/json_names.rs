//! JSON name validation types.
//!
//! Provides validated string types for JSON object keys and entry names.
//! Keys must match `[a-z_]+` and entry names must match `[a-z0-9-]+`.

use std::fmt;

/// Error returned when attempting to construct an invalid [`JsonKey`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum JsonKeyError {
    /// JSON key must match [a-z_]+.
    InvalidAlphabet,
}

impl fmt::Display for JsonKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAlphabet => write!(
                f,
                "JSON key must contain only lowercase letters and underscores"
            ),
        }
    }
}

impl core::error::Error for JsonKeyError {}

/// A validated string that matches `[a-z_]+` for use as a JSON object key.
///
/// The constructor ensures only lowercase letters and underscores are present,
/// making all output valid ASCII. Escape sequences cannot be written.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct JsonKey(String);

impl JsonKey {
    /// Creates a validated key matching `[a-z_]+`.
    ///
    /// # Errors
    ///
    /// Returns `Err(JsonKeyError::InvalidAlphabet)` if any character is outside the allowed set.
    pub fn new(s: impl AsRef<str>) -> Result<Self, JsonKeyError> {
        let s = s.as_ref();
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
            .then(|| Self(s.to_owned()))
            .ok_or(JsonKeyError::InvalidAlphabet)
    }

    /// Builds a key from text already known to match the alphabet.
    ///
    /// Crate-private, and reachable only through the closed set in `keys`.
    /// A public infallible constructor would be the same hole as a public
    /// raw-value setter: it would let a caller assert validity the type is
    /// supposed to establish.
    pub(crate) fn from_known(s: &'static str) -> Self {
        Self(s.to_owned())
    }

    /// Returns the validated key as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Error returned when attempting to construct an invalid [`JsonEntryName`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum JsonEntryNameError {
    /// JSON entry name must match [a-z0-9-]+.
    InvalidAlphabet,
}

impl fmt::Display for JsonEntryNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAlphabet => {
                write!(
                    f,
                    "JSON entry name must contain only lowercase letters, digits, and hyphens"
                )
            }
        }
    }
}

impl core::error::Error for JsonEntryNameError {}

/// A validated string that matches `[a-z0-9-]+` for use as a catalog entry name.
///
/// The constructor ensures only lowercase letters, digits, and hyphens are
/// present, making all output valid ASCII. Escape sequences cannot be written.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct JsonEntryName(String);

impl JsonEntryName {
    /// Creates a validated entry name matching `[a-z0-9-]+`.
    ///
    /// # Errors
    ///
    /// Returns `Err(JsonEntryNameError::InvalidAlphabet)` if any character is outside the allowed set.
    pub fn new(s: impl AsRef<str>) -> Result<Self, JsonEntryNameError> {
        let s = s.as_ref();
        (!s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
        .then(|| Self(s.to_owned()))
        .ok_or(JsonEntryNameError::InvalidAlphabet)
    }

    /// Returns the validated entry name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(s: &str) -> JsonKey {
        JsonKey::new(s).expect("test key must be valid")
    }

    fn make_entry_name(s: &str) -> JsonEntryName {
        JsonEntryName::new(s).expect("test entry name must be valid")
    }

    #[test]
    fn test_json_key_valid() {
        assert_eq!(make_key("background").as_str(), "background");
        assert_eq!(make_key("frame_count").as_str(), "frame_count");
        assert_eq!(make_key("a").as_str(), "a");
        assert_eq!(make_key("_").as_str(), "_");
    }

    #[test]
    fn test_json_key_invalid() {
        assert!(JsonKey::new("Background").is_err(), "uppercase rejected");
        assert!(JsonKey::new("frame-count").is_err(), "hyphen rejected");
        assert!(JsonKey::new("").is_err(), "empty rejected");
        assert!(JsonKey::new("123").is_err(), "digit in key rejected");
    }

    #[test]
    fn test_json_entry_name_valid() {
        assert_eq!(make_entry_name("example").as_str(), "example");
        assert_eq!(make_entry_name("test-1").as_str(), "test-1");
        assert_eq!(make_entry_name("a0").as_str(), "a0");
    }

    #[test]
    fn test_json_entry_name_invalid() {
        assert!(JsonEntryName::new("Test").is_err(), "uppercase rejected");
        assert!(
            JsonEntryName::new("test_name").is_err(),
            "underscore rejected"
        );
        assert!(JsonEntryName::new("").is_err(), "empty rejected");
    }
}
