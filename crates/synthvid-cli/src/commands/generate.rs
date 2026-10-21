//! The generate subcommand: argument parsing, and the report it computes.

use super::entry_name::EntryName;
use super::message::Message;
use super::modes::Execution;
use super::report::{Artifact, FileBytes, FileName, Flag, Publication, Report, Step, Word};
use super::{Argv, Outcome};
use std::path::{Path, PathBuf};
use synthvid_catalog::cover::{generate_catalogue, Entry};
use synthvid_catalog::generate_entry;

/// Arguments for the generate subcommand.
struct GenerateArgs {
    /// Output directory path.
    out_dir: PathBuf,
    /// Names of entries to generate. Empty means every entry.
    only_names: Vec<Word>,
    /// Whether to write the files or only report them.
    dry_run: Execution,
}

/// Computes the report for the generate subcommand.
pub(super) fn plan_generate(args: Argv<'_>) -> Report {
    match parse_generate_args(args) {
        Ok(parsed) => plan_generate_run(&parsed),
        Err(report) => report,
    }
}

/// Parses arguments for the generate subcommand. The `Err` carries the report
/// of the usage error.
fn parse_generate_args(args: Argv<'_>) -> Result<GenerateArgs, Report> {
    let mut out_value: Option<PathBuf> = None;
    let mut only_names: Vec<Word> = Vec::new();
    let mut dry_run = Execution::Run;

    let mut args_iter = args.0.iter();
    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--out" => {
                if let Some(value) = args_iter.next() {
                    out_value = Some(PathBuf::from(value));
                } else {
                    return Err(Report::failed(Message::MissingValue(Flag::Out)));
                }
            }
            "--only" => {
                if let Some(value) = args_iter.next() {
                    only_names.push(Word(value.clone()));
                } else {
                    return Err(Report::failed(Message::MissingValue(Flag::Only)));
                }
            }
            "--dry-run" => {
                dry_run = Execution::DryRun;
            }
            _ => {
                return Err(Report::failed(Message::UnknownOption(Word(arg.clone()))));
            }
        }
    }

    let Some(out_dir) = out_value else {
        return Err(Report::failed(Message::MissingFlag(Flag::Out)));
    };

    Ok(GenerateArgs {
        out_dir,
        only_names,
        dry_run,
    })
}

/// Computes the steps of a generate run: the directory to create, then for
/// each selected entry either a would-generate line or the publication of its
/// two files.
fn plan_generate_run(args: &GenerateArgs) -> Report {
    let catalogue = match generate_catalogue() {
        Ok(cat) => cat,
        Err(cause) => return Report::failed(Message::CatalogFailed(cause)),
    };

    let entries_to_generate: Vec<_> = if args.only_names.is_empty() {
        catalogue
    } else {
        catalogue
            .into_iter()
            .filter(|entry| {
                let name = Word(entry.manifest_name().as_str().to_owned());
                args.only_names.contains(&name)
            })
            .collect()
    };

    let mut steps = Vec::new();
    let mut outcome = Outcome::Success;
    if args.dry_run == Execution::Run {
        steps.push(Step::CreateDir(args.out_dir.clone()));
    }

    for entry in entries_to_generate {
        match generate_entry(&entry) {
            Ok(generated) => match args.dry_run {
                Execution::DryRun => {
                    steps.push(Step::Stderr(Message::WouldGenerate(EntryName::of_entry(
                        &entry,
                    ))));
                }
                Execution::Run => {
                    let bytes = EntryBytes {
                        manifest: FileBytes(generated.manifest_json().as_str().as_bytes().to_vec()),
                        media: FileBytes(generated.media_bytes().as_bytes().to_vec()),
                    };
                    steps.push(publish_entry(&args.out_dir, &entry, bytes));
                }
            },
            Err(cause) => {
                steps.push(Step::Stderr(Message::GenerateFailed {
                    name: EntryName::of_entry(&entry),
                    cause,
                }));
                outcome = Outcome::Failure;
            }
        }
    }

    Report::new(steps, outcome)
}

/// The contents of a generated entry's two files.
struct EntryBytes {
    /// The manifest file's bytes.
    manifest: FileBytes,
    /// The media file's bytes.
    media: FileBytes,
}

/// The step that publishes an entry's two files into `out_dir` and then
/// announces them.
fn publish_entry(out_dir: &Path, entry: &Entry, bytes: EntryBytes) -> Step {
    let manifest_name = FileName(format!("{}.json", entry.manifest_name().as_str()));
    let media_name = FileName(entry.media_name().to_string());
    Step::Publish {
        files: vec![
            Publication {
                artifact: Artifact::Manifest,
                dest: out_dir.join(&manifest_name.0),
                bytes: bytes.manifest,
            },
            Publication {
                artifact: Artifact::Media,
                dest: out_dir.join(&media_name.0),
                bytes: bytes.media,
            },
        ],
        announce: Some(Message::Generated {
            manifest: manifest_name,
            media: media_name,
        }),
    }
}

#[cfg(test)]
mod tests;
