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

use std::collections::BTreeMap;

use synthvid_scene::Ratio;

// Re-export JSON name validation types for public API compatibility
pub use crate::json_names::{JsonEntryName, JsonKey};

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
    /// Takes a [`Ratio`], which is normalised with a strictly positive
    /// denominator by its own constructor. Taking a loose numerator and
    /// denominator instead would let a caller write a zero denominator, a
    /// negative one, or an unreduced pair, none of which the format admits.
    pub fn insert_ratio(&mut self, key: JsonKey, value: Ratio) {
        let (numer, denom) = (value.numer(), value.denom().get());
        self.insert_raw(key, format!("{{\"den\":{denom},\"num\":{numer}}}"));
    }

    /// Inserts an integer key-value pair.
    ///
    /// Takes a [`JsonKey`] and an integer value.
    pub fn insert_int(&mut self, key: JsonKey, value: i64) {
        self.insert_raw(key, value.to_string());
    }

    /// Inserts a string key-value pair.
    ///
    /// Takes a [`JsonEntryName`], whose alphabet excludes every character that
    /// would need escaping, and adds the quotes here. The previous signature
    /// took a `String` the caller was expected to have quoted already: nothing
    /// distinguished quoted from unquoted text, and nothing escaped the
    /// interior, so a value containing a quote produced invalid JSON.
    pub fn insert_string(&mut self, key: JsonKey, value: &JsonEntryName) {
        self.insert_raw(key, format!("\"{}\"", value.as_str()));
    }

    /// Inserts an already-serialised JSON fragment.
    ///
    /// Deliberately not public. Every guarantee this type makes — no floats,
    /// no escape sequences, canonical form, valid JSON — is a guarantee about
    /// what its callers can express. A public method taking an unrestricted
    /// `String` would reduce all of them to a doc comment, so this door is
    /// open only to the manifest module that builds values through the
    /// validated types above.
    pub(crate) fn insert_raw(&mut self, key: JsonKey, value: String) {
        self.entries.insert(key, JsonValue::Raw(value));
    }

    /// Inserts a nested JSON object.
    pub fn insert_object(&mut self, key: JsonKey, obj: Self) {
        self.entries.insert(key, JsonValue::Object(obj));
    }

    /// Inserts a JSON array.
    pub fn insert_array(&mut self, key: JsonKey, array: JsonArray) {
        self.entries.insert(key, JsonValue::Array(array));
    }

    /// Serializes the object directly into a buffer with sorted keys.
    ///
    /// No whitespace is emitted outside string literals and keys are sorted
    /// by UTF-8 byte value.
    fn build_into(&self, buffer: &mut String) {
        buffer.push('{');
        for (i, (key, value)) in self.entries.iter().enumerate() {
            if i > 0 {
                buffer.push(',');
            }
            buffer.push('"');
            buffer.push_str(key.as_str());
            buffer.push_str("\":");
            match value {
                JsonValue::Raw(s) => buffer.push_str(s),
                JsonValue::Object(obj) => obj.build_into(buffer),
                JsonValue::Array(arr) => arr.build_into(buffer),
            }
        }
        buffer.push('}');
    }

    /// Serializes the object to canonical JSON with sorted keys.
    ///
    /// Returns a string with no whitespace outside string literals and keys
    /// sorted by UTF-8 byte value.
    #[must_use]
    pub fn build(&self) -> String {
        let mut result = String::new();
        self.build_into(&mut result);
        result
    }
}

impl Default for JsonObject {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for JsonObject {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.build())
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

    /// Appends an already-serialised JSON fragment.
    ///
    /// Deliberately not public, for the reason given on
    /// [`JsonObject::insert_raw`]: an unrestricted `String` in the public API
    /// would make every other guarantee here unenforceable.
    pub(crate) fn push_raw(&mut self, value: String) {
        self.values.push(JsonValue::Raw(value));
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
    pub fn push_int(&mut self, value: i64) {
        self.push_raw(value.to_string());
    }

    /// Appends a string to the array.
    ///
    /// Takes a [`JsonEntryName`] and quotes it here, so that no caller can
    /// supply text needing an escape sequence. See
    /// [`JsonObject::insert_string`].
    pub fn push_string(&mut self, value: &JsonEntryName) {
        self.push_raw(format!("\"{}\"", value.as_str()));
    }

    /// Serializes the array directly into a buffer.
    ///
    /// No whitespace is emitted outside string literals.
    fn build_into(&self, buffer: &mut String) {
        buffer.push('[');
        for (i, value) in self.values.iter().enumerate() {
            if i > 0 {
                buffer.push(',');
            }
            match value {
                JsonValue::Raw(s) => buffer.push_str(s),
                JsonValue::Object(obj) => obj.build_into(buffer),
                JsonValue::Array(arr) => arr.build_into(buffer),
            }
        }
        buffer.push(']');
    }

    /// Serializes the array to canonical JSON.
    ///
    /// Returns a string with no whitespace outside string literals.
    #[must_use]
    pub fn build(&self) -> String {
        let mut result = String::new();
        self.build_into(&mut result);
        result
    }
}

impl Default for JsonArray {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for JsonArray {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.build())
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroI64;

    /// Builds an exact rational for a test fixture.
    fn make_ratio(n: i64, d: i64) -> Ratio {
        Ratio::new(n, NonZeroI64::new(d).unwrap()).ok().unwrap()
    }
    use super::*;

    fn make_key(s: &str) -> JsonKey {
        JsonKey::new(s).expect("test key must be valid")
    }

    #[test]
    fn test_object_sorted_keys() {
        let mut obj = JsonObject::new();
        obj.insert_int(make_key("z"), 1);
        obj.insert_int(make_key("a"), 2);
        obj.insert_int(make_key("m"), 3);
        let serialized = obj.build();
        // Check that 'a' comes before 'm' comes before 'z'
        let a_pos = serialized.find("\"a\"").expect("key a in output");
        let m_pos = serialized.find("\"m\"").expect("key m in output");
        let z_pos = serialized.find("\"z\"").expect("key z in output");
        assert!(a_pos < m_pos && m_pos < z_pos, "keys must be sorted");
    }

    #[test]
    fn test_object_no_whitespace() {
        let mut obj = JsonObject::new();
        obj.insert_int(make_key("a"), 1);
        let serialized = obj.build();
        // Should be {"a":1} with no spaces
        assert_eq!(serialized, r#"{"a":1}"#);
    }

    #[test]
    fn test_array_preserves_order() {
        let mut arr = JsonArray::new();
        arr.push_int(10);
        arr.push_int(20);
        arr.push_int(30);
        assert_eq!(arr.build(), "[10,20,30]");
    }

    #[test]
    fn test_ratio_serialization() {
        let mut obj = JsonObject::new();
        obj.insert_ratio(make_key("test"), make_ratio(3, 2));
        assert_eq!(obj.build(), r#"{"test":{"den":2,"num":3}}"#);
    }

    #[test]
    fn test_nested_object() {
        let mut inner = JsonObject::new();
        inner.insert_int(make_key("x"), 42);
        let mut outer = JsonObject::new();
        outer.insert_object(make_key("obj"), inner);
        assert_eq!(outer.build(), r#"{"obj":{"x":42}}"#);
    }

    /// Validates RFC 8259 JSON structure by checking for basic syntax.
    fn validate_json_structure(json_str: &str) -> bool {
        // Check if the JSON structure is valid by verifying matching braces/brackets
        // and that it doesn't contain control characters outside strings.
        let mut in_string = false;
        let mut escape_next = false;
        let mut depth: i32 = 0;

        for ch in json_str.chars() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                '{' | '[' if !in_string => {
                    depth = depth.saturating_add(1);
                }
                '}' | ']' if !in_string => {
                    depth = depth.saturating_sub(1);
                    if depth < 0 {
                        return false;
                    }
                }
                ch if !in_string && ch.is_control() && ch != '\t' && ch != '\n' && ch != '\r' => {
                    return false;
                }
                _ => {}
            }
        }

        depth == 0 && !in_string
    }

    /// Tests RFC 8259 validity of `JsonObject::build()` output.
    #[test]
    fn test_object_rfc8259_validity() {
        let mut obj = JsonObject::new();
        obj.insert_int(make_key("a"), 1);
        obj.insert_int(make_key("b"), 2);
        let json_str = obj.build();
        assert!(
            validate_json_structure(&json_str),
            "generated JSON must be RFC 8259 valid"
        );
    }

    /// Tests RFC 8259 validity of `JsonArray::build()` output.
    #[test]
    fn test_array_rfc8259_validity() {
        let mut arr = JsonArray::new();
        arr.push_int(10);
        arr.push_int(20);
        let json_str = arr.build();
        assert!(
            validate_json_structure(&json_str),
            "generated JSON must be RFC 8259 valid"
        );
    }

    /// Tests RFC 8259 validity of nested structures.
    #[test]
    fn test_nested_structure_rfc8259_validity() {
        let mut inner = JsonObject::new();
        inner.insert_int(make_key("x"), 42);
        inner.insert_int(make_key("y"), 100);

        let mut arr = JsonArray::new();
        arr.push_object(inner);
        arr.push_int(99);

        let mut outer = JsonObject::new();
        outer.insert_array(make_key("items"), arr);
        outer.insert_string(make_key("name"), &JsonEntryName::new("test").unwrap());

        let json_str = outer.build();
        assert!(
            validate_json_structure(&json_str),
            "nested structure must be RFC 8259 valid"
        );
    }

    /// Verifies that keys in JSON output are in sorted order.
    fn verify_keys_sorted(json_str: &str) -> bool {
        let mut in_string = false;
        let mut escape_next = false;
        let mut last_key: Option<String> = None;

        let mut chars = json_str.chars().peekable();
        while let Some(ch) = chars.next() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if in_string => escape_next = true,
                '"' => {
                    in_string = !in_string;
                    if in_string {
                        // Start of a potential key
                        let mut key = String::new();
                        let mut is_key = false;

                        loop {
                            match chars.next() {
                                Some('\\') => {
                                    escape_next = true;
                                    key.push('\\');
                                }
                                Some('"') => {
                                    in_string = false;
                                    // Check if next non-whitespace char is ':'
                                    while let Some(&next_ch) = chars.peek() {
                                        if next_ch == ':' {
                                            is_key = true;
                                            break;
                                        } else if !next_ch.is_whitespace() {
                                            break;
                                        }
                                        chars.next();
                                    }
                                    break;
                                }
                                Some(c) => key.push(c),
                                None => break,
                            }
                        }

                        if is_key {
                            if let Some(ref last) = last_key {
                                if key < *last {
                                    return false;
                                }
                            }
                            last_key = Some(key);
                        }
                    }
                }
                _ => {}
            }
        }

        true
    }

    /// Tests that `test_object_sorted_keys` output has sorted keys.
    #[test]
    fn test_object_sorted_keys_verified() {
        let mut obj = JsonObject::new();
        obj.insert_int(make_key("z"), 1);
        obj.insert_int(make_key("a"), 2);
        obj.insert_int(make_key("m"), 3);
        let json_str = obj.build();
        assert!(verify_keys_sorted(&json_str), "keys must be sorted");
    }

    /// Tests that `nested_object` output has sorted keys.
    #[test]
    fn test_nested_object_keys_verified() {
        let mut inner = JsonObject::new();
        inner.insert_int(make_key("x"), 42);
        let mut outer = JsonObject::new();
        outer.insert_object(make_key("obj"), inner);
        let json_str = outer.build();
        assert!(verify_keys_sorted(&json_str), "keys must be sorted");
    }

    /// Deterministic test for key ordering with fixed insertion patterns.
    #[test]
    fn test_object_deterministic_key_ordering() {
        // Test with multiple key sets in different insertion orders
        let test_cases = [("a", "b", "c"), ("z", "m", "a"), ("x", "y", "z")];

        for (k1, k2, k3) in test_cases {
            let mut obj = JsonObject::new();
            obj.insert_int(make_key(k1), 1);
            obj.insert_int(make_key(k2), 2);
            obj.insert_int(make_key(k3), 3);

            let json_str = obj.build();
            assert!(
                verify_keys_sorted(&json_str),
                "keys must be sorted for keys {k1}, {k2}, {k3}"
            );
        }
    }

    /// Tests that the same `JsonObject` built twice produces byte-identical output.
    #[test]
    fn test_determinism_same_object() {
        let mut obj1 = JsonObject::new();
        obj1.insert_int(make_key("a"), 10);
        obj1.insert_int(make_key("b"), 20);
        obj1.insert_int(make_key("c"), 30);

        let mut obj2 = JsonObject::new();
        obj2.insert_int(make_key("a"), 10);
        obj2.insert_int(make_key("b"), 20);
        obj2.insert_int(make_key("c"), 30);

        let json1 = obj1.build();
        let json2 = obj2.build();
        assert_eq!(
            json1, json2,
            "identical objects must produce identical JSON"
        );
    }

    /// Tests determinism when inserting fields via different call orders.
    #[test]
    fn test_determinism_different_insertion_order() {
        let mut obj1 = JsonObject::new();
        obj1.insert_int(make_key("z"), 1);
        obj1.insert_int(make_key("a"), 2);
        obj1.insert_int(make_key("m"), 3);

        let mut obj2 = JsonObject::new();
        obj2.insert_int(make_key("a"), 2);
        obj2.insert_int(make_key("m"), 3);
        obj2.insert_int(make_key("z"), 1);

        let json1 = obj1.build();
        let json2 = obj2.build();
        assert_eq!(
            json1, json2,
            "different insertion orders must produce identical JSON"
        );
    }

    /// Tests that no floating-point numbers are emitted.
    ///
    /// Checks the raw JSON string for telltale float markers (`.` or `e`/`E`
    /// in bare numbers), being careful not to match inside quoted strings.
    fn check_no_floats_in_json(json_str: &str) -> Result<(), String> {
        let mut in_string = false;
        let mut escape_next = false;
        let chars: Vec<char> = json_str.chars().collect();

        for (i, &ch) in chars.iter().enumerate() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                '.' | 'e' | 'E' if !in_string => {
                    // Could be a float. Check context: should be surrounded by digits and operators.
                    let before_digit = i > 0
                        && chars
                            .get(i.saturating_sub(1))
                            .is_some_and(char::is_ascii_digit);
                    if before_digit {
                        return Err(format!("Possible float marker '{ch}' at position {i}"));
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Tests that `JsonObject` output contains no floating-point numbers.
    #[test]
    fn test_no_floats_object() {
        let mut obj = JsonObject::new();
        obj.insert_int(make_key("a"), 42);
        obj.insert_ratio(make_key("ratio"), make_ratio(3, 2));
        let json_str = obj.build();
        assert!(
            check_no_floats_in_json(&json_str).is_ok(),
            "`JsonObject` output must not contain floats"
        );
    }

    /// Tests that `JsonArray` output contains no floating-point numbers.
    #[test]
    fn test_no_floats_array() {
        let mut arr = JsonArray::new();
        arr.push_int(10);
        arr.push_int(20);
        let json_str = arr.build();
        assert!(
            check_no_floats_in_json(&json_str).is_ok(),
            "`JsonArray` output must not contain floats"
        );
    }
}
