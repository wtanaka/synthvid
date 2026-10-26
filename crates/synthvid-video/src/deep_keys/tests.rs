extern crate alloc;
use super::*;
use alloc::collections::BTreeSet;
use synthvid_testkit::{ensure, ensure_eq, Fail};

/// Every deep key satisfies the alphabet its own validator enforces.
#[test]
fn every_deep_key_satisfies_the_key_alphabet() {
    assert_eq!(check_every_deep_key_satisfies_the_key_alphabet(), Ok(()));
}

/// The conversion skips that check, so this is what stands behind it.
fn check_every_deep_key_satisfies_the_key_alphabet() -> Result<(), Fail> {
    for key in DeepKey::ALL {
        let known = JsonKey::from(key);
        let text = known.as_str();
        ensure_eq!(JsonKey::new(text).as_ref(), Ok(&known));
    }
    Ok(())
}

/// No two deep keys collide, and they are listed in the order of their spellings.
#[test]
fn deep_keys_are_distinct_and_sorted() {
    assert_eq!(check_deep_keys_are_distinct_and_sorted(), Ok(()));
}

/// Every key in `ALL` is distinct, `ALL` lists all fifteen keys, and it lists them sorted.
fn check_deep_keys_are_distinct_and_sorted() -> Result<(), Fail> {
    let mut seen = BTreeSet::new();
    let mut listed = Vec::new();
    for key in DeepKey::ALL {
        let known = JsonKey::from(key);
        ensure!(
            seen.insert(known.clone()),
            "duplicate deep key {}",
            known.as_str()
        );
        listed.push(known);
    }
    ensure_eq!(
        listed.len(),
        15,
        "ALL must list every deep key exactly once"
    );
    let sorted: Vec<JsonKey> = seen.into_iter().collect();
    ensure_eq!(
        listed,
        sorted,
        "ALL must list the spellings in sorted order"
    );
    Ok(())
}

/// The deep keys are spelled as the deep manifest writes them.
#[test]
fn deep_keys_are_spelled_as_written() {
    assert_eq!(check_deep_keys_are_spelled_as_written(), Ok(()));
}

/// Each deep key converts to its manifest spelling.
fn check_deep_keys_are_spelled_as_written() -> Result<(), Fail> {
    ensure_eq!(
        JsonKey::from(DeepKey::PixelSamples).as_str(),
        "pixel_samples"
    );
    ensure_eq!(JsonKey::from(DeepKey::FrameHeight).as_str(), "frame_height");
    ensure_eq!(JsonKey::from(DeepKey::Vertices).as_str(), "vertices");
    ensure_eq!(JsonKey::from(DeepKey::PointZ).as_str(), "z");
    Ok(())
}
