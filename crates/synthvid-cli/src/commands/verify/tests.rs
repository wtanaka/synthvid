//! Unit tests for the verify command.

use super::super::entry_name::RequestedName;
use super::super::verify_args::{parse_verify_args, Flag, UsageError};
use super::*;
use core::cell::RefCell;
use synthvid_catalog::{sha256, ContentLength, GenerationError};

fn lockfile_name(text: &str) -> LockfileName {
    LockfileName::new(text).unwrap()
}

fn length_of(payload: &[u8]) -> ContentLength {
    ContentLength::new(u64::try_from(payload.len()).unwrap())
}

/// A lockfile entry for `name` whose bytes are `payload`.
fn lock_entry(name: &str, payload: &[u8]) -> LockfileEntry {
    LockfileEntry::new(lockfile_name(name), sha256(payload), length_of(payload))
}

fn named(texts: &[&str]) -> Requested {
    Requested::Named(
        texts
            .iter()
            .map(|text| RequestedName::new((*text).to_owned()))
            .collect(),
    )
}

/// The digest pair a generator would report for `entry` if both files held `payload`.
fn digests_for(entry: &Entry, payload: &[u8]) -> EntryDigests {
    let files = EntryFiles::of(entry).unwrap();
    EntryDigests {
        manifest: LockfileEntry::new(files.manifest, sha256(payload), length_of(payload)),
        media: LockfileEntry::new(files.media, sha256(payload), length_of(payload)),
    }
}

/// The first three catalogue entries and a lockfile that expects `b"expected"` for each.
fn three_entries_and_lockfile() -> (Vec<Entry>, Vec<LockfileEntry>) {
    let catalogue: Vec<Entry> = generate_catalogue().unwrap().into_iter().take(3).collect();
    let mut lockfile = Vec::new();
    for entry in &catalogue {
        let digests = digests_for(entry, b"expected");
        lockfile.push(digests.manifest);
        lockfile.push(digests.media);
    }
    (catalogue, lockfile)
}

fn names_of(catalogue: &[Entry]) -> Vec<EntryName> {
    catalogue.iter().map(EntryName::of_entry).collect()
}

#[test]
fn test_missing_entries_all_selection_reports_missing() {
    let lockfile = vec![lock_entry("test.json", b"test")];
    let missing = missing_entries(&lockfile, &BTreeSet::new(), &EntrySelection::All);
    assert_eq!(
        missing,
        vec![LockfileDifference::Missing {
            name: lockfile_name("test.json")
        }]
    );
}

#[test]
fn test_missing_entries_all_selection_reports_names_that_belong_to_no_entry() {
    let lockfile = vec![lock_entry("notes.txt", b"x")];
    let missing = missing_entries(&lockfile, &BTreeSet::new(), &EntrySelection::All);
    assert_eq!(
        missing,
        vec![LockfileDifference::Missing {
            name: lockfile_name("notes.txt")
        }]
    );
}

#[test]
fn test_missing_entries_only_selection_skips_unselected() {
    let lockfile = vec![
        lock_entry("test1.json", b"test1"),
        lock_entry("test2.json", b"test2"),
        lock_entry("notes.txt", b"x"),
    ];
    let selection = EntrySelection::resolve(&named(&["test1"]), &lockfile).unwrap();
    let missing = missing_entries(&lockfile, &BTreeSet::new(), &selection);
    assert_eq!(
        missing,
        vec![LockfileDifference::Missing {
            name: lockfile_name("test1.json")
        }]
    );
}

#[test]
fn test_compare_entry_digest_and_length_mismatch() {
    let expected = lock_entry("test.json", b"expected");
    let actual = lock_entry("test.json", b"actual");
    let diffs = compare_entry(Some(&expected), &actual);
    assert_eq!(diffs.len(), 2);
    assert!(diffs
        .iter()
        .any(|d| matches!(d, LockfileDifference::DigestMismatch { .. })));
    assert!(diffs
        .iter()
        .any(|d| matches!(d, LockfileDifference::LengthMismatch { .. })));
}

#[test]
fn test_compare_entry_length_mismatch_only() {
    let expected = LockfileEntry::new(
        lockfile_name("test.json"),
        sha256(b"same"),
        ContentLength::new(10),
    );
    let actual = LockfileEntry::new(
        lockfile_name("test.json"),
        sha256(b"same"),
        ContentLength::new(5),
    );
    let diffs = compare_entry(Some(&expected), &actual);
    assert!(matches!(
        diffs.as_slice(),
        [LockfileDifference::LengthMismatch { .. }]
    ));
}

#[test]
fn test_compare_entry_extra() {
    let actual = lock_entry("test.json", b"actual");
    let diffs = compare_entry(None, &actual);
    assert_eq!(
        diffs,
        vec![LockfileDifference::Extra {
            name: lockfile_name("test.json")
        }]
    );
}

#[test]
fn test_compare_entry_no_differences() {
    let expected = lock_entry("test.json", b"same");
    let actual = lock_entry("test.json", b"same");
    assert!(compare_entry(Some(&expected), &actual).is_empty());
}

#[test]
fn test_mismatch_survives_a_later_generation_failure() {
    let (catalogue, lockfile) = three_entries_and_lockfile();
    let names = names_of(&catalogue);
    let mut progress: Vec<u8> = Vec::new();

    let report = verify_entries_regenerated(
        catalogue,
        &lockfile,
        &EntrySelection::All,
        |entry| match EntryName::of_entry(entry) {
            name if name == names[0] => Ok(digests_for(entry, b"changed")),
            name if name == names[1] => Err(EntryDigestError::Generation(
                GenerationError::ManifestConstruction,
            )),
            _ => Ok(digests_for(entry, b"expected")),
        },
        &mut progress,
    )
    .unwrap();

    assert_eq!(report.checked, 2);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].entry_name, names[1]);
    // Entry 0's manifest and media each differ in digest and length; entry 1's files are
    // declared by the catalogue, so its failure is not also reported as missing.
    assert_eq!(report.differences.len(), 4);
    assert!(report
        .differences
        .iter()
        .all(|d| !matches!(d, LockfileDifference::Missing { .. })));

    // `ok` is written for the entry that matched and not for the one that differed.
    let progress = String::from_utf8(progress).unwrap();
    assert_eq!(progress, format!("ok {}\n", names[2]));
}

#[test]
fn test_only_selection_generates_just_the_named_entries() {
    let (catalogue, lockfile) = three_entries_and_lockfile();
    let names = names_of(&catalogue);
    let wanted = names[2].to_string();
    let selection = EntrySelection::resolve(&named(&[&wanted]), &lockfile).unwrap();
    let generated: RefCell<Vec<EntryName>> = RefCell::new(Vec::new());
    let mut progress: Vec<u8> = Vec::new();

    let report = verify_entries_regenerated(
        catalogue,
        &lockfile,
        &selection,
        |entry| {
            generated.borrow_mut().push(EntryName::of_entry(entry));
            Ok(digests_for(entry, b"expected"))
        },
        &mut progress,
    )
    .unwrap();

    assert_eq!(*generated.borrow(), vec![names[2].clone()]);
    assert_eq!(report.checked, 1);
    assert!(report.failures.is_empty());
    assert!(report.differences.is_empty());
}

#[test]
fn test_write_failure_is_reported_not_swallowed() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let (catalogue, lockfile) = three_entries_and_lockfile();
    let result = verify_entries_regenerated(
        catalogue,
        &lockfile,
        &EntrySelection::All,
        |entry| Ok(digests_for(entry, b"expected")),
        &mut Broken,
    );
    assert_eq!(
        result.err().map(|e| e.kind()),
        Some(io::ErrorKind::BrokenPipe)
    );
}

#[test]
fn test_entry_name_of_lockfile_name_covers_manifest_and_media() {
    let manifest = EntryName::of_lockfile_name(&lockfile_name("a-b.json")).unwrap();
    let iso = EntryName::of_lockfile_name(&lockfile_name("a-b-media.mp4")).unwrap();
    let avi = EntryName::of_lockfile_name(&lockfile_name("a-b-media.avi")).unwrap();
    assert_eq!(manifest.to_string(), "a-b");
    assert_eq!(iso, manifest);
    assert_eq!(avi, manifest);
    assert_eq!(
        EntryName::of_lockfile_name(&lockfile_name("notes.txt")),
        None
    );
}

#[test]
fn test_entry_files_map_back_to_their_entry_name() {
    let (catalogue, _) = three_entries_and_lockfile();
    for entry in &catalogue {
        let files = EntryFiles::of(entry).unwrap();
        let expected = EntryName::of_entry(entry);
        assert_eq!(
            EntryName::of_lockfile_name(&files.manifest),
            Some(expected.clone())
        );
        assert_eq!(EntryName::of_lockfile_name(&files.media), Some(expected));
    }
}

#[test]
fn test_resolve_rejects_a_name_the_lockfile_does_not_contain() {
    let (_, lockfile) = three_entries_and_lockfile();
    let result = EntrySelection::resolve(&named(&["nope"]), &lockfile);
    assert_eq!(
        result,
        Err(UnknownEntryName {
            name: RequestedName::new("nope".to_owned())
        })
    );
}

#[test]
fn test_resolve_everything_covers_every_entry() {
    let (catalogue, lockfile) = three_entries_and_lockfile();
    let selection = EntrySelection::resolve(&Requested::Everything, &lockfile).unwrap();
    for entry in &catalogue {
        assert!(selection.covers_entry(&EntryName::of_entry(entry)));
    }
}

#[test]
fn test_first_duplicate_name_finds_a_repeated_lockfile_name() {
    let lockfile = vec![
        lock_entry("a.json", b"1"),
        lock_entry("b.json", b"2"),
        lock_entry("a.json", b"3"),
    ];
    assert_eq!(
        first_duplicate_name(&lockfile),
        Some(&lockfile_name("a.json"))
    );
    assert_eq!(first_duplicate_name(&lockfile[..2]), None);
}

#[test]
fn test_error_chain_shows_each_cause_once() {
    let error = EntryDigestError::Generation(GenerationError::ManifestConstruction);
    let inner = GenerationError::ManifestConstruction.to_string();
    let chain = error_chain(&error);
    assert!(chain.starts_with("entry generation failed: "), "{chain}");
    assert_eq!(chain.matches(inner.as_str()).count(), 1, "{chain}");
}

fn args(text: &[&str]) -> Vec<String> {
    text.iter().map(|arg| (*arg).to_owned()).collect()
}

#[test]
fn test_parse_reports_a_flag_without_its_value() {
    assert_eq!(
        parse_verify_args(&args(&["--lock"])),
        Err(UsageError::MissingValue(Flag::Lock))
    );
}

#[test]
fn test_parse_does_not_take_a_flag_as_a_value() {
    assert_eq!(
        parse_verify_args(&args(&["--lock", "--only", "x"])),
        Err(UsageError::MissingValue(Flag::Lock))
    );
}

#[test]
fn test_parse_reports_an_unknown_option() {
    assert_eq!(
        parse_verify_args(&args(&["--bogus"])),
        Err(UsageError::UnknownOption("--bogus".to_owned()))
    );
}

#[test]
fn test_parse_rejects_corpus_with_regeneration_flags() {
    assert_eq!(
        parse_verify_args(&args(&["--corpus", "d", "--only", "x"])),
        Err(UsageError::CorpusWithRegenerationFlags)
    );
}
