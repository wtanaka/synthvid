use core::fmt;
use std::process::{Command, Stdio};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};
use synthvid_video::writer::{JsonArray, JsonInt, JsonKey, JsonObject};

use crate::support::{availability, Availability, OracleFailure, Program};

/// JSON text handed to an external oracle.
#[derive(Clone, Copy)]
struct Json<'text>(&'text str);

/// What an external oracle reported when it accepted its input.
struct OraclePass(String);

impl fmt::Debug for OraclePass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OraclePass").field(&self.0).finish()
    }
}

/// The canonical form of a JSON text, as `jq -S -c .` writes it.
#[derive(Debug)]
struct CanonicalJson(String);

/// Tries to validate JSON with jq.
///
/// Returns `Ok(Some(msg))` if jq was used and validation passed,
/// `Ok(None)` if jq is not available,
/// `Err(msg)` if validation failed.
fn validate_json_with_jq(json: Json<'_>) -> Result<Option<OraclePass>, OracleFailure> {
    match availability(Program("jq")) {
        Availability::Installed => {}
        Availability::Missing => return Ok(None),
    }

    let mut child = Command::new("jq")
        .arg("empty")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| OracleFailure(format!("failed to spawn jq: {error}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write as _;
        stdin
            .write_all(json.0.as_bytes())
            .map_err(|error| OracleFailure(format!("failed to write to jq stdin: {error}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| OracleFailure(format!("failed to wait for jq: {error}")))?;

    if output.status.success() {
        Ok(Some(OraclePass("jq RFC 8259 validation passed".to_owned())))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(OracleFailure(format!("jq validation failed: {stderr}")))
    }
}

/// Tries to validate JSON with python3.
///
/// Returns `Ok(Some(msg))` if python3 was used and validation passed,
/// `Ok(None)` if python3 is not available,
/// `Err(msg)` if validation failed.
fn validate_json_with_python3(json: Json<'_>) -> Result<Option<OraclePass>, OracleFailure> {
    match availability(Program("python3")) {
        Availability::Installed => {}
        Availability::Missing => return Ok(None),
    }

    let code = "import json,sys; json.loads(sys.argv[1])";
    let child = Command::new("python3")
        .arg("-c")
        .arg(code)
        .arg(json.0)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| OracleFailure(format!("failed to spawn python3: {error}")))?;

    let output = child
        .wait_with_output()
        .map_err(|error| OracleFailure(format!("failed to wait for python3: {error}")))?;

    if output.status.success() {
        Ok(Some(OraclePass(
            "python3 RFC 8259 validation passed".to_owned(),
        )))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(OracleFailure(format!(
            "python3 validation failed: {stderr}"
        )))
    }
}

/// Canonicalizes JSON using jq.
///
/// Returns the canonicalized JSON (sorted keys, compact), or None if jq is not available.
fn canonicalize_with_jq(json: Json<'_>) -> Result<Option<CanonicalJson>, OracleFailure> {
    match availability(Program("jq")) {
        Availability::Installed => {}
        Availability::Missing => return Ok(None),
    }

    let mut child = Command::new("jq")
        .arg("-S")
        .arg("-c")
        .arg(".")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| OracleFailure(format!("failed to spawn jq -S -c: {error}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write as _;
        stdin
            .write_all(json.0.as_bytes())
            .map_err(|error| OracleFailure(format!("failed to write to jq stdin: {error}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| OracleFailure(format!("failed to wait for jq: {error}")))?;

    if output.status.success() {
        let result = String::from_utf8(output.stdout)
            .map_err(|error| OracleFailure(format!("jq output not UTF-8: {error}")))?
            .trim_end()
            .to_owned();
        Ok(Some(CanonicalJson(result)))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(OracleFailure(format!("jq -S -c failed: {stderr}")))
    }
}

/// Checks for floating-point markers in JSON using jq.
///
/// Returns Ok(Some(msg)) if check passed, Ok(None) if jq unavailable, Err if check failed.
fn check_no_floats_with_jq(json: Json<'_>) -> Result<Option<OraclePass>, OracleFailure> {
    match availability(Program("jq")) {
        Availability::Installed => {}
        Availability::Missing => return Ok(None),
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
        .map_err(|error| OracleFailure(format!("failed to spawn jq float check: {error}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write as _;
        stdin
            .write_all(json.0.as_bytes())
            .map_err(|error| OracleFailure(format!("failed to write to jq stdin: {error}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| OracleFailure(format!("failed to wait for jq: {error}")))?;

    if output.status.success() {
        Ok(Some(OraclePass("jq float marker check passed".to_owned())))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(OracleFailure(format!("jq float check failed: {stderr}")))
    }
}

/// Checks for floating-point markers in JSON using python3.
///
/// Returns Ok(Some(msg)) if check passed, Ok(None) if python3 unavailable, Err if check failed.
fn check_no_floats_with_python3(json: Json<'_>) -> Result<Option<OraclePass>, OracleFailure> {
    match availability(Program("python3")) {
        Availability::Installed => {}
        Availability::Missing => return Ok(None),
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
        .arg(json.0)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| OracleFailure(format!("failed to spawn python3 float check: {error}")))?;

    let output = child
        .wait_with_output()
        .map_err(|error| OracleFailure(format!("failed to wait for python3: {error}")))?;

    if output.status.success() {
        Ok(Some(OraclePass(
            "python3 float marker check passed".to_owned(),
        )))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(OracleFailure(format!(
            "python3 float check failed: {stderr}"
        )))
    }
}

/// The frozen JSON vector from the manifest test.
// This is the same frozen vector used in manifest.rs test
const EXAMPLE_MANIFEST_JSON: &str = r#"{"background":"grid","dimensions":{"height":16,"width":16},"frame_count":1,"frame_rate":{"den":1,"num":30},"frames":[{"camera":{"a":{"den":1,"num":1},"b":{"den":1,"num":0},"c":{"den":1,"num":0},"d":{"den":1,"num":1},"tx":{"den":1,"num":0},"ty":{"den":1,"num":0}},"index":0,"objects":[{"bbox":{"max_x":{"den":1,"num":10},"max_y":{"den":1,"num":10},"min_x":{"den":1,"num":6},"min_y":{"den":1,"num":6}},"centre_screen":{"x":{"den":1,"num":8},"y":{"den":1,"num":8}},"index":0,"on_screen":{"den":1,"num":1}}]}],"name":"example","objects":[{"index":0,"shape":"disc","visible":{"end":1,"start":0}}],"schema_version":1}"#;

/// Test RFC 8259 validity with jq.
#[test]
fn manifest_json_rfc8259_with_jq() {
    let json = EXAMPLE_MANIFEST_JSON.to_owned();

    // Ok(None) means jq is not installed; that is a skip, not a failure.
    let result = validate_json_with_jq(Json(&json));
    assert!(result.is_ok(), "jq validation failed: {result:?}");
}

/// Test RFC 8259 validity with python3.
#[test]
fn manifest_json_rfc8259_with_python3() {
    let json = EXAMPLE_MANIFEST_JSON.to_owned();

    // Ok(None) means python3 is not installed; that is a skip, not a failure.
    let result = validate_json_with_python3(Json(&json));
    assert!(result.is_ok(), "python3 validation failed: {result:?}");
}

/// Test canonical form (sorted keys, compact) with jq.
#[test]
fn manifest_json_canonical_form_with_jq() {
    let json = EXAMPLE_MANIFEST_JSON.to_owned();

    let result = canonicalize_with_jq(Json(&json));
    assert!(
        result.is_ok(),
        "jq canonicalization check failed: {result:?}"
    );
    // Ok(None) means jq is not installed; skip.
    let Ok(Some(canonicalized)) = result else {
        return;
    };
    assert_eq!(
        json, canonicalized.0,
        "manifest JSON must be byte-identical to jq -S -c . output"
    );
}

/// Test no floating-point numbers with jq.
#[test]
fn manifest_json_no_floats_with_jq() {
    let json = EXAMPLE_MANIFEST_JSON.to_owned();

    // Ok(None) means jq is not installed; that is a skip, not a failure.
    let result = check_no_floats_with_jq(Json(&json));
    assert!(result.is_ok(), "jq float check failed: {result:?}");
}

/// Test no floating-point numbers with python3.
#[test]
fn manifest_json_no_floats_with_python3() {
    let json = EXAMPLE_MANIFEST_JSON.to_owned();

    // Ok(None) means python3 is not installed; that is a skip, not a failure.
    let result = check_no_floats_with_python3(Json(&json));
    assert!(result.is_ok(), "python3 float check failed: {result:?}");
}

/// Test small hand-built `JsonObject` against jq oracle.
#[test]
fn small_json_object_with_jq() {
    assert_eq!(check_small_json_object_with_jq(), Ok(()));
}

fn check_small_json_object_with_jq() -> Result<(), Fail> {
    let mut obj = JsonObject::new();
    obj.insert_int(JsonKey::new("a").or_fail()?, JsonInt(42));
    obj.insert_int(JsonKey::new("b").or_fail()?, JsonInt(100));
    let json = obj.to_string();

    // Test canonicalization
    let result = canonicalize_with_jq(Json(&json));
    ensure!(result.is_ok(), "small object jq test failed: {result:?}");
    // Ok(None) means jq is not installed; skip.
    let Ok(Some(canonicalized)) = result else {
        return Ok(());
    };
    ensure_eq!(
        json,
        canonicalized.0,
        "small object must match jq -S -c output"
    );
    Ok(())
}

/// Test small hand-built `JsonArray` against jq oracle.
#[test]
fn small_json_array_with_jq() {
    let mut arr = JsonArray::new();
    arr.push_int(JsonInt(1));
    arr.push_int(JsonInt(2));
    arr.push_int(JsonInt(3));
    let json = arr.to_string();

    // Test canonicalization
    let result = canonicalize_with_jq(Json(&json));
    assert!(result.is_ok(), "small array jq test failed: {result:?}");
    // Ok(None) means jq is not installed; skip.
    let Ok(Some(canonicalized)) = result else {
        return;
    };
    assert_eq!(
        json, canonicalized.0,
        "small array must match jq -S -c output"
    );
}

/// Test nested objects with rational numbers.
#[test]
fn nested_json_with_rationals_and_jq() {
    assert_eq!(check_nested_json_with_rationals_and_jq(), Ok(()));
}

fn check_nested_json_with_rationals_and_jq() -> Result<(), Fail> {
    let mut inner = JsonObject::new();
    inner.insert_int(JsonKey::new("den").or_fail()?, JsonInt(3));
    inner.insert_int(JsonKey::new("num").or_fail()?, JsonInt(2));

    let mut outer = JsonObject::new();
    outer.insert_object(JsonKey::new("ratio").or_fail()?, inner);
    let json = outer.to_string();

    // Test RFC 8259 validity
    let jq_result = validate_json_with_jq(Json(&json));
    ensure!(
        jq_result.is_ok(),
        "nested object jq validation failed: {jq_result:?}"
    );

    // Test no floats
    let float_result = check_no_floats_with_jq(Json(&json));
    ensure!(
        float_result.is_ok(),
        "nested object jq float check failed: {float_result:?}"
    );
    Ok(())
}

/// Test that external oracles are available.
///
/// This test documents which external tools are available for validation
/// in this environment, without failing if they're missing.
#[test]
fn external_oracle_availability() {
    // The probe must answer "not installed" for a command that cannot exist,
    // so that a probe which always says yes cannot hide a missing oracle.
    let missing = availability(Program("synthvid-no-such-command-0c1f"));
    assert_eq!(
        missing,
        Availability::Missing,
        "a command that is not installed must report as unavailable"
    );

    // Tests gracefully skip if external tools are not available.
    // This is not a failure condition - validation just won't occur.
}
