//! The lock subcommand: argument parsing, and the report it computes.

use super::entry_digests::entry_digests;
use super::entry_name::EntryName;
use super::message::Message;
use super::modes::LockAction;
use super::report::{Artifact, FileBytes, Flag, OutText, Publication, Report, Step, Word};
use super::{Argv, Outcome};
use std::path::PathBuf;
use synthvid_catalog::cover::generate_catalogue;
use synthvid_catalog::generate_lockfile;

/// Arguments for the lock subcommand.
struct LockArgs {
    /// Corpus directory path.
    corpus_dir: PathBuf,
    /// What to do with the lockfile.
    action: LockAction,
}

/// Computes the report for the lock subcommand.
pub(super) fn plan_lock(args: Argv<'_>) -> Report {
    match parse_lock_args(args) {
        Ok(parsed) => plan_lock_run(&parsed),
        Err(report) => report,
    }
}

/// Parses arguments for the lock subcommand. The `Err` carries the report of
/// the usage error.
fn parse_lock_args(args: Argv<'_>) -> Result<LockArgs, Report> {
    let mut corpus_value: Option<PathBuf> = None;
    let mut write = false;
    let mut dry_run = false;

    let mut args_iter = args.0.iter();
    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--corpus" => {
                if let Some(value) = args_iter.next() {
                    corpus_value = Some(PathBuf::from(value));
                } else {
                    return Err(Report::failed(Message::MissingValue(Flag::Corpus)));
                }
            }
            "--write" => {
                write = true;
            }
            "--dry-run" => {
                dry_run = true;
            }
            _ => {
                return Err(Report::failed(Message::UnknownOption(Word(arg.clone()))));
            }
        }
    }

    let Some(corpus_dir) = corpus_value else {
        return Err(Report::failed(Message::MissingFlag(Flag::Corpus)));
    };

    let action = match (write, dry_run) {
        (false, _) => LockAction::Print,
        (true, false) => LockAction::Write,
        (true, true) => LockAction::DryRun,
    };

    Ok(LockArgs { corpus_dir, action })
}

/// Computes the report for a lock run: the lockfile text is built from every
/// entry's digests, then printed, written, or dropped according to the action.
fn plan_lock_run(args: &LockArgs) -> Report {
    let catalogue = match generate_catalogue() {
        Ok(cat) => cat,
        Err(cause) => return Report::failed(Message::CatalogFailed(cause)),
    };

    let mut lockfile_entries = Vec::new();
    for entry in catalogue {
        match entry_digests(&entry) {
            Ok(digests) => {
                lockfile_entries.push(digests.manifest);
                lockfile_entries.push(digests.media);
            }
            Err(cause) => {
                return Report::failed(Message::DigestFailed {
                    name: EntryName::of_entry(&entry),
                    cause,
                });
            }
        }
    }

    let lockfile_content = generate_lockfile(lockfile_entries);

    let steps = match args.action {
        LockAction::Print => vec![Step::Stdout(OutText(lockfile_content.0))],
        LockAction::DryRun => Vec::new(),
        LockAction::Write => vec![Step::Publish {
            files: vec![Publication {
                artifact: Artifact::Lockfile,
                dest: args.corpus_dir.join("catalog.lock"),
                bytes: FileBytes(lockfile_content.0.into_bytes()),
            }],
            announce: None,
        }],
    };

    Report::new(steps, Outcome::Success)
}
