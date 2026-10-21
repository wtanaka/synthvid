//! Unit tests for verifying inputs the shell has already loaded.

use super::super::entry_name::RequestedName;
use super::super::report::{FileBytes, FileName};
use super::super::verify_input::CorpusFile;
use super::tests::{lock_entry, Payload};
use super::*;
use std::io;
use synthvid_catalog::generate_lockfile;
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// A lockfile whose only line is `name` with `payload`'s digest and length.
fn lockfile_text_for(name: &FileName, payload: &FileBytes) -> Result<LockfileText, Fail> {
    Ok(generate_lockfile(vec![lock_entry(
        LockfileName::new(&name.0).or_fail()?,
        Payload(&payload.0),
    )?]))
}

/// A corpus file called `name` holding `bytes`.
const fn corpus_file(name: FileName, bytes: FileBytes) -> CorpusFile {
    CorpusFile { name, bytes }
}

/// A corpus whose lockfile and files were both read.
fn listed(lockfile: LockfileText, files: Vec<CorpusFile>) -> LoadedVerify {
    LoadedVerify::Corpus(CorpusSnapshot::Listed {
        lockfile,
        files: Ok(CorpusFiles(files)),
    })
}

#[test]
fn corpus_matching_files_report_nothing() {
    assert_eq!(check_corpus_matching_files_report_nothing(), Ok(()));
}

fn check_corpus_matching_files_report_nothing() -> Result<(), Fail> {
    let loaded = listed(
        lockfile_text_for(&FileName("a.json".to_owned()), &FileBytes(b"same".to_vec()))?,
        vec![corpus_file(
            FileName("a.json".to_owned()),
            FileBytes(b"same".to_vec()),
        )],
    );
    let report = plan_loaded(loaded);
    ensure!(report.trace().0.is_empty());
    ensure_eq!(report.outcome, Outcome::Success);
    Ok(())
}

#[test]
fn corpus_digest_mismatch_is_one_stderr_line() {
    assert_eq!(check_corpus_digest_mismatch_is_one_stderr_line(), Ok(()));
}

fn check_corpus_digest_mismatch_is_one_stderr_line() -> Result<(), Fail> {
    let loaded = listed(
        lockfile_text_for(&FileName("a.json".to_owned()), &FileBytes(b"aaaa".to_vec()))?,
        vec![corpus_file(
            FileName("a.json".to_owned()),
            FileBytes(b"bbbb".to_vec()),
        )],
    );
    let report = plan_loaded(loaded);
    let trace = report.trace().0;
    ensure_eq!(trace.len(), 1, "{trace:?}");
    ensure!(
        trace
            .first()
            .or_fail()?
            .starts_with("stderr digest mismatch in a.json: "),
        "{trace:?}"
    );
    ensure_eq!(report.outcome, Outcome::Failure);
    Ok(())
}

#[test]
fn corpus_missing_and_extra_files_are_both_reported() {
    assert_eq!(
        check_corpus_missing_and_extra_files_are_both_reported(),
        Ok(())
    );
}

fn check_corpus_missing_and_extra_files_are_both_reported() -> Result<(), Fail> {
    let loaded = listed(
        lockfile_text_for(&FileName("a.json".to_owned()), &FileBytes(b"x".to_vec()))?,
        vec![corpus_file(
            FileName("b.txt".to_owned()),
            FileBytes(b"y".to_vec()),
        )],
    );
    let trace = plan_loaded(loaded).trace().0;
    ensure_eq!(trace.len(), 2, "{trace:?}");
    ensure!(trace
        .iter()
        .any(|line| line == "stderr missing artifact: a.json"));
    ensure!(trace
        .iter()
        .any(|line| line == "stderr extra artifact: b.txt"));
    Ok(())
}

#[test]
fn corpus_unreadable_lockfile_is_the_only_report() {
    let loaded = LoadedVerify::Corpus(CorpusSnapshot::Unreadable(InputError::LockfileRead(
        io::Error::other("boom"),
    )));
    let report = plan_loaded(loaded);
    assert_eq!(report.trace().0, ["stderr failed to read lockfile: boom"]);
    assert_eq!(report.outcome, Outcome::Failure);
}

#[test]
fn corpus_directory_failure_is_reported_once_the_lockfile_parses() {
    let loaded = LoadedVerify::Corpus(CorpusSnapshot::Listed {
        lockfile: LockfileText(String::new()),
        files: Err(InputError::CorpusDirectory(io::Error::other("nope"))),
    });
    let report = plan_loaded(loaded);
    assert_eq!(
        report.trace().0,
        ["stderr failed to read corpus directory: nope"]
    );
    assert_eq!(report.outcome, Outcome::Failure);
}

#[test]
fn corpus_file_failure_names_the_file() {
    let loaded = LoadedVerify::Corpus(CorpusSnapshot::Listed {
        lockfile: LockfileText(String::new()),
        files: Err(InputError::CorpusFile {
            name: FileName("a.json".to_owned()),
            cause: io::Error::other("boom"),
        }),
    });
    assert_eq!(
        plan_loaded(loaded).trace().0,
        ["stderr failed to read file a.json: boom"]
    );
}

#[test]
fn corpus_lockfile_parse_error_comes_before_a_directory_failure() {
    assert_eq!(
        check_corpus_lockfile_parse_error_comes_before_a_directory_failure(),
        Ok(())
    );
}

fn check_corpus_lockfile_parse_error_comes_before_a_directory_failure() -> Result<(), Fail> {
    let loaded = LoadedVerify::Corpus(CorpusSnapshot::Listed {
        lockfile: LockfileText("garbage\n".to_owned()),
        files: Err(InputError::CorpusDirectory(io::Error::other("nope"))),
    });
    let trace = plan_loaded(loaded).trace().0;
    ensure_eq!(trace.len(), 1, "{trace:?}");
    ensure!(
        trace
            .first()
            .or_fail()?
            .starts_with("stderr failed to parse lockfile: "),
        "{trace:?}"
    );
    Ok(())
}

#[test]
fn regenerate_unreadable_lockfile_is_reported() {
    let loaded = LoadedVerify::Regenerate {
        requested: Requested::Everything,
        lockfile: Err(InputError::LockfileRead(io::Error::other("boom"))),
    };
    let report = plan_loaded(loaded);
    assert_eq!(report.trace().0, ["stderr failed to read lockfile: boom"]);
    assert_eq!(report.outcome, Outcome::Failure);
}

#[test]
fn regenerate_duplicate_name_is_refused() {
    let digest = "0".repeat(64);
    let loaded = LoadedVerify::Regenerate {
        requested: Requested::Everything,
        lockfile: Ok(LockfileText(format!(
            "a.json {digest} 1\na.json {digest} 2\n"
        ))),
    };
    assert_eq!(
        plan_loaded(loaded).trace().0,
        ["stderr lockfile lists a.json more than once"]
    );
}

#[test]
fn regenerate_unknown_name_is_reported() {
    let loaded = LoadedVerify::Regenerate {
        requested: Requested::Named(vec![RequestedName::new("nope".to_owned())]),
        lockfile: Ok(generate_lockfile(Vec::new())),
    };
    let report = plan_loaded(loaded);
    assert_eq!(report.trace().0, ["stderr unknown entry name: nope"]);
    assert_eq!(report.outcome, Outcome::Failure);
}
