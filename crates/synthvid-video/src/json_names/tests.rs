use super::*;
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

fn check_json_key_valid() -> Result<(), Fail> {
    ensure_eq!(JsonKey::new("background").or_fail()?.as_str(), "background");
    ensure_eq!(
        JsonKey::new("frame_count").or_fail()?.as_str(),
        "frame_count"
    );
    ensure_eq!(JsonKey::new("a").or_fail()?.as_str(), "a");
    ensure_eq!(JsonKey::new("_").or_fail()?.as_str(), "_");
    Ok(())
}

#[test]
fn json_key_valid() {
    assert_eq!(check_json_key_valid(), Ok(()));
}

#[test]
fn json_key_invalid() {
    assert!(JsonKey::new("Background").is_err(), "uppercase rejected");
    assert!(JsonKey::new("frame-count").is_err(), "hyphen rejected");
    assert!(JsonKey::new("").is_err(), "empty rejected");
    assert!(JsonKey::new("123").is_err(), "digit in key rejected");
}

fn check_json_entry_name_valid() -> Result<(), Fail> {
    ensure_eq!(JsonEntryName::new("example").or_fail()?.as_str(), "example");
    ensure_eq!(JsonEntryName::new("test-1").or_fail()?.as_str(), "test-1");
    ensure_eq!(JsonEntryName::new("a0").or_fail()?.as_str(), "a0");
    Ok(())
}

#[test]
fn json_entry_name_valid() {
    assert_eq!(check_json_entry_name_valid(), Ok(()));
}

#[test]
fn json_entry_name_invalid() {
    assert!(JsonEntryName::new("Test").is_err(), "uppercase rejected");
    assert!(
        JsonEntryName::new("test_name").is_err(),
        "underscore rejected"
    );
    assert!(JsonEntryName::new("").is_err(), "empty rejected");
}
