use super::*;
use crate::testutil::Frac;
use core::iter::Peekable;
use core::str::Chars;
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// JSON text handed to one of the structural checks below.
#[derive(Copy, Clone, Debug)]
struct Text<'json>(&'json str);

fn check_object_sorted_keys() -> Result<(), Fail> {
    let mut obj = JsonObject::new();
    obj.insert_int(JsonKey::new("z").or_fail()?, JsonInt(1));
    obj.insert_int(JsonKey::new("a").or_fail()?, JsonInt(2));
    obj.insert_int(JsonKey::new("m").or_fail()?, JsonInt(3));
    let serialized = obj.to_string();
    // Check that 'a' comes before 'm' comes before 'z'
    let a_pos = serialized.find("\"a\"").or_fail()?;
    let m_pos = serialized.find("\"m\"").or_fail()?;
    let z_pos = serialized.find("\"z\"").or_fail()?;
    ensure!(a_pos < m_pos && m_pos < z_pos, "keys must be sorted");
    Ok(())
}

#[test]
fn object_sorted_keys() {
    assert_eq!(check_object_sorted_keys(), Ok(()));
}

fn check_object_no_whitespace() -> Result<(), Fail> {
    let mut obj = JsonObject::new();
    obj.insert_int(JsonKey::new("a").or_fail()?, JsonInt(1));
    // Should be {"a":1} with no spaces
    ensure_eq!(obj.to_string(), r#"{"a":1}"#);
    Ok(())
}

#[test]
fn object_no_whitespace() {
    assert_eq!(check_object_no_whitespace(), Ok(()));
}

#[test]
fn array_preserves_order() {
    let mut arr = JsonArray::new();
    arr.push_int(JsonInt(10));
    arr.push_int(JsonInt(20));
    arr.push_int(JsonInt(30));
    assert_eq!(arr.to_string(), "[10,20,30]");
}

fn check_ratio_serialization() -> Result<(), Fail> {
    let mut obj = JsonObject::new();
    obj.insert_ratio(JsonKey::new("test").or_fail()?, Frac(3, 2).ratio()?);
    ensure_eq!(obj.to_string(), r#"{"test":{"den":2,"num":3}}"#);
    Ok(())
}

#[test]
fn ratio_serialization() {
    assert_eq!(check_ratio_serialization(), Ok(()));
}

fn check_nested_object() -> Result<(), Fail> {
    let mut inner = JsonObject::new();
    inner.insert_int(JsonKey::new("x").or_fail()?, JsonInt(42));
    let mut outer = JsonObject::new();
    outer.insert_object(JsonKey::new("obj").or_fail()?, inner);
    ensure_eq!(outer.to_string(), r#"{"obj":{"x":42}}"#);
    Ok(())
}

#[test]
fn nested_object() {
    assert_eq!(check_nested_object(), Ok(()));
}

/// Validates RFC 8259 JSON structure by checking for basic syntax.
///
/// Fails unless braces and brackets match and no control character
/// appears outside a string.
fn validate_json_structure(json: Text<'_>) -> Result<(), Fail> {
    let Text(json_str) = json;
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
                depth = depth.checked_add(1).or_fail()?;
            }
            '}' | ']' if !in_string => {
                depth = depth.checked_sub(1).or_fail()?;
                ensure!(depth >= 0_i32, "a closing bracket has no opening one");
            }
            _ if !in_string && ch.is_control() && ch != '\t' && ch != '\n' && ch != '\r' => {
                return Err(Fail::here(format_args!(
                    "control character {ch:?} outside a string"
                )));
            }
            _ => {}
        }
    }

    ensure!(depth == 0_i32 && !in_string, "unbalanced JSON");
    Ok(())
}

fn check_object_rfc8259_validity() -> Result<(), Fail> {
    let mut obj = JsonObject::new();
    obj.insert_int(JsonKey::new("a").or_fail()?, JsonInt(1));
    obj.insert_int(JsonKey::new("b").or_fail()?, JsonInt(2));
    validate_json_structure(Text(&obj.to_string()))
}

/// Tests RFC 8259 validity of `JsonObject` output.
#[test]
fn object_rfc8259_validity() {
    assert_eq!(check_object_rfc8259_validity(), Ok(()));
}

/// Tests RFC 8259 validity of `JsonArray` output.
#[test]
fn array_rfc8259_validity() {
    let mut arr = JsonArray::new();
    arr.push_int(JsonInt(10));
    arr.push_int(JsonInt(20));
    let json_str = arr.to_string();
    assert_eq!(validate_json_structure(Text(&json_str)), Ok(()));
}

fn check_nested_structure_rfc8259_validity() -> Result<(), Fail> {
    let mut inner = JsonObject::new();
    inner.insert_int(JsonKey::new("x").or_fail()?, JsonInt(42));
    inner.insert_int(JsonKey::new("y").or_fail()?, JsonInt(100));

    let mut arr = JsonArray::new();
    arr.push_object(inner);
    arr.push_int(JsonInt(99));

    let mut outer = JsonObject::new();
    outer.insert_array(JsonKey::new("items").or_fail()?, arr);
    outer.insert_string(
        JsonKey::new("name").or_fail()?,
        &JsonEntryName::new("test").or_fail()?,
    );

    validate_json_structure(Text(&outer.to_string()))
}

/// Tests RFC 8259 validity of nested structures.
#[test]
fn nested_structure_rfc8259_validity() {
    assert_eq!(check_nested_structure_rfc8259_validity(), Ok(()));
}

/// The text of an object key, escapes left as written.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct KeyText(String);

/// Reads from just after an opening quote through the closing quote, then
/// looks past any whitespace for the colon that marks a key.
///
/// Returns the literal's text, escapes left as written, when it is a key.
#[must_use]
fn read_key(chars: &mut Peekable<Chars<'_>>) -> Option<KeyText> {
    let mut text = String::new();
    let mut escaped = false;
    for ch in chars.by_ref() {
        match (escaped, ch) {
            (false, '"') => break,
            (false, '\\') => {
                escaped = true;
                text.push(ch);
            }
            _ => {
                escaped = false;
                text.push(ch);
            }
        }
    }
    while chars.next_if(|next| next.is_whitespace()).is_some() {}
    (chars.peek() == Some(&':')).then_some(KeyText(text))
}

/// Verifies that keys in JSON output are in sorted order.
///
/// Fails at the first key that sorts before the key preceding it.
fn verify_keys_sorted(json: Text<'_>) -> Result<(), Fail> {
    let Text(json_str) = json;
    let mut last_key: Option<KeyText> = None;

    let mut chars = json_str.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            if let Some(key) = read_key(&mut chars) {
                if let Some(last) = &last_key {
                    ensure!(key >= *last, "key {key:?} sorts before {last:?}");
                }
                last_key = Some(key);
            }
        }
    }

    Ok(())
}

fn check_object_sorted_keys_verified() -> Result<(), Fail> {
    let mut obj = JsonObject::new();
    obj.insert_int(JsonKey::new("z").or_fail()?, JsonInt(1));
    obj.insert_int(JsonKey::new("a").or_fail()?, JsonInt(2));
    obj.insert_int(JsonKey::new("m").or_fail()?, JsonInt(3));
    verify_keys_sorted(Text(&obj.to_string()))
}

/// Tests that `test_object_sorted_keys` output has sorted keys.
#[test]
fn object_sorted_keys_verified() {
    assert_eq!(check_object_sorted_keys_verified(), Ok(()));
}

fn check_nested_object_keys_verified() -> Result<(), Fail> {
    let mut inner = JsonObject::new();
    inner.insert_int(JsonKey::new("x").or_fail()?, JsonInt(42));
    let mut outer = JsonObject::new();
    outer.insert_object(JsonKey::new("obj").or_fail()?, inner);
    verify_keys_sorted(Text(&outer.to_string()))
}

/// Tests that `nested_object` output has sorted keys.
#[test]
fn nested_object_keys_verified() {
    assert_eq!(check_nested_object_keys_verified(), Ok(()));
}

fn check_object_deterministic_key_ordering() -> Result<(), Fail> {
    // Test with multiple key sets in different insertion orders
    let test_cases = [("a", "b", "c"), ("z", "m", "a"), ("x", "y", "z")];

    for (k1, k2, k3) in test_cases {
        let mut obj = JsonObject::new();
        obj.insert_int(JsonKey::new(k1).or_fail()?, JsonInt(1));
        obj.insert_int(JsonKey::new(k2).or_fail()?, JsonInt(2));
        obj.insert_int(JsonKey::new(k3).or_fail()?, JsonInt(3));
        verify_keys_sorted(Text(&obj.to_string()))?;
    }
    Ok(())
}

/// Deterministic test for key ordering with fixed insertion patterns.
#[test]
fn object_deterministic_key_ordering() {
    assert_eq!(check_object_deterministic_key_ordering(), Ok(()));
}

fn check_determinism_same_object() -> Result<(), Fail> {
    let mut obj1 = JsonObject::new();
    obj1.insert_int(JsonKey::new("a").or_fail()?, JsonInt(10));
    obj1.insert_int(JsonKey::new("b").or_fail()?, JsonInt(20));
    obj1.insert_int(JsonKey::new("c").or_fail()?, JsonInt(30));

    let mut obj2 = JsonObject::new();
    obj2.insert_int(JsonKey::new("a").or_fail()?, JsonInt(10));
    obj2.insert_int(JsonKey::new("b").or_fail()?, JsonInt(20));
    obj2.insert_int(JsonKey::new("c").or_fail()?, JsonInt(30));

    ensure_eq!(
        obj1.to_string(),
        obj2.to_string(),
        "identical objects must produce identical JSON"
    );
    Ok(())
}

/// Tests that the same `JsonObject` built twice produces byte-identical output.
#[test]
fn determinism_same_object() {
    assert_eq!(check_determinism_same_object(), Ok(()));
}

fn check_determinism_different_insertion_order() -> Result<(), Fail> {
    let mut obj1 = JsonObject::new();
    obj1.insert_int(JsonKey::new("z").or_fail()?, JsonInt(1));
    obj1.insert_int(JsonKey::new("a").or_fail()?, JsonInt(2));
    obj1.insert_int(JsonKey::new("m").or_fail()?, JsonInt(3));

    let mut obj2 = JsonObject::new();
    obj2.insert_int(JsonKey::new("a").or_fail()?, JsonInt(2));
    obj2.insert_int(JsonKey::new("m").or_fail()?, JsonInt(3));
    obj2.insert_int(JsonKey::new("z").or_fail()?, JsonInt(1));

    ensure_eq!(
        obj1.to_string(),
        obj2.to_string(),
        "different insertion orders must produce identical JSON"
    );
    Ok(())
}

/// Tests determinism when inserting fields via different call orders.
#[test]
fn determinism_different_insertion_order() {
    assert_eq!(check_determinism_different_insertion_order(), Ok(()));
}

/// Tests that no floating-point numbers are emitted.
///
/// Checks the raw JSON string for telltale float markers (`.` or `e`/`E`
/// after a digit in a bare number), being careful not to match inside
/// quoted strings.
fn check_no_floats_in_json(json: Text<'_>) -> Result<(), Fail> {
    let Text(json_str) = json;
    let mut in_string = false;
    let mut escape_next = false;
    let mut previous: Option<char> = None;

    for (position, ch) in json_str.chars().enumerate() {
        let before = previous;
        previous = Some(ch);
        if escape_next {
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '.' | 'e' | 'E' if !in_string => {
                // Could be a float. Check context: should be surrounded by digits and operators.
                let before_digit = matches!(before, Some(prior) if prior.is_ascii_digit());
                ensure!(
                    !before_digit,
                    "Possible float marker '{ch}' at position {position}"
                );
            }
            _ => {}
        }
    }

    Ok(())
}

fn check_no_floats_object() -> Result<(), Fail> {
    let mut obj = JsonObject::new();
    obj.insert_int(JsonKey::new("a").or_fail()?, JsonInt(42));
    obj.insert_ratio(JsonKey::new("ratio").or_fail()?, Frac(3, 2).ratio()?);
    check_no_floats_in_json(Text(&obj.to_string()))
}

/// Tests that `JsonObject` output contains no floating-point numbers.
#[test]
fn no_floats_object() {
    assert_eq!(check_no_floats_object(), Ok(()));
}

/// Tests that `JsonArray` output contains no floating-point numbers.
#[test]
fn no_floats_array() {
    let mut arr = JsonArray::new();
    arr.push_int(JsonInt(10));
    arr.push_int(JsonInt(20));
    let json_str = arr.to_string();
    assert_eq!(check_no_floats_in_json(Text(&json_str)), Ok(()));
}
