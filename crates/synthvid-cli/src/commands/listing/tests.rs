//! Unit tests for the list and manifest subcommand plans.

use super::*;
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

#[test]
fn list_text_prints_one_name_per_line() {
    assert_eq!(check_list_text_prints_one_name_per_line(), Ok(()));
}

fn check_list_text_prints_one_name_per_line() -> Result<(), Fail> {
    let args: Vec<String> = vec!["--format".into(), "text".into()];
    let report = plan_list(Argv(&args));
    let names = generate_catalogue().or_fail()?.len();
    let trace = report.trace().0;
    ensure_eq!(trace.len(), 1);
    let line = trace.first().or_fail()?;
    ensure!(line.starts_with("stdout \""));
    ensure_eq!(line.matches("\\n").count(), names);
    ensure_eq!(report.outcome, Outcome::Success);
    Ok(())
}

#[test]
fn list_json_prints_a_bracketed_array() {
    assert_eq!(check_list_json_prints_a_bracketed_array(), Ok(()));
}

fn check_list_json_prints_a_bracketed_array() -> Result<(), Fail> {
    let args: Vec<String> = vec!["--format".into(), "json".into()];
    let report = plan_list(Argv(&args));
    let trace = report.trace().0;
    ensure_eq!(trace.len(), 1);
    let line = trace.first().or_fail()?;
    ensure!(line.starts_with("stdout \"[\\\""));
    ensure!(line.ends_with("]\\n\""));
    ensure_eq!(report.outcome, Outcome::Success);
    Ok(())
}

#[test]
fn list_unknown_format_is_reported_after_the_catalogue() {
    let args: Vec<String> = vec!["--format".into(), "xml".into()];
    let report = plan_list(Argv(&args));
    assert_eq!(report.trace().0, ["stderr unknown format: xml"]);
    assert_eq!(report.outcome, Outcome::Failure);
}

#[test]
fn list_format_without_value_is_named() {
    let args: Vec<String> = vec!["--format".into()];
    let report = plan_list(Argv(&args));
    assert_eq!(report.trace().0, ["stderr --format requires an argument"]);
}

#[test]
fn manifest_without_name_is_reported() {
    let args: Vec<String> = Vec::new();
    let report = plan_manifest(Argv(&args));
    assert_eq!(report.trace().0, ["stderr --name is required"]);
    assert_eq!(report.outcome, Outcome::Failure);
}

#[test]
fn manifest_for_an_unknown_name_reports_it() {
    let args: Vec<String> = vec!["--name".into(), "no-such-entry".into()];
    let report = plan_manifest(Argv(&args));
    assert_eq!(report.trace().0, ["stderr entry not found: no-such-entry"]);
    assert_eq!(report.outcome, Outcome::Failure);
}
