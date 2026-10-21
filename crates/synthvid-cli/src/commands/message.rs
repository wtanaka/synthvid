//! Every message a command prints on stderr, as a value that prints itself.
//!
//! The text of each message is fixed here and nowhere else.

use super::entry_digests::EntryDigestError;
use super::entry_name::{EntryName, RequestedName};
use super::error_chain::ErrorChain;
use super::report::{Artifact, FileName, Flag, Word};
use super::verify_args::UsageError;
use super::verify_input::InputError;
use core::fmt::{Display, Formatter, Result as FmtResult};
use std::io;
use synthvid_catalog::cover::CatalogError;
use synthvid_catalog::{EntryError, LockfileDifference, LockfileName, ParseLockfileError};

/// The usage summary. It has no final newline: a stderr line adds it.
const USAGE: &str = concat!(
    "usage: synthvid <command> [options]\n",
    "commands:\n",
    "  generate --out DIR [--only NAME]... [--dry-run]\n",
    "  verify [--corpus DIR] [--lock FILE] [--only NAME]...\n",
    "  list [--format text|json]\n",
    "  manifest --name NAME\n",
    "  lock --corpus DIR [--write] [--dry-run]",
);

/// A filesystem operation that failed, named for the message that reports it.
#[derive(Debug)]
pub(super) enum FsStep {
    /// Creating the output directory.
    CreateDir,
    /// Writing the temporary file for an artifact.
    Write(Artifact),
    /// Renaming the temporary file for an artifact into place.
    Rename(Artifact),
}

impl Display for FsStep {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::CreateDir => f.write_str("failed to create output directory"),
            Self::Write(Artifact::Manifest) => f.write_str("failed to write manifest"),
            Self::Write(Artifact::Media) => f.write_str("failed to write media"),
            Self::Write(Artifact::Lockfile) => f.write_str("failed to write lockfile"),
            Self::Rename(Artifact::Manifest) => f.write_str("failed to rename manifest file"),
            Self::Rename(Artifact::Media) => f.write_str("failed to rename media file"),
            Self::Rename(Artifact::Lockfile) => f.write_str("failed to rename lockfile"),
        }
    }
}

impl Display for Flag {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Out => f.write_str("--out"),
            Self::Only => f.write_str("--only"),
            Self::Corpus => f.write_str("--corpus"),
            Self::Format => f.write_str("--format"),
            Self::Name => f.write_str("--name"),
        }
    }
}

impl Display for Word {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(&self.0)
    }
}

impl Display for FileName {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(&self.0)
    }
}

/// One line of diagnostic output, or the usage summary.
#[derive(Debug)]
pub(super) enum Message {
    /// The usage summary, printed when no command is given.
    Usage,
    /// A flag that takes a value was given without one.
    MissingValue(Flag),
    /// A flag that is required was not given.
    MissingFlag(Flag),
    /// An argument that no option accepts.
    UnknownOption(Word),
    /// A command word that names no subcommand.
    UnknownCommand(Word),
    /// `--format` named a format that does not exist.
    UnknownFormat(Word),
    /// `--name` named no catalogue entry.
    EntryNotFound(Word),
    /// The catalogue could not be generated.
    CatalogFailed(CatalogError),
    /// An entry could not be generated, for `generate`.
    GenerateFailed {
        /// The entry's manifest name.
        name: EntryName,
        /// Why generation failed.
        cause: EntryError,
    },
    /// The manifest of the requested entry could not be generated.
    ManifestFailed(EntryError),
    /// A verify run's arguments were not usable.
    VerifyUsage(UsageError),
    /// A file a verify run needs could not be read.
    Input(InputError),
    /// A lockfile's text could not be parsed.
    LockfileParse(ParseLockfileError),
    /// A lockfile lists the same name more than once.
    LockfileDuplicate(LockfileName),
    /// A requested entry name is not in the lockfile.
    UnknownEntry(RequestedName),
    /// The corpus or the regenerated entries differ from the lockfile.
    Difference(LockfileDifference),
    /// An entry could not be checked against the lockfile.
    FailedToCheck {
        /// The entry's name.
        name: EntryName,
        /// Why it could not be checked.
        error: EntryDigestError,
    },
    /// An entry's lockfile digests could not be computed.
    DigestFailed {
        /// The entry's manifest name.
        name: EntryName,
        /// Why the digests could not be computed.
        cause: EntryDigestError,
    },
    /// A dry run would have generated an entry.
    WouldGenerate(EntryName),
    /// An entry's two files were written.
    Generated {
        /// The manifest file.
        manifest: FileName,
        /// The media file.
        media: FileName,
    },
    /// A filesystem or stdout operation failed.
    Failed {
        /// The operation that failed.
        step: FsStep,
        /// The I/O error the operation returned.
        cause: io::Error,
    },
}

impl Display for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Usage => f.write_str(USAGE),
            Self::MissingValue(flag) => f.write_str(&format!("{flag} requires an argument")),
            Self::MissingFlag(flag) => f.write_str(&format!("{flag} is required")),
            Self::UnknownOption(word) => f.write_str(&format!("unknown option: {word}")),
            Self::UnknownCommand(word) => f.write_str(&format!("unknown command: {word}")),
            Self::UnknownFormat(word) => f.write_str(&format!("unknown format: {word}")),
            Self::EntryNotFound(word) => f.write_str(&format!("entry not found: {word}")),
            Self::CatalogFailed(cause) => {
                f.write_str(&format!("failed to generate catalog: {cause}"))
            }
            Self::GenerateFailed { name, cause } => {
                f.write_str(&format!("failed to generate {name}: {cause}"))
            }
            Self::ManifestFailed(cause) => {
                f.write_str(&format!("failed to generate manifest: {cause}"))
            }
            Self::DigestFailed { name, cause } => {
                f.write_str(&format!("failed to generate {name}: {cause}"))
            }
            Self::WouldGenerate(name) => f.write_str(&format!("would generate: {name}")),
            Self::Generated { manifest, media } => {
                f.write_str(&format!("generated: {manifest} and {media}"))
            }
            Self::VerifyUsage(error) => write!(f, "{error}"),
            Self::Input(InputError::LockfileRead(cause)) => {
                f.write_str(&format!("failed to read lockfile: {cause}"))
            }
            Self::Input(InputError::CorpusDirectory(cause)) => {
                f.write_str(&format!("failed to read corpus directory: {cause}"))
            }
            Self::Input(InputError::CorpusDirectoryEntry(cause)) => {
                f.write_str(&format!("failed to read directory entry: {cause}"))
            }
            Self::Input(InputError::CorpusFile { name, cause }) => {
                f.write_str(&format!("failed to read file {name}: {cause}"))
            }
            Self::LockfileParse(cause) => {
                f.write_str(&format!("failed to parse lockfile: {cause}"))
            }
            Self::LockfileDuplicate(name) => {
                f.write_str(&format!("lockfile lists {} more than once", name.as_str()))
            }
            Self::UnknownEntry(name) => f.write_str(&format!("unknown entry name: {name}")),
            Self::Difference(difference) => write!(f, "{difference}"),
            Self::FailedToCheck { name, error } => {
                f.write_str(&format!("failed to check {name}: {}", ErrorChain(error)))
            }
            Self::Failed { step, cause } => f.write_str(&format!("{step}: {cause}")),
        }
    }
}
