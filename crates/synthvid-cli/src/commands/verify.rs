//! Verification command implementation.

use super::entry_digests::{entry_digests, EntryDigestError, EntryDigests};
use super::entry_name::{EntryFiles, EntryName, EntrySelection, Requested, UnknownEntryName};
use super::verify_args::{parse_verify_args, VerifyMode};
use core::fmt::{Display, Formatter, Result as FmtResult};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use synthvid_catalog::{
    cover::{generate_catalogue, CatalogError, Entry},
    parse_lockfile, LockfileDifference, LockfileEntry, LockfileName, ParseLockfileError,
};

/// An entry that could not be checked, and why.
#[derive(Debug)]
pub(super) struct RegenerationFailure {
    /// The entry that could not be checked.
    pub(super) entry_name: EntryName,
    /// Why it could not be checked.
    pub(super) error: EntryDigestError,
}

/// Report from regenerating and comparing entries.
pub(super) struct RegenerationReport {
    /// All differences found between generated and expected entries.
    pub(super) differences: Vec<LockfileDifference>,
    /// Entries that could not be checked.
    pub(super) failures: Vec<RegenerationFailure>,
    /// Count of entries that were generated and compared.
    pub(super) checked: usize,
}

/// The result of a verification run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum VerifyOutcome {
    /// Everything checked matched.
    Success,
    /// A difference, a failure, or an unreadable input was found.
    Failure,
}

impl VerifyOutcome {
    /// The process exit code for this outcome.
    const fn exit_code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
        }
    }
}

/// Why a corpus on disk could not be verified.
#[derive(Debug)]
pub(super) enum CorpusError {
    /// The corpus lockfile could not be read or parsed.
    Lockfile(LockfileLoadError),
    /// The corpus directory could not be listed.
    ReadDirectory(io::Error),
    /// An entry of the corpus directory could not be read.
    ReadDirectoryEntry(io::Error),
    /// A corpus file could not be read.
    ReadFile {
        /// The file's name.
        name: String,
        /// Why reading failed.
        source: io::Error,
    },
    /// Writing the report failed.
    Output(io::Error),
}

impl Display for CorpusError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Lockfile(e) => write!(f, "{e}"),
            Self::ReadDirectory(e) => write!(f, "failed to read corpus directory: {e}"),
            Self::ReadDirectoryEntry(e) => write!(f, "failed to read directory entry: {e}"),
            Self::ReadFile { name, source } => write!(f, "failed to read file {name}: {source}"),
            Self::Output(e) => write!(f, "failed to write output: {e}"),
        }
    }
}

/// Verifies the files of a corpus directory against its `catalog.lock`, writing one line
/// per difference to `err`.
fn run_corpus<E: Write>(corpus_dir: &Path, err: &mut E) -> Result<VerifyOutcome, CorpusError> {
    let lockfile_entries =
        read_lockfile(&corpus_dir.join("catalog.lock")).map_err(CorpusError::Lockfile)?;

    let mut corpus: Vec<(String, Vec<u8>)> = Vec::new();
    for dir_entry in fs::read_dir(corpus_dir).map_err(CorpusError::ReadDirectory)? {
        let dir_entry = dir_entry.map_err(CorpusError::ReadDirectoryEntry)?;
        let name = dir_entry.file_name().to_string_lossy().into_owned();
        if name == "catalog.lock" {
            continue;
        }
        let content = fs::read(dir_entry.path()).map_err(|source| CorpusError::ReadFile {
            name: name.clone(),
            source,
        })?;
        corpus.push((name, content));
    }

    let differences = synthvid_catalog::verify(&lockfile_entries, corpus);
    for difference in &differences {
        writeln!(err, "{difference}").map_err(CorpusError::Output)?;
    }
    Ok(if differences.is_empty() {
        VerifyOutcome::Success
    } else {
        VerifyOutcome::Failure
    })
}

/// Turns a finished run into an outcome, reporting an error on `err`.
fn finish<E: Write>(result: Result<VerifyOutcome, impl Display>, err: &mut E) -> VerifyOutcome {
    match result {
        Ok(outcome) => outcome,
        Err(error) => {
            // Last resort: if `err` itself is unwritable there is nowhere left to report to.
            writeln!(err, "{error}").ok();
            VerifyOutcome::Failure
        }
    }
}

/// Implements disk-based corpus verification.
fn verify_corpus_impl(corpus_dir: &Path) -> VerifyOutcome {
    let mut err = io::stderr().lock();
    let result = run_corpus(corpus_dir, &mut err);
    finish(result, &mut err)
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

/// Why a lockfile could not be used.
#[derive(Debug)]
pub(super) enum LockfileLoadError {
    /// The file could not be read.
    Read(io::Error),
    /// The file is not a valid lockfile.
    Parse(ParseLockfileError),
    /// The same name is listed more than once.
    DuplicateName(LockfileName),
}

impl Display for LockfileLoadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Read(e) => write!(f, "failed to read lockfile: {e}"),
            Self::Parse(e) => write!(f, "failed to parse lockfile: {e}"),
            Self::DuplicateName(name) => {
                write!(f, "lockfile lists {} more than once", name.as_str())
            }
        }
    }
}

/// The first name a lockfile lists twice, if any.
fn first_duplicate_name(lockfile: &[LockfileEntry]) -> Option<&LockfileName> {
    let mut seen = BTreeSet::new();
    lockfile
        .iter()
        .map(|entry| &entry.name)
        .find(|name| !seen.insert(*name))
}

/// Reads and parses a lockfile.
fn read_lockfile(path: &Path) -> Result<Vec<LockfileEntry>, LockfileLoadError> {
    let content = fs::read_to_string(path).map_err(LockfileLoadError::Read)?;
    parse_lockfile(&content).map_err(LockfileLoadError::Parse)
}

/// Reads and parses a lockfile, refusing one that lists a name twice.
fn load_lockfile(path: &Path) -> Result<Vec<LockfileEntry>, LockfileLoadError> {
    let entries = read_lockfile(path)?;
    let duplicate = first_duplicate_name(&entries).cloned();
    duplicate.map_or(Ok(entries), |name| {
        Err(LockfileLoadError::DuplicateName(name))
    })
}

/// Why an in-memory verification could not run to a verdict.
#[derive(Debug)]
pub(super) enum RegenerateError {
    /// The lockfile could not be used.
    Lockfile(LockfileLoadError),
    /// A requested entry name is not in the lockfile.
    UnknownEntry(UnknownEntryName),
    /// The catalogue could not be built.
    Catalogue(CatalogError),
    /// Writing the report failed.
    Output(io::Error),
}

impl Display for RegenerateError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Lockfile(e) => write!(f, "{e}"),
            Self::UnknownEntry(e) => write!(f, "{e}"),
            Self::Catalogue(e) => write!(f, "failed to generate catalog: {e}"),
            Self::Output(e) => write!(f, "failed to write output: {e}"),
        }
    }
}

/// Finds lockfile entries that were not produced by the catalogue.
fn missing_entries(
    lockfile: &[LockfileEntry],
    produced: &BTreeSet<LockfileName>,
    selection: &EntrySelection,
) -> Vec<LockfileDifference> {
    lockfile
        .iter()
        .filter(|entry| selection.covers_file(&entry.name))
        .filter(|entry| !produced.contains(&entry.name))
        .map(|entry| LockfileDifference::Missing {
            name: entry.name.clone(),
        })
        .collect()
}

/// Compares generated entries against lockfile entries, writing `ok NAME` to `progress`
/// for each entry whose files both match.
fn verify_entries_regenerated<F, W>(
    catalogue: Vec<Entry>,
    lockfile: &[LockfileEntry],
    selection: &EntrySelection,
    generate: F,
    progress: &mut W,
) -> io::Result<RegenerationReport>
where
    F: Fn(&Entry) -> Result<EntryDigests, EntryDigestError>,
    W: Write,
{
    let expected: BTreeMap<&LockfileName, &LockfileEntry> =
        lockfile.iter().map(|entry| (&entry.name, entry)).collect();
    let mut differences: Vec<LockfileDifference> = Vec::new();
    let mut failures: Vec<RegenerationFailure> = Vec::new();
    let mut checked: usize = 0;
    let mut declared: BTreeSet<LockfileName> = BTreeSet::new();

    for entry in catalogue {
        let entry_name = EntryName::of_entry(&entry);
        if !selection.covers_entry(&entry_name) {
            continue;
        }

        // The catalogue declares these files whether or not generation succeeds, so a failure
        // is reported once, as a failure, and not again as missing. A name that is not a valid
        // lockfile name is reported by `generate` as the failure.
        if let Ok(files) = EntryFiles::of(&entry) {
            declared.insert(files.manifest);
            declared.insert(files.media);
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
                if entry_differences.is_empty() {
                    writeln!(progress, "ok {entry_name}")?;
                }
                differences.append(&mut entry_differences);
                checked = checked.saturating_add(1);
            }
            Err(error) => failures.push(RegenerationFailure { entry_name, error }),
        }
    }

    differences.append(&mut missing_entries(lockfile, &declared, selection));

    Ok(RegenerationReport {
        differences,
        failures,
        checked,
    })
}

/// An error and its causes, each once, separated by colons.
fn error_chain(error: &(dyn core::error::Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(inner) = cause {
        text.push_str(": ");
        text.push_str(&inner.to_string());
        cause = inner.source();
    }
    text
}

/// Runs the in-memory verification, writing results to `out` and problems to `err`.
fn run_regenerate<O: Write, E: Write>(
    lock_file: &Path,
    requested: &Requested,
    out: &mut O,
    err: &mut E,
) -> Result<VerifyOutcome, RegenerateError> {
    let lockfile_entries = load_lockfile(lock_file).map_err(RegenerateError::Lockfile)?;
    let selection = EntrySelection::resolve(requested, &lockfile_entries)
        .map_err(RegenerateError::UnknownEntry)?;
    let catalogue = generate_catalogue().map_err(RegenerateError::Catalogue)?;

    let report =
        verify_entries_regenerated(catalogue, &lockfile_entries, &selection, entry_digests, out)
            .map_err(RegenerateError::Output)?;

    for diff in &report.differences {
        writeln!(err, "{diff}").map_err(RegenerateError::Output)?;
    }
    for failure in &report.failures {
        writeln!(
            err,
            "failed to check {}: {}",
            failure.entry_name,
            error_chain(&failure.error)
        )
        .map_err(RegenerateError::Output)?;
    }

    if report.failures.is_empty() {
        writeln!(out, "verified {} entries", report.checked)
    } else {
        writeln!(
            out,
            "verified {} entries, {} could not be checked",
            report.checked,
            report.failures.len()
        )
    }
    .map_err(RegenerateError::Output)?;

    Ok(
        if report.differences.is_empty() && report.failures.is_empty() {
            VerifyOutcome::Success
        } else {
            VerifyOutcome::Failure
        },
    )
}

/// Implements in-memory regeneration verification.
fn verify_regenerate_impl(lock_file: &Path, requested: &Requested) -> VerifyOutcome {
    let mut out = io::stdout().lock();
    let mut err = io::stderr().lock();
    let result = run_regenerate(lock_file, requested, &mut out, &mut err);
    finish(result, &mut err)
}

/// Dispatches verify subcommand to the appropriate implementation.
pub(super) fn dispatch(mode: VerifyMode) -> i32 {
    let outcome = match mode {
        VerifyMode::Corpus(corpus_dir) => verify_corpus_impl(&corpus_dir),
        VerifyMode::Regenerate {
            lock_file,
            requested,
        } => verify_regenerate_impl(&lock_file, &requested),
    };
    outcome.exit_code()
}

/// Runs the verify subcommand: parses its arguments and dispatches.
pub(super) fn run(args: &[String]) -> i32 {
    match parse_verify_args(args) {
        Ok(parsed) => dispatch(parsed.mode),
        Err(error) => finish(Err(error), &mut io::stderr().lock()).exit_code(),
    }
}

#[cfg(test)]
mod tests;
