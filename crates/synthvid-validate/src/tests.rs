use std::process::{Command, Stdio};
use synthvid_catalog::writer::{JsonArray, JsonKey, JsonObject};

use crate::support::is_command_available;

/// Tries to validate JSON with jq.
///
/// Returns `Ok(Some(msg))` if jq was used and validation passed,
/// `Ok(None)` if jq is not available,
/// `Err(msg)` if validation failed.
fn validate_json_with_jq(json: &str) -> Result<Option<String>, String> {
    if !is_command_available("jq") {
        return Ok(None);
    }

    let mut child = Command::new("jq")
        .arg("empty")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn jq: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(json.as_bytes())
            .map_err(|e| format!("failed to write to jq stdin: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for jq: {e}"))?;

    if output.status.success() {
        Ok(Some("jq RFC 8259 validation passed".to_owned()))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("jq validation failed: {stderr}"))
    }
}

/// Tries to validate JSON with python3.
///
/// Returns `Ok(Some(msg))` if python3 was used and validation passed,
/// `Ok(None)` if python3 is not available,
/// `Err(msg)` if validation failed.
fn validate_json_with_python3(json: &str) -> Result<Option<String>, String> {
    if !is_command_available("python3") {
        return Ok(None);
    }

    let code = "import json,sys; json.loads(sys.argv[1])";
    let child = Command::new("python3")
        .arg("-c")
        .arg(code)
        .arg(json)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn python3: {e}"))?;

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for python3: {e}"))?;

    if output.status.success() {
        Ok(Some("python3 RFC 8259 validation passed".to_owned()))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("python3 validation failed: {stderr}"))
    }
}

/// Canonicalizes JSON using jq.
///
/// Returns the canonicalized JSON (sorted keys, compact), or None if jq is not available.
fn canonicalize_with_jq(json: &str) -> Result<Option<String>, String> {
    if !is_command_available("jq") {
        return Ok(None);
    }

    let mut child = Command::new("jq")
        .arg("-S")
        .arg("-c")
        .arg(".")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn jq -S -c: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(json.as_bytes())
            .map_err(|e| format!("failed to write to jq stdin: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for jq: {e}"))?;

    if output.status.success() {
        let result = String::from_utf8(output.stdout)
            .map_err(|e| format!("jq output not UTF-8: {e}"))?
            .trim_end()
            .to_owned();
        Ok(Some(result))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("jq -S -c failed: {stderr}"))
    }
}

/// Checks for floating-point markers in JSON using jq.
///
/// Returns Ok(Some(msg)) if check passed, Ok(None) if jq unavailable, Err if check failed.
fn check_no_floats_with_jq(json: &str) -> Result<Option<String>, String> {
    if !is_command_available("jq") {
        return Ok(None);
    }

    // Use jq to find any bare numbers with float markers
    let code = r#"
    def has_float_markers:
      tostring | test("[0-9]\\.[0-9]|[0-9][eE]");

    walk(
      if type == "number" then
        if has_float_markers then
          error("found floating-point number: " + tostring)
        else
          .
        end
      else
        .
      end
    )
    "#;

    let mut child = Command::new("jq")
        .arg(code)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn jq float check: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(json.as_bytes())
            .map_err(|e| format!("failed to write to jq stdin: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for jq: {e}"))?;

    if output.status.success() {
        Ok(Some("jq float marker check passed".to_owned()))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("jq float check failed: {stderr}"))
    }
}

/// Checks for floating-point markers in JSON using python3.
///
/// Returns Ok(Some(msg)) if check passed, Ok(None) if python3 unavailable, Err if check failed.
fn check_no_floats_with_python3(json: &str) -> Result<Option<String>, String> {
    if !is_command_available("python3") {
        return Ok(None);
    }

    let code = r#"
import json, sys, re
try:
    data = json.loads(sys.argv[1])
    # Check that the raw JSON string doesn't contain float markers outside of strings
    in_string = False
    escape_next = False
    for i, ch in enumerate(sys.argv[1]):
        if escape_next:
            escape_next = False
            continue
        if ch == '\\' and in_string:
            escape_next = True
            continue
        if ch == '"':
            in_string = not in_string
            continue
        if not in_string and ch in '.eE':
            # Check context: ensure it's part of a number token
            before_digit = i > 0 and sys.argv[1][i-1].isdigit()
            if before_digit:
                sys.exit(1)
except Exception as e:
    print(f"Error: {e}", file=sys.stderr)
    sys.exit(1)
"#;

    let child = Command::new("python3")
        .arg("-c")
        .arg(code)
        .arg(json)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn python3 float check: {e}"))?;

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for python3: {e}"))?;

    if output.status.success() {
        Ok(Some("python3 float marker check passed".to_owned()))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("python3 float check failed: {stderr}"))
    }
}

fn make_key(s: &str) -> JsonKey {
    JsonKey::new(s).expect("test key must be valid")
}

// Test helper: return the frozen JSON vector from manifest.rs test
fn make_example_manifest_json() -> String {
    // This is the same frozen vector used in manifest.rs test
    r#"{"background":"grid","dimensions":{"height":16,"width":16},"frame_count":1,"frame_rate":{"den":1,"num":30},"frames":[{"camera":{"a":{"den":1,"num":1},"b":{"den":1,"num":0},"c":{"den":1,"num":0},"d":{"den":1,"num":1},"tx":{"den":1,"num":0},"ty":{"den":1,"num":0}},"index":0,"objects":[{"bbox":{"max_x":{"den":1,"num":10},"max_y":{"den":1,"num":10},"min_x":{"den":1,"num":6},"min_y":{"den":1,"num":6}},"centre_screen":{"x":{"den":1,"num":8},"y":{"den":1,"num":8}},"index":0,"on_screen":{"den":1,"num":1}}]}],"name":"example","objects":[{"index":0,"shape":"disc","visible":{"end":1,"start":0}}],"schema_version":1}"#.to_owned()
}

/// Test RFC 8259 validity with jq.
#[test]
fn test_manifest_json_rfc8259_with_jq() {
    let json = make_example_manifest_json();

    match validate_json_with_jq(&json) {
        Ok(Some(_msg)) => {
            // jq validation passed
        }
        Ok(None) => {
            // jq not available, skip
        }
        Err(e) => {
            panic!("jq validation failed: {e}");
        }
    }
}

/// Test RFC 8259 validity with python3.
#[test]
fn test_manifest_json_rfc8259_with_python3() {
    let json = make_example_manifest_json();

    match validate_json_with_python3(&json) {
        Ok(Some(_msg)) => {
            // python3 validation passed
        }
        Ok(None) => {
            // python3 not available, skip
        }
        Err(e) => {
            panic!("python3 validation failed: {e}");
        }
    }
}

/// Test canonical form (sorted keys, compact) with jq.
#[test]
fn test_manifest_json_canonical_form_with_jq() {
    let json = make_example_manifest_json();

    match canonicalize_with_jq(&json) {
        Ok(Some(canonicalized)) => {
            assert_eq!(
                json, canonicalized,
                "manifest JSON must be byte-identical to jq -S -c . output"
            );
        }
        Ok(None) => {
            // jq not available, skip
        }
        Err(e) => {
            panic!("jq canonicalization check failed: {e}");
        }
    }
}

/// Test no floating-point numbers with jq.
#[test]
fn test_manifest_json_no_floats_with_jq() {
    let json = make_example_manifest_json();

    match check_no_floats_with_jq(&json) {
        Ok(Some(_msg)) => {
            // jq float check passed
        }
        Ok(None) => {
            // jq not available, skip
        }
        Err(e) => {
            panic!("jq float check failed: {e}");
        }
    }
}

/// Test no floating-point numbers with python3.
#[test]
fn test_manifest_json_no_floats_with_python3() {
    let json = make_example_manifest_json();

    match check_no_floats_with_python3(&json) {
        Ok(Some(_msg)) => {
            // python3 validation passed
        }
        Ok(None) => {
            // python3 not available, skip
        }
        Err(e) => {
            panic!("python3 float check failed: {e}");
        }
    }
}

/// Test small hand-built `JsonObject` against jq oracle.
#[test]
fn test_small_json_object_with_jq() {
    let mut obj = JsonObject::new();
    obj.insert_int(make_key("a"), 42);
    obj.insert_int(make_key("b"), 100);
    let json = obj.build();

    // Test canonicalization
    match canonicalize_with_jq(&json) {
        Ok(Some(canonicalized)) => {
            assert_eq!(
                json, canonicalized,
                "small object must match jq -S -c output"
            );
        }
        Ok(None) => {
            // jq not available, skip
        }
        Err(e) => {
            panic!("small object jq test failed: {e}");
        }
    }
}

/// Test small hand-built `JsonArray` against jq oracle.
#[test]
fn test_small_json_array_with_jq() {
    let mut arr = JsonArray::new();
    arr.push_int(1);
    arr.push_int(2);
    arr.push_int(3);
    let json = arr.build();

    // Test canonicalization
    match canonicalize_with_jq(&json) {
        Ok(Some(canonicalized)) => {
            assert_eq!(
                json, canonicalized,
                "small array must match jq -S -c output"
            );
        }
        Ok(None) => {
            // jq not available, skip
        }
        Err(e) => {
            panic!("small array jq test failed: {e}");
        }
    }
}

/// Test nested objects with rational numbers.
#[test]
fn test_nested_json_with_rationals_and_jq() {
    let mut inner = JsonObject::new();
    inner.insert_int(make_key("den"), 3);
    inner.insert_int(make_key("num"), 2);

    let mut outer = JsonObject::new();
    outer.insert_object(make_key("ratio"), inner);
    let json = outer.build();

    // Test RFC 8259 validity
    match validate_json_with_jq(&json) {
        Ok(Some(_msg)) => {
            // validation passed
        }
        Ok(None) => {
            // jq not available, skip
        }
        Err(e) => {
            panic!("nested object jq validation failed: {e}");
        }
    }

    // Test no floats
    match check_no_floats_with_jq(&json) {
        Ok(Some(_msg)) => {
            // float check passed
        }
        Ok(None) => {
            // jq not available, skip
        }
        Err(e) => {
            panic!("nested object jq float check failed: {e}");
        }
    }
}

/// Test that external oracles are available.
///
/// This test documents which external tools are available for validation
/// in this environment, without failing if they're missing.
#[test]
fn test_external_oracle_availability() {
    let _has_jq = is_command_available("jq");
    let _has_python3 = is_command_available("python3");

    // Tests gracefully skip if external tools are not available.
    // This is not a failure condition - validation just won't occur.
}
