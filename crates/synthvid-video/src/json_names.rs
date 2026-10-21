//! JSON name validation types.
//!
//! Provides validated string types for JSON object keys and entry names.
//! Keys must match `[a-z_]+` and entry names must match `[a-z0-9-]+`.

use core::error::Error;
use core::fmt;

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

impl Error for JsonKeyError {}

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
    pub fn new<Name>(name: Name) -> Result<Self, JsonKeyError>
    where
        Name: AsRef<str>,
    {
        let text = name.as_ref();
        (!text.is_empty()
            && text
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_'))
        .then(|| Self(text.to_owned()))
        .ok_or(JsonKeyError::InvalidAlphabet)
    }

    /// Builds a key from text already known to match the alphabet.
    ///
    /// Crate-private, and reachable only through the closed set in `keys`.
    /// A public infallible constructor would be the same hole as a public
    /// raw-value setter: it would let a caller assert validity the type is
    /// supposed to establish.
    pub(crate) fn from_known(text: &'static str) -> Self {
        Self(text.to_owned())
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

impl Error for JsonEntryNameError {}

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
    pub fn new<Name>(name: Name) -> Result<Self, JsonEntryNameError>
    where
        Name: AsRef<str>,
    {
        let text = name.as_ref();
        (!text.is_empty()
            && text
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'))
        .then(|| Self(text.to_owned()))
        .ok_or(JsonEntryNameError::InvalidAlphabet)
    }

    /// Returns the validated entry name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests;
