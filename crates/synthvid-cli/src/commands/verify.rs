//! Verification: the pure core that compares a corpus or a regenerated catalogue
//! with a lockfile.
//!
//! Nothing here reads a file or writes a line. `plan_verify` takes the inputs
//! the shell has already loaded and returns a [`Report`]; `shell.rs` carries it
//! out.

extern crate alloc;

use super::entry_digests::{entry_digests, EntryDigestError, EntryDigests};
use super::entry_name::{EntryFiles, EntryName, EntrySelection, Requested};
use super::message::Message;
use super::modes::Coverage;
use super::report::{OutText, Report, Step};
use super::verify_args::{parse_verify_args, VerifyMode};
use super::verify_input::{CorpusFiles, CorpusSnapshot, InputError, LoadedVerify, LockfileText};
use super::verify_values::EntryCount;
use super::{Argv, Outcome};
use alloc::collections::{BTreeMap, BTreeSet};
use synthvid_catalog::{
    cover::{generate_catalogue, Entry},
    parse_lockfile, ArtifactBytes, CorpusArtifact, LockfileDifference, LockfileEntry, LockfileName,
    RawName,
};

/// An entry that could not be checked, and why.
#[derive(Debug)]
pub(super) struct RegenerationFailure {
    /// The entry that could not be checked.
    pub(super) entry_name: EntryName,
    /// Why it could not be checked.
    pub(super) error: EntryDigestError,
}

/// The differences a regeneration check found, in the order they are reported.
pub(super) struct Differences(pub(super) Vec<LockfileDifference>);

/// The entries a regeneration check could not check, in catalogue order.
pub(super) struct Failures(pub(super) Vec<RegenerationFailure>);

/// The entries whose two files both matched, in catalogue order.
pub(super) struct Matched(pub(super) Vec<EntryName>);

/// What comparing regenerated entries with a lockfile found.
pub(super) struct RegenerationReport {
    /// All differences found between generated and expected entries.
    pub(super) differences: Differences,
    /// Entries that could not be checked.
    pub(super) failures: Failures,
    /// Entries whose two files both matched.
    pub(super) matched: Matched,
    /// Count of entries that were generated and compared.
    pub(super) checked: EntryCount,
}

impl RegenerationReport {
    /// The steps that print this report: an `ok` line on stdout for each
    /// matching entry, each difference and failure on stderr, and the summary
    /// on stdout.
    fn into_report(self) -> Report {
        let outcome = if self.differences.0.is_empty() && self.failures.0.is_empty() {
            Outcome::Success
        } else {
            Outcome::Failure
        };
        let summary = if self.failures.0.is_empty() {
            format!("verified {} entries\n", self.checked)
        } else {
            format!(
                "verified {} entries, {} could not be checked\n",
                self.checked,
                self.failures.0.len()
            )
        };

        let mut steps: Vec<Step> = self
            .matched
            .0
            .into_iter()
            .map(|name| Step::Stdout(OutText(format!("ok {name}\n"))))
            .collect();
        steps.extend(
            self.differences
                .0
                .into_iter()
                .map(|difference| Step::Stderr(Message::Difference(difference))),
        );
        steps.extend(self.failures.0.into_iter().map(|failure| {
            Step::Stderr(Message::FailedToCheck {
                name: failure.entry_name,
                error: failure.error,
            })
        }));
        steps.push(Step::Stdout(OutText(summary)));
        Report::new(steps, outcome)
    }
}

/// Plans a verify run. `load` performs the reads that the arguments call for;
/// a usage error is reported before anything is read.
pub(super) fn plan_verify(args: Argv<'_>, load: impl FnOnce(VerifyMode) -> LoadedVerify) -> Report {
    match parse_verify_args(args) {
        Ok(parsed) => plan_loaded(load(parsed.mode)),
        Err(error) => Report::failed(Message::VerifyUsage(error)),
    }
}

/// Plans the report for inputs that have already been loaded.
fn plan_loaded(loaded: LoadedVerify) -> Report {
    match loaded {
        LoadedVerify::Corpus(snapshot) => plan_corpus(snapshot),
        LoadedVerify::Regenerate {
            requested,
            lockfile,
        } => plan_regenerate(lockfile, &requested),
    }
}

/// Plans a corpus check. The first failure to read the lockfile, parse it, or
/// read the directory ends the check; otherwise every difference is one stderr
/// line.
fn plan_corpus(snapshot: CorpusSnapshot) -> Report {
    let (lockfile, listing) = match snapshot {
        CorpusSnapshot::Unreadable(cause) => return Report::failed(Message::Input(cause)),
        CorpusSnapshot::Listed { lockfile, files } => (lockfile, files),
    };
    let entries = match parse_entries(&lockfile) {
        Ok(entries) => entries,
        Err(message) => return Report::failed(*message),
    };
    let files = match listing {
        Ok(files) => files,
        Err(cause) => return Report::failed(Message::Input(cause)),
    };
    corpus_report(&entries, files)
}

/// Compares a corpus's files with the lockfile's entries.
fn corpus_report(entries: &[LockfileEntry], files: CorpusFiles) -> Report {
    let differences = synthvid_catalog::verify(
        entries,
        files.0.into_iter().map(|file| CorpusArtifact {
            name: RawName(file.name.0),
            bytes: ArtifactBytes(file.bytes.0),
        }),
    );
    let outcome = if differences.is_empty() {
        Outcome::Success
    } else {
        Outcome::Failure
    };
    let steps = differences
        .into_iter()
        .map(|difference| Step::Stderr(Message::Difference(difference)))
        .collect();
    Report::new(steps, outcome)
}

/// Plans an in-memory regeneration check of the requested entries.
fn plan_regenerate(lockfile: Result<LockfileText, InputError>, requested: &Requested) -> Report {
    let entries = match checked_entries(lockfile) {
        Ok(entries) => entries,
        Err(message) => return Report::failed(*message),
    };
    let selection = match EntrySelection::resolve(requested, &entries) {
        Ok(selection) => selection,
        Err(unknown) => return Report::failed(Message::UnknownEntry(unknown.name)),
    };
    let catalogue = match generate_catalogue() {
        Ok(catalogue) => catalogue,
        Err(cause) => return Report::failed(Message::CatalogFailed(cause)),
    };
    verify_entries_regenerated(catalogue, &entries, &selection, entry_digests).into_report()
}

/// Parses a lockfile's text into its entries.
fn parse_entries(text: &LockfileText) -> Result<Vec<LockfileEntry>, Box<Message>> {
    parse_lockfile(text).map_err(|cause| Box::new(Message::LockfileParse(cause)))
}

/// Reads the entries of a lockfile, refusing one that lists a name twice.
fn checked_entries(
    lockfile: Result<LockfileText, InputError>,
) -> Result<Vec<LockfileEntry>, Box<Message>> {
    lockfile
        .map_err(|cause| Box::new(Message::Input(cause)))
        .and_then(|text| parse_entries(&text))
        .and_then(unique_entries)
}

/// The entries, unless a name is listed twice.
fn unique_entries(entries: Vec<LockfileEntry>) -> Result<Vec<LockfileEntry>, Box<Message>> {
    if let Some(name) = first_duplicate_name(&entries).cloned() {
        return Err(Box::new(Message::LockfileDuplicate(name)));
    }
    Ok(entries)
}

/// Compares an expected and actual lockfile entry.
fn compare_entry(
    expected: Option<&LockfileEntry>,
    actual: &LockfileEntry,
) -> Vec<LockfileDifference> {
    let mut differences = Vec::new();

    match expected {
        Some(exp_entry) => {
            if actual.digest != exp_entry.digest {
                differences.push(LockfileDifference::DigestMismatch {
                    name: actual.name.clone(),
                    expected: exp_entry.digest,
                    actual: actual.digest,
                });
            }
            if actual.length != exp_entry.length {
                differences.push(LockfileDifference::LengthMismatch {
                    name: actual.name.clone(),
                    expected: exp_entry.length,
                    actual: actual.length,
                });
            }
        }
        None => {
            differences.push(LockfileDifference::Extra {
                name: actual.name.clone(),
            });
        }
    }

    differences
}

/// The first name a lockfile lists twice, if any.
#[must_use]
fn first_duplicate_name(lockfile: &[LockfileEntry]) -> Option<&LockfileName> {
    let mut seen = BTreeSet::new();
    lockfile
        .iter()
        .map(|entry| &entry.name)
        .find(|name| !seen.insert(*name))
}

/// Finds lockfile entries that were not produced by the catalogue.
fn missing_entries(
    lockfile: &[LockfileEntry],
    produced: &BTreeSet<LockfileName>,
    selection: &EntrySelection,
) -> Vec<LockfileDifference> {
    lockfile
        .iter()
        .filter(|entry| selection.covers_file(&entry.name) == Coverage::Covered)
        .filter(|entry| !produced.contains(&entry.name))
        .map(|entry| LockfileDifference::Missing {
            name: entry.name.clone(),
        })
        .collect()
}

/// Compares generated entries against lockfile entries. Each entry the
/// selection covers is generated and its two files compared with the lockfile;
/// a failure to generate it is kept as a failure, not as a missing entry.
fn verify_entries_regenerated<F>(
    catalogue: Vec<Entry>,
    lockfile: &[LockfileEntry],
    selection: &EntrySelection,
    generate: F,
) -> RegenerationReport
where
    F: Fn(&Entry) -> Result<EntryDigests, EntryDigestError>,
{
    let expected: BTreeMap<&LockfileName, &LockfileEntry> =
        lockfile.iter().map(|entry| (&entry.name, entry)).collect();
    let mut differences: Vec<LockfileDifference> = Vec::new();
    let mut failures: Vec<RegenerationFailure> = Vec::new();
    let mut matched: Vec<EntryName> = Vec::new();
    let mut compared: Vec<EntryName> = Vec::new();
    let mut declared: BTreeSet<LockfileName> = BTreeSet::new();

    for entry in catalogue {
        let entry_name = EntryName::of_entry(&entry);
        if selection.covers_entry(&entry_name) == Coverage::Uncovered {
            continue;
        }

        // The catalogue declares these files whether or not generation succeeds, so a failure
        // is reported once, as a failure, and not again as missing. A name that is not a valid
        // lockfile name is reported by `generate` as the failure.
        if let Ok(files) = EntryFiles::of(&entry) {
            declared.extend([files.manifest, files.media]);
        } else {
            // A failure is reported by `generate` below, not here.
        }

        match generate(&entry) {
            Ok(digests) => {
                let mut entry_differences = compare_entry(
                    expected.get(&digests.manifest.name).copied(),
                    &digests.manifest,
                );
                entry_differences.extend(compare_entry(
                    expected.get(&digests.media.name).copied(),
                    &digests.media,
                ));
                compared.push(entry_name.clone());
                if entry_differences.is_empty() {
                    matched.push(entry_name);
                }
                differences.append(&mut entry_differences);
            }
            Err(error) => failures.push(RegenerationFailure { entry_name, error }),
        }
    }

    differences.append(&mut missing_entries(lockfile, &declared, selection));

    RegenerationReport {
        differences: Differences(differences),
        failures: Failures(failures),
        matched: Matched(matched),
        checked: EntryCount(compared.len()),
    }
}

#[cfg(test)]
mod loaded_tests;
#[cfg(test)]
mod tests;
