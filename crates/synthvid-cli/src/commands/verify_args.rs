//! Command-line arguments of the verify subcommand.

use super::entry_name::{Requested, RequestedName};
use core::fmt::{Display, Formatter, Result as FmtResult};
use core::iter::Peekable;
use std::path::PathBuf;

/// A flag the verify subcommand takes a value for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Flag {
    /// `--corpus DIR`
    Corpus,
    /// `--lock FILE`
    Lock,
    /// `--only NAME`
    Only,
}

impl Flag {
    /// The flag as typed.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Corpus => "--corpus",
            Self::Lock => "--lock",
            Self::Only => "--only",
        }
    }
}

/// A mistake in the verify arguments.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum UsageError {
    /// A flag that needs a value was last.
    MissingValue(Flag),
    /// An argument that is not a verify flag.
    UnknownOption(String),
    /// `--corpus` together with a flag that only applies to regeneration.
    CorpusWithRegenerationFlags,
}

impl Display for UsageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::MissingValue(flag) => write!(f, "{} requires an argument", flag.as_str()),
            Self::UnknownOption(arg) => write!(f, "unknown option: {arg}"),
            Self::CorpusWithRegenerationFlags => {
                f.write_str("--corpus cannot be combined with --only or --lock")
            }
        }
    }
}

/// Verification mode: either disk-based or in-memory regeneration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum VerifyMode {
    /// Disk-based verification against an existing corpus directory.
    Corpus(PathBuf),
    /// In-memory regeneration verification using a lockfile.
    Regenerate {
        /// Path to the lockfile.
        lock_file: PathBuf,
        /// The entries asked for, not yet checked against the lockfile.
        requested: Requested,
    },
}

/// Parsed arguments for the verify subcommand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct VerifyArgs {
    /// The verification mode: disk-based or in-memory.
    pub(super) mode: VerifyMode,
}

/// The value after a flag. A following argument that is itself a flag is not a value.
fn value_of<'a>(
    args: &mut Peekable<impl Iterator<Item = &'a String>>,
    flag: Flag,
) -> Result<String, UsageError> {
    args.next_if(|next| !next.starts_with("--"))
        .cloned()
        .ok_or(UsageError::MissingValue(flag))
}

/// Parses arguments for the verify subcommand.
pub(super) fn parse_verify_args(args: &[String]) -> Result<VerifyArgs, UsageError> {
    let mut corpus_dir = None;
    let mut lock_file = None;
    let mut only_names: Vec<RequestedName> = Vec::new();

    let mut args_iter = args.iter().peekable();
    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--corpus" => corpus_dir = Some(value_of(&mut args_iter, Flag::Corpus)?),
            "--lock" => lock_file = Some(value_of(&mut args_iter, Flag::Lock)?),
            "--only" => only_names.push(RequestedName::new(value_of(&mut args_iter, Flag::Only)?)),
            _ => return Err(UsageError::UnknownOption(arg.clone())),
        }
    }

    let mode = if let Some(dir) = corpus_dir {
        if !only_names.is_empty() || lock_file.is_some() {
            return Err(UsageError::CorpusWithRegenerationFlags);
        }
        VerifyMode::Corpus(PathBuf::from(dir))
    } else {
        let requested = if only_names.is_empty() {
            Requested::Everything
        } else {
            Requested::Named(only_names)
        };
        VerifyMode::Regenerate {
            lock_file: PathBuf::from(lock_file.unwrap_or_else(|| "catalog.lock".to_owned())),
            requested,
        }
    };

    Ok(VerifyArgs { mode })
}
