//! Canonical JSON writer for manifests.
//!
//! OLPC Canonical JSON dialect: UTF-8 with no whitespace outside string
//! literals, object members sorted by UTF-8 key bytes, rational numbers as
//! `{"den":D,"num":N}`, and no floating-point numbers anywhere.
//!
//! Strings are restricted at the type level: only keys matching `[a-z_]+` and
//! entry names matching `[a-z0-9-]+` can be written. This makes escape
//! sequences unreachable, so the output is valid both as canonical JSON and
//! as ordinary JSON (OLPC permits unescaped control characters, which would
//! violate RFC 8259).

extern crate alloc;
use alloc::collections::BTreeMap;
use core::fmt;

use synthvid_scene::Ratio;

// Re-export JSON name validation types for public API compatibility
pub use crate::json_names::{JsonEntryName, JsonKey};

/// An integer as a JSON document records it.
///
/// Every `i64` is a JSON integer this writer can emit, so the value is held
/// openly: there is no state to refuse. The type names what the number is
/// for at the point it enters a document.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct JsonInt(pub i64);

/// A fragment of JSON text already in canonical form.
///
/// Crate-private: only code in this crate that builds the fragment from
/// validated values may hand one to [`JsonObject::insert_encoded`].
#[derive(Clone, Debug)]
pub(crate) struct RawJson(pub(crate) String);

/// Represents a value stored in a JSON object or array.
///
/// By storing objects and arrays directly instead of as pre-serialized strings,
/// we reduce allocations: each value is serialized exactly once into a single
/// growing buffer, rather than serializing nested objects to strings and then
/// copying those strings when building the parent.
#[derive(Debug)]
enum JsonValue {
    /// An already-serialized JSON fragment (string, number, ratio, etc.).
    Raw(String),
    /// A JSON object that will be serialized inline.
    Object(JsonObject),
    /// A JSON array that will be serialized inline.
    Array(JsonArray),
}
/// A builder for canonical JSON objects.
///
/// Keys are sorted by UTF-8 byte value and written in that order. Only valid
/// keys can be added. No whitespace is emitted outside string literals.
/// Objects and arrays are stored without pre-serialization, reducing allocations.
#[derive(Debug)]
pub struct JsonObject {
    /// Entries keyed by name. A map rather than a list of pairs because a
    /// duplicate key is forbidden by the format: storing pairs makes the
    /// invalid state representable and leaves nothing but a runtime check
    /// between it and the output. Iterating a `BTreeMap` is also already in
    /// byte order, so no sort is needed at build time.
    /// Values are stored as `JsonValue` to avoid pre-serialization of nested
    /// objects and arrays.
    entries: BTreeMap<JsonKey, JsonValue>,
}

impl JsonObject {
    /// Creates an empty JSON object.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Inserts a rational key-value pair.
    ///
    /// Takes a [`Ratio`], which is normalized with a strictly positive
    /// denominator by its own constructor. Taking a loose numerator and
    /// denominator instead would let a caller write a zero denominator, a
    /// negative one, or an unreduced pair, none of which the format admits.
    pub fn insert_ratio(&mut self, key: JsonKey, value: Ratio) {
        let (numer, denom) = (value.numer().0, value.denom().get());
        self.insert_encoded(key, RawJson(format!("{{\"den\":{denom},\"num\":{numer}}}")));
    }

    /// Inserts an integer key-value pair.
    ///
    /// Takes a [`JsonKey`] and an integer value.
    pub fn insert_int(&mut self, key: JsonKey, value: JsonInt) {
        self.insert_encoded(key, RawJson(value.0.to_string()));
    }

    /// Inserts a string key-value pair.
    ///
    /// Takes a [`JsonEntryName`], whose alphabet excludes every character that
    /// would need escaping, and adds the quotes here. The previous signature
    /// took a `String` the caller was expected to have quoted already: nothing
    /// distinguished quoted from unquoted text, and nothing escaped the
    /// interior, so a value containing a quote produced invalid JSON.
    pub fn insert_string(&mut self, key: JsonKey, value: &JsonEntryName) {
        self.insert_encoded(key, RawJson(format!("\"{}\"", value.as_str())));
    }

    /// Inserts an already-serialized JSON fragment.
    ///
    /// Deliberately not public. Every guarantee this type makes — no floats,
    /// no escape sequences, canonical form, valid JSON — is a guarantee about
    /// what its callers can express. A public method taking an unrestricted
    /// `String` would reduce all of them to a doc comment, so this door is
    /// open only to the manifest module that builds values through the
    /// validated types above.
    pub(crate) fn insert_encoded(&mut self, key: JsonKey, value: RawJson) {
        self.entries.extend([(key, JsonValue::Raw(value.0))]);
    }

    /// Inserts a nested JSON object.
    pub fn insert_object(&mut self, key: JsonKey, obj: Self) {
        self.entries.extend([(key, JsonValue::Object(obj))]);
    }

    /// Inserts a JSON array.
    pub fn insert_array(&mut self, key: JsonKey, array: JsonArray) {
        self.entries.extend([(key, JsonValue::Array(array))]);
    }

    /// Serializes the object directly into a buffer with sorted keys.
    ///
    /// No whitespace is emitted outside string literals and keys are sorted
    /// by UTF-8 byte value.
    fn write_into(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("{")?;
        for (position, (key, value)) in self.entries.iter().enumerate() {
            if position > 0 {
                formatter.write_str(",")?;
            }
            formatter.write_str("\"")?;
            formatter.write_str(key.as_str())?;
            formatter.write_str("\":")?;
            match value {
                JsonValue::Raw(text) => formatter.write_str(text)?,
                JsonValue::Object(obj) => obj.write_into(formatter)?,
                JsonValue::Array(arr) => arr.write_into(formatter)?,
            }
        }
        formatter.write_str("}")
    }
}

impl Default for JsonObject {
    fn default() -> Self {
        Self::new()
    }
}

/// Writes the object as canonical JSON with sorted keys.
///
/// No whitespace is emitted outside string literals, and keys are sorted by
/// UTF-8 byte value.
///
/// # Example
///
/// ```rust
/// use synthvid_video::writer::{JsonInt, JsonKey, JsonObject};
///
/// let mut obj = JsonObject::new();
/// match (JsonKey::new("b"), JsonKey::new("a")) {
///     (Ok(b), Ok(a)) => {
///         obj.insert_int(b, JsonInt(2));
///         obj.insert_int(a, JsonInt(1));
///     }
///     _ => return,
/// }
/// obj.to_string();
/// ```
impl fmt::Display for JsonObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_into(f)
    }
}

/// A builder for canonical JSON arrays.
///
/// Values are emitted in insertion order. No whitespace is emitted outside
/// string literals.
#[derive(Debug)]
pub struct JsonArray {
    /// Values stored without pre-serialization. This reduces allocations when
    /// nested objects and arrays are stored; each is serialized exactly once
    /// into the final buffer.
    values: Vec<JsonValue>,
}

impl JsonArray {
    /// Creates an empty JSON array.
    #[must_use]
    pub const fn new() -> Self {
        Self { values: Vec::new() }
    }

    /// Appends a JSON object to the array.
    pub fn push_object(&mut self, obj: JsonObject) {
        self.values.push(JsonValue::Object(obj));
    }

    /// Appends a JSON array to the array.
    pub fn push_array(&mut self, arr: Self) {
        self.values.push(JsonValue::Array(arr));
    }

    /// Appends an integer to the array.
    pub fn push_int(&mut self, value: JsonInt) {
        self.values.push(JsonValue::Raw(value.0.to_string()));
    }

    /// Appends a string to the array.
    ///
    /// Takes a [`JsonEntryName`] and quotes it here, so that no caller can
    /// supply text needing an escape sequence. See
    /// [`JsonObject::insert_string`].
    pub fn push_string(&mut self, value: &JsonEntryName) {
        self.values
            .push(JsonValue::Raw(format!("\"{}\"", value.as_str())));
    }

    /// Writes the array into a formatter.
    ///
    /// No whitespace is emitted outside string literals.
    fn write_into(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[")?;
        for (position, value) in self.values.iter().enumerate() {
            if position > 0 {
                formatter.write_str(",")?;
            }
            match value {
                JsonValue::Raw(text) => formatter.write_str(text)?,
                JsonValue::Object(obj) => obj.write_into(formatter)?,
                JsonValue::Array(arr) => arr.write_into(formatter)?,
            }
        }
        formatter.write_str("]")
    }
}

impl Default for JsonArray {
    fn default() -> Self {
        Self::new()
    }
}

/// Writes the array as canonical JSON, with no whitespace outside string
/// literals.
impl fmt::Display for JsonArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_into(f)
    }
}

#[cfg(test)]
mod tests;
