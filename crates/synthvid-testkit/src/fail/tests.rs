//! Tests of the failure value and its conversions.

use super::{refusal, Fail, OrFail as _};
use crate::{ensure, ensure_eq, ensure_ne};

fn check_refusal_yields_the_error_and_rejects_acceptance() -> Result<(), Fail> {
    ensure_eq!(refusal(Err::<(), u8>(7))?, 7);
    ensure!(refusal(Ok::<(), u8>(())).is_err());
    Ok(())
}

#[test]
fn a_refusal_yields_its_error_and_an_acceptance_fails() {
    assert_eq!(
        check_refusal_yields_the_error_and_rejects_acceptance(),
        Ok(())
    );
}

fn check_passes() -> Result<(), Fail> {
    let parsed: u8 = "7".parse().or_fail()?;
    ensure_eq!(parsed, 7);
    ensure_ne!(parsed, 8);
    ensure!(parsed > 6);
    Ok(())
}

fn check_missing() -> Result<(), Fail> {
    let found: Option<u8> = None;
    let value = found.or_fail()?;
    ensure_eq!(value, 0);
    Ok(())
}

fn check_unequal() -> Result<(), Fail> {
    ensure_eq!(1_u8, 2_u8, "one is not two");
    Ok(())
}

#[test]
fn a_check_that_holds_passes() {
    assert_eq!(check_passes(), Ok(()));
}

#[test]
fn a_missing_value_fails() {
    assert_ne!(check_missing(), Ok(()));
}

#[test]
fn an_unequal_pair_fails_with_both_sides() {
    let outcome = check_unequal().map_err(|fail| fail.to_string());
    assert_eq!(
        outcome.map_err(|text| text.ends_with("one is not two\n  left: 1\n right: 2")),
        Err(true)
    );
}
