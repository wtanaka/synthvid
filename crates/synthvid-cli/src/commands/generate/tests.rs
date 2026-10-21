//! Unit tests for the generate subcommand plans.

use super::*;
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

#[test]
fn missing_out_is_reported_without_steps() {
    let args: Vec<String> = vec!["--only".into(), "x".into()];
    let report = plan_generate(Argv(&args));
    assert_eq!(report.trace().0, ["stderr --out is required"]);
    assert_eq!(report.outcome, Outcome::Failure);
}

#[test]
fn unknown_option_names_the_word() {
    let args: Vec<String> = vec!["--out".into(), "d".into(), "--bogus".into()];
    let report = plan_generate(Argv(&args));
    assert_eq!(report.trace().0, ["stderr unknown option: --bogus"]);
    assert_eq!(report.outcome, Outcome::Failure);
}

#[test]
fn value_flag_without_value_is_named() {
    let args: Vec<String> = vec!["--out".into()];
    let report = plan_generate(Argv(&args));
    assert_eq!(report.trace().0, ["stderr --out requires an argument"]);
}

#[test]
fn dry_run_without_matching_names_creates_no_directory() {
    let args: Vec<String> = vec![
        "--out".into(),
        "d".into(),
        "--only".into(),
        "no-such-entry".into(),
        "--dry-run".into(),
    ];
    let report = plan_generate(Argv(&args));
    assert!(report.steps.is_empty());
    assert_eq!(report.outcome, Outcome::Success);
}

#[test]
fn dry_run_names_each_selected_entry_and_writes_nothing() {
    assert_eq!(
        check_dry_run_names_each_selected_entry_and_writes_nothing(),
        Ok(())
    );
}

fn check_dry_run_names_each_selected_entry_and_writes_nothing() -> Result<(), Fail> {
    let first: String = generate_catalogue()
        .or_fail()?
        .first()
        .or_fail()?
        .manifest_name()
        .as_str()
        .into();
    let args: Vec<String> = vec![
        "--out".into(),
        "d".into(),
        "--only".into(),
        first.clone(),
        "--dry-run".into(),
    ];
    let report = plan_generate(Argv(&args));
    ensure_eq!(
        report.trace().0,
        [format!("stderr would generate: {first}")]
    );
    ensure_eq!(report.outcome, Outcome::Success);
    Ok(())
}

#[test]
fn run_creates_the_directory_then_publishes_the_entry() {
    assert_eq!(
        check_run_creates_the_directory_then_publishes_the_entry(),
        Ok(())
    );
}

fn check_run_creates_the_directory_then_publishes_the_entry() -> Result<(), Fail> {
    let first: String = generate_catalogue()
        .or_fail()?
        .first()
        .or_fail()?
        .manifest_name()
        .as_str()
        .into();
    let args: Vec<String> = vec!["--out".into(), "d".into(), "--only".into(), first];
    let report = plan_generate(Argv(&args));
    ensure_eq!(
        report.trace().0,
        [
            "mkdir d".to_owned(),
            "publish 2 file(s), announce: true".to_owned()
        ]
    );
    ensure_eq!(report.outcome, Outcome::Success);
    Ok(())
}
