//! Unit tests for the verify command.

use super::super::entry_name::{RequestedName, UnknownEntryName};
use super::super::error_chain::ErrorChain;
use super::super::shell::execute_with;
use super::super::verify_args::{Flag, UsageError};
use super::*;
use core::cell::{Cell, RefCell};
use std::io;
use synthvid_catalog::{sha256, ContentLength, EntryError};
use synthvid_scene::FrameCountError;
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};
use synthvid_video::manifest_error::ManifestError;
use synthvid_video::GenerationError;

/// The bytes a test file holds.
#[derive(Clone, Copy)]
pub(super) struct Payload<'bytes>(pub(super) &'bytes [u8]);

/// A manifest-construction failure, as the render pipeline reports one.
fn manifest_construction_failure() -> GenerationError {
    GenerationError::ManifestConstruction(ManifestError::Empty(FrameCountError::Zero))
}

fn length_of(payload: Payload<'_>) -> Result<ContentLength, Fail> {
    Ok(ContentLength::new(
        u64::try_from(payload.0.len()).or_fail()?,
    ))
}

/// A lockfile entry for `name` whose bytes are `payload`.
pub(super) fn lock_entry(name: LockfileName, payload: Payload<'_>) -> Result<LockfileEntry, Fail> {
    Ok(LockfileEntry::new(
        name,
        sha256(payload.0),
        length_of(payload)?,
    ))
}

/// The digest pair a generator would report for `entry` if both files held `payload`.
fn digests_for(entry: &Entry, payload: Payload<'_>) -> Result<EntryDigests, EntryDigestError> {
    let files = EntryFiles::of(entry).map_err(EntryDigestError::Files)?;
    let length = ContentLength::new(
        u64::try_from(payload.0.len()).map_err(EntryDigestError::ManifestLength)?,
    );
    Ok(EntryDigests {
        manifest: LockfileEntry::new(files.manifest, sha256(payload.0), length),
        media: LockfileEntry::new(files.media, sha256(payload.0), length),
    })
}

/// Catalogue entries, and the lockfile entries a check compares them with.
type EntriesAndLockfile = (Vec<Entry>, Vec<LockfileEntry>);

/// The first three catalogue entries and a lockfile that expects `b"expected"` for each.
fn three_entries_and_lockfile() -> Result<EntriesAndLockfile, Fail> {
    let catalogue: Vec<Entry> = generate_catalogue()
        .or_fail()?
        .into_iter()
        .take(3)
        .collect();
    let mut lockfile = Vec::new();
    for entry in &catalogue {
        let digests = digests_for(entry, Payload(b"expected")).or_fail()?;
        lockfile.push(digests.manifest);
        lockfile.push(digests.media);
    }
    Ok((catalogue, lockfile))
}

fn names_of(catalogue: &[Entry]) -> Vec<EntryName> {
    catalogue.iter().map(EntryName::of_entry).collect()
}

#[test]
fn missing_entries_all_selection_reports_missing() {
    assert_eq!(
        check_missing_entries_all_selection_reports_missing(),
        Ok(())
    );
}

fn check_missing_entries_all_selection_reports_missing() -> Result<(), Fail> {
    let lockfile = vec![lock_entry(
        LockfileName::new("test.json").or_fail()?,
        Payload(b"test"),
    )?];
    let missing = missing_entries(&lockfile, &BTreeSet::new(), &EntrySelection::All);
    ensure_eq!(
        missing,
        vec![LockfileDifference::Missing {
            name: LockfileName::new("test.json").or_fail()?
        }]
    );
    Ok(())
}

#[test]
fn missing_entries_all_selection_reports_names_that_belong_to_no_entry() {
    assert_eq!(
        check_missing_entries_all_selection_reports_names_that_belong_to_no_entry(),
        Ok(())
    );
}

fn check_missing_entries_all_selection_reports_names_that_belong_to_no_entry() -> Result<(), Fail> {
    let lockfile = vec![lock_entry(
        LockfileName::new("notes.txt").or_fail()?,
        Payload(b"x"),
    )?];
    let missing = missing_entries(&lockfile, &BTreeSet::new(), &EntrySelection::All);
    ensure_eq!(
        missing,
        vec![LockfileDifference::Missing {
            name: LockfileName::new("notes.txt").or_fail()?
        }]
    );
    Ok(())
}

#[test]
fn missing_entries_only_selection_skips_unselected() {
    assert_eq!(
        check_missing_entries_only_selection_skips_unselected(),
        Ok(())
    );
}

fn check_missing_entries_only_selection_skips_unselected() -> Result<(), Fail> {
    let lockfile = vec![
        lock_entry(
            LockfileName::new("test1.json").or_fail()?,
            Payload(b"test1"),
        )?,
        lock_entry(
            LockfileName::new("test2.json").or_fail()?,
            Payload(b"test2"),
        )?,
        lock_entry(LockfileName::new("notes.txt").or_fail()?, Payload(b"x"))?,
    ];
    let requested = Requested::Named(vec![RequestedName::new("test1".to_owned())]);
    let selection = EntrySelection::resolve(&requested, &lockfile).or_fail()?;
    let missing = missing_entries(&lockfile, &BTreeSet::new(), &selection);
    ensure_eq!(
        missing,
        vec![LockfileDifference::Missing {
            name: LockfileName::new("test1.json").or_fail()?
        }]
    );
    Ok(())
}

#[test]
fn compare_entry_digest_and_length_mismatch() {
    assert_eq!(check_compare_entry_digest_and_length_mismatch(), Ok(()));
}

fn check_compare_entry_digest_and_length_mismatch() -> Result<(), Fail> {
    let name = LockfileName::new("test.json").or_fail()?;
    let expected = lock_entry(name.clone(), Payload(b"expected"))?;
    let actual = lock_entry(name.clone(), Payload(b"actual"))?;
    let diffs = compare_entry(Some(&expected), &actual);
    ensure_eq!(
        diffs,
        vec![
            LockfileDifference::DigestMismatch {
                name: name.clone(),
                expected: sha256(b"expected"),
                actual: sha256(b"actual"),
            },
            LockfileDifference::LengthMismatch {
                name,
                expected: ContentLength::new(8),
                actual: ContentLength::new(6),
            },
        ]
    );
    Ok(())
}

#[test]
fn compare_entry_length_mismatch_only() {
    assert_eq!(check_compare_entry_length_mismatch_only(), Ok(()));
}

fn check_compare_entry_length_mismatch_only() -> Result<(), Fail> {
    let name = LockfileName::new("test.json").or_fail()?;
    let expected = LockfileEntry::new(name.clone(), sha256(b"same"), ContentLength::new(10));
    let actual = LockfileEntry::new(name.clone(), sha256(b"same"), ContentLength::new(5));
    let diffs = compare_entry(Some(&expected), &actual);
    ensure_eq!(
        diffs,
        vec![LockfileDifference::LengthMismatch {
            name,
            expected: ContentLength::new(10),
            actual: ContentLength::new(5),
        }]
    );
    Ok(())
}

#[test]
fn compare_entry_extra() {
    assert_eq!(check_compare_entry_extra(), Ok(()));
}

fn check_compare_entry_extra() -> Result<(), Fail> {
    let name = LockfileName::new("test.json").or_fail()?;
    let actual = lock_entry(name.clone(), Payload(b"actual"))?;
    let diffs = compare_entry(None, &actual);
    ensure_eq!(diffs, vec![LockfileDifference::Extra { name }]);
    Ok(())
}

#[test]
fn compare_entry_no_differences() {
    assert_eq!(check_compare_entry_no_differences(), Ok(()));
}

fn check_compare_entry_no_differences() -> Result<(), Fail> {
    let name = LockfileName::new("test.json").or_fail()?;
    let expected = lock_entry(name.clone(), Payload(b"same"))?;
    let actual = lock_entry(name, Payload(b"same"))?;
    ensure!(compare_entry(Some(&expected), &actual).is_empty());
    Ok(())
}

#[test]
fn mismatch_survives_a_later_generation_failure() {
    assert_eq!(check_mismatch_survives_a_later_generation_failure(), Ok(()));
}

fn check_mismatch_survives_a_later_generation_failure() -> Result<(), Fail> {
    let (catalogue, lockfile) = three_entries_and_lockfile()?;
    let names = names_of(&catalogue);
    let changed = EntryFiles::of(catalogue.first().or_fail()?).or_fail()?;
    let first = names.first().or_fail()?.clone();
    let second = names.get(1).or_fail()?.clone();
    let third = names.get(2).or_fail()?.clone();

    let report = verify_entries_regenerated(catalogue, &lockfile, &EntrySelection::All, |entry| {
        match EntryName::of_entry(entry) {
            name if name == first => digests_for(entry, Payload(b"changed")),
            name if name == second => Err(EntryDigestError::Generation(EntryError::Render(
                manifest_construction_failure(),
            ))),
            _ => digests_for(entry, Payload(b"expected")),
        }
    });

    ensure_eq!(report.checked, EntryCount(2));
    ensure_eq!(report.failures.0.len(), 1);
    ensure_eq!(report.failures.0.first().or_fail()?.entry_name, second);
    // Entry 0's manifest and media each differ in digest and length; entry 1's files are
    // declared by the catalogue, so its failure is not also reported as missing.
    let mut expected = Vec::new();
    for name in [changed.manifest, changed.media] {
        expected.push(LockfileDifference::DigestMismatch {
            name: name.clone(),
            expected: sha256(b"expected"),
            actual: sha256(b"changed"),
        });
        expected.push(LockfileDifference::LengthMismatch {
            name,
            expected: ContentLength::new(8),
            actual: ContentLength::new(7),
        });
    }
    ensure_eq!(report.differences.0, expected);

    // `ok` is reported for the entry that matched and not for the one that differed.
    ensure_eq!(report.matched.0, vec![third]);
    Ok(())
}

#[test]
fn only_selection_generates_just_the_named_entries() {
    assert_eq!(
        check_only_selection_generates_just_the_named_entries(),
        Ok(())
    );
}

fn check_only_selection_generates_just_the_named_entries() -> Result<(), Fail> {
    let (catalogue, lockfile) = three_entries_and_lockfile()?;
    let names = names_of(&catalogue);
    let wanted = names.get(2).or_fail()?.clone();
    let requested = Requested::Named(vec![RequestedName::new(wanted.to_string())]);
    let selection = EntrySelection::resolve(&requested, &lockfile).or_fail()?;
    let generated: RefCell<Vec<EntryName>> = RefCell::new(Vec::new());

    let report = verify_entries_regenerated(catalogue, &lockfile, &selection, |entry| {
        generated.borrow_mut().push(EntryName::of_entry(entry));
        digests_for(entry, Payload(b"expected"))
    });

    ensure_eq!(*generated.borrow(), vec![wanted]);
    ensure_eq!(report.checked, EntryCount(1));
    ensure!(report.failures.0.is_empty());
    ensure!(report.differences.0.is_empty());
    Ok(())
}

#[test]
fn write_failure_is_reported_not_swallowed() {
    assert_eq!(check_write_failure_is_reported_not_swallowed(), Ok(()));
}

fn check_write_failure_is_reported_not_swallowed() -> Result<(), Fail> {
    // A pipe whose reading end is closed refuses every write with a broken pipe.
    let (reader, mut broken) = io::pipe().or_fail()?;
    drop(reader);
    let (catalogue, lockfile) = three_entries_and_lockfile()?;
    let report = verify_entries_regenerated(catalogue, &lockfile, &EntrySelection::All, |entry| {
        digests_for(entry, Payload(b"expected"))
    });
    let result = execute_with(report.into_report(), &mut broken, &mut Vec::<u8>::new());
    ensure_eq!(
        result.map_err(|error| error.kind()),
        Err(io::ErrorKind::BrokenPipe)
    );
    Ok(())
}

#[test]
fn entry_name_of_lockfile_name_covers_manifest_and_media() {
    assert_eq!(
        check_entry_name_of_lockfile_name_covers_manifest_and_media(),
        Ok(())
    );
}

fn check_entry_name_of_lockfile_name_covers_manifest_and_media() -> Result<(), Fail> {
    let manifest =
        EntryName::of_lockfile_name(&LockfileName::new("a-b.json").or_fail()?).or_fail()?;
    let iso =
        EntryName::of_lockfile_name(&LockfileName::new("a-b-media.mp4").or_fail()?).or_fail()?;
    let avi =
        EntryName::of_lockfile_name(&LockfileName::new("a-b-media.avi").or_fail()?).or_fail()?;
    ensure_eq!(manifest.to_string(), "a-b");
    ensure_eq!(iso, manifest);
    ensure_eq!(avi, manifest);
    ensure_eq!(
        EntryName::of_lockfile_name(&LockfileName::new("notes.txt").or_fail()?),
        None
    );
    Ok(())
}

#[test]
fn entry_files_map_back_to_their_entry_name() {
    assert_eq!(check_entry_files_map_back_to_their_entry_name(), Ok(()));
}

fn check_entry_files_map_back_to_their_entry_name() -> Result<(), Fail> {
    let (catalogue, _) = three_entries_and_lockfile()?;
    for entry in &catalogue {
        let files = EntryFiles::of(entry).or_fail()?;
        let expected = EntryName::of_entry(entry);
        ensure_eq!(
            EntryName::of_lockfile_name(&files.manifest),
            Some(expected.clone())
        );
        ensure_eq!(EntryName::of_lockfile_name(&files.media), Some(expected));
    }
    Ok(())
}

#[test]
fn resolve_rejects_a_name_the_lockfile_does_not_contain() {
    assert_eq!(
        check_resolve_rejects_a_name_the_lockfile_does_not_contain(),
        Ok(())
    );
}

fn check_resolve_rejects_a_name_the_lockfile_does_not_contain() -> Result<(), Fail> {
    let (_, lockfile) = three_entries_and_lockfile()?;
    let requested = Requested::Named(vec![RequestedName::new("nope".to_owned())]);
    let result = EntrySelection::resolve(&requested, &lockfile);
    ensure_eq!(
        result,
        Err(UnknownEntryName {
            name: RequestedName::new("nope".to_owned())
        })
    );
    Ok(())
}

#[test]
fn resolve_everything_covers_every_entry() {
    assert_eq!(check_resolve_everything_covers_every_entry(), Ok(()));
}

fn check_resolve_everything_covers_every_entry() -> Result<(), Fail> {
    let (catalogue, lockfile) = three_entries_and_lockfile()?;
    let selection = EntrySelection::resolve(&Requested::Everything, &lockfile).or_fail()?;
    for entry in &catalogue {
        ensure_eq!(
            selection.covers_entry(&EntryName::of_entry(entry)),
            Coverage::Covered
        );
    }
    Ok(())
}

#[test]
fn first_duplicate_name_finds_a_repeated_lockfile_name() {
    assert_eq!(
        check_first_duplicate_name_finds_a_repeated_lockfile_name(),
        Ok(())
    );
}

fn check_first_duplicate_name_finds_a_repeated_lockfile_name() -> Result<(), Fail> {
    let first = LockfileName::new("a.json").or_fail()?;
    let lockfile = vec![
        lock_entry(first.clone(), Payload(b"1"))?,
        lock_entry(LockfileName::new("b.json").or_fail()?, Payload(b"2"))?,
        lock_entry(first.clone(), Payload(b"3"))?,
    ];
    ensure_eq!(first_duplicate_name(&lockfile), Some(&first));
    ensure_eq!(first_duplicate_name(lockfile.get(..2).or_fail()?), None);
    Ok(())
}

#[test]
fn error_chain_shows_each_cause_once() {
    let error = EntryDigestError::Generation(EntryError::Render(manifest_construction_failure()));
    let inner = EntryError::Render(manifest_construction_failure()).to_string();
    let chain = ErrorChain(&error).to_string();
    assert!(chain.starts_with("entry generation failed: "), "{chain}");
    assert_eq!(chain.matches(inner.as_str()).count(), 1, "{chain}");
}

#[test]
fn regenerate_matching_entry_is_reported_on_stdout() {
    assert_eq!(
        check_regenerate_matching_entry_is_reported_on_stdout(),
        Ok(())
    );
}

fn check_regenerate_matching_entry_is_reported_on_stdout() -> Result<(), Fail> {
    let catalogue = generate_catalogue().or_fail()?;
    let first = catalogue.first().or_fail()?;
    let name = EntryName::of_entry(first);
    let digests = entry_digests(first).or_fail()?;
    let lockfile = synthvid_catalog::generate_lockfile(vec![digests.manifest, digests.media]);
    let loaded = LoadedVerify::Regenerate {
        requested: Requested::Named(vec![RequestedName::new(name.to_string())]),
        lockfile: Ok(lockfile),
    };
    let report = plan_loaded(loaded);
    ensure_eq!(
        report.trace().0,
        [
            format!("stdout {:?}", format!("ok {name}\n")),
            format!("stdout {:?}", "verified 1 entries\n"),
        ]
    );
    ensure_eq!(report.outcome, Outcome::Success);
    Ok(())
}

#[test]
fn regenerated_report_lists_each_match_then_the_summary() {
    assert_eq!(
        check_regenerated_report_lists_each_match_then_the_summary(),
        Ok(())
    );
}

fn check_regenerated_report_lists_each_match_then_the_summary() -> Result<(), Fail> {
    let (catalogue, lockfile) = three_entries_and_lockfile()?;
    let names = names_of(&catalogue);
    let report = verify_entries_regenerated(catalogue, &lockfile, &EntrySelection::All, |entry| {
        digests_for(entry, Payload(b"expected"))
    })
    .into_report();
    let mut expected: Vec<String> = names
        .iter()
        .map(|name| format!("stdout {:?}", format!("ok {name}\n")))
        .collect();
    expected.push(format!("stdout {:?}", "verified 3 entries\n"));
    ensure_eq!(report.trace().0, expected);
    ensure_eq!(report.outcome, Outcome::Success);
    Ok(())
}

#[test]
fn parse_reports_a_flag_without_its_value() {
    assert_eq!(
        parse_verify_args(Argv(&["--lock".to_owned()])),
        Err(UsageError::MissingValue(Flag::Lock))
    );
}

#[test]
fn parse_does_not_take_a_flag_as_a_value() {
    assert_eq!(
        parse_verify_args(Argv(&[
            "--lock".to_owned(),
            "--only".to_owned(),
            "x".to_owned()
        ])),
        Err(UsageError::MissingValue(Flag::Lock))
    );
}

#[test]
fn parse_reports_an_unknown_option() {
    assert_eq!(
        parse_verify_args(Argv(&["--bogus".to_owned()])),
        Err(UsageError::UnknownOption("--bogus".to_owned()))
    );
}

#[test]
fn parse_rejects_corpus_with_regeneration_flags() {
    assert_eq!(
        parse_verify_args(Argv(&[
            "--corpus".to_owned(),
            "d".to_owned(),
            "--only".to_owned(),
            "x".to_owned()
        ])),
        Err(UsageError::CorpusWithRegenerationFlags)
    );
}

#[test]
fn usage_error_is_reported_without_loading_anything() {
    let loaded_called = Cell::new(false);
    let report = plan_verify(Argv(&["--bogus".to_owned()]), |_| {
        loaded_called.set(true);
        LoadedVerify::Corpus(CorpusSnapshot::Unreadable(InputError::LockfileRead(
            io::Error::other("unused"),
        )))
    });
    assert!(!loaded_called.get());
    assert_eq!(report.trace().0, ["stderr unknown option: --bogus"]);
    assert_eq!(report.outcome, Outcome::Failure);
}
