extern crate alloc;
use super::*;
use alloc::collections::BTreeSet;

/// Every key really does satisfy the alphabet its own validator enforces.
///
/// The conversion above skips that check, so this is what stands behind
/// it. The conversion is a match, so a new variant cannot be added
/// without an arm; adding it here as well is what keeps this honest.
#[test]
fn every_key_satisfies_the_key_alphabet() {
    for key in ManifestKey::ALL {
        let known = JsonKey::from(key);
        let text = known.as_str();
        assert_eq!(
            JsonKey::new(text).as_ref(),
            Ok(&known),
            "manifest key {text:?} does not match the key alphabet, or did not survive validation exactly"
        );
    }
}

/// No two keys collide, which a duplicated literal would cause.
#[test]
fn keys_are_distinct() {
    let mut seen = BTreeSet::new();
    for key in ManifestKey::ALL {
        let known = JsonKey::from(key);
        assert!(
            seen.insert(known.clone()),
            "duplicate manifest key {:?}",
            known.as_str()
        );
    }
    assert_eq!(seen.len(), 55, "ALL must list every key exactly once");
}
