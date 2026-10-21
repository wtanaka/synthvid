//! Command-line arguments of the verify subcommand.

use super::entry_name::{Requested, RequestedName};
use super::verify_values::FlagValue;
use super::Argv;
use core::fmt::{Display, Formatter, Result as FmtResult};
use core::iter::Peekable;
use core::slice::Iter;
use std::path::PathBuf;

/// A flag the verify subcommand takes a value for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Flag {
    /// `--corpus DIR`.
    Corpus,
    /// `--lock FILE`.
    Lock,
    /// `--only NAME`.
    Only,
}

/// Writes the flag as typed.
impl Display for Flag {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(match self {
            Self::Corpus => "--corpus",
            Self::Lock => "--lock",
            Self::Only => "--only",
        })
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
            Self::MissingValue(flag) => write!(f, "{flag} requires an argument"),
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

/// The verify arguments not yet read, in the order they were typed.
struct Remaining<'args>(Peekable<Iter<'args, String>>);

/// The value after a flag. A following argument that is itself a flag is not a value.
fn value_of(args: &mut Remaining<'_>, flag: Flag) -> Result<FlagValue, UsageError> {
    args.0
        .next_if(|next| !next.starts_with("--"))
        .cloned()
        .map(FlagValue)
        .ok_or(UsageError::MissingValue(flag))
}

/// Parses arguments for the verify subcommand.
pub(super) fn parse_verify_args(args: Argv<'_>) -> Result<VerifyArgs, UsageError> {
    let mut corpus_dir: Option<String> = None;
    // The lock file used when `--lock` is not given.
    let mut lock_file = PathBuf::from("catalog.lock");
    let mut lock_given = false;
    let mut only_names: Vec<RequestedName> = Vec::new();

    let mut args_iter = Remaining(args.0.iter().peekable());
    while let Some(arg) = args_iter.0.next() {
        match arg.as_str() {
            "--corpus" => corpus_dir = Some(value_of(&mut args_iter, Flag::Corpus)?.0),
            "--lock" => {
                lock_file = PathBuf::from(value_of(&mut args_iter, Flag::Lock)?.0);
                lock_given = true;
            }
            "--only" => {
                only_names.push(RequestedName::new(value_of(&mut args_iter, Flag::Only)?.0));
            }
            _ => return Err(UsageError::UnknownOption(arg.clone())),
        }
    }

    let mode = if let Some(dir) = corpus_dir {
        if !only_names.is_empty() || lock_given {
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
            lock_file,
            requested,
        }
    };

    Ok(VerifyArgs { mode })
}
