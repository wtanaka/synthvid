//! The values the verify command reads before it plans a report.
//!
//! Each type holds what the shell read and nothing else. `verify_load.rs`
//! builds them; the pure core in `verify.rs` consumes them without touching
//! the filesystem.

use super::entry_name::Requested;
use super::report::{FileBytes, FileName};
use std::io;

/// The text of a lockfile, exactly as read.
pub(super) use synthvid_catalog::LockfileText;

/// A file of a corpus directory: its name and its bytes.
pub(super) struct CorpusFile {
    /// The file's name in the directory.
    pub(super) name: FileName,
    /// The file's contents.
    pub(super) bytes: FileBytes,
}

/// The files of a corpus directory, in the order the directory listed them.
pub(super) struct CorpusFiles(pub(super) Vec<CorpusFile>);

/// A read that failed, named for the operation that failed.
#[derive(Debug)]
pub(super) enum InputError {
    /// The lockfile could not be read.
    LockfileRead(io::Error),
    /// The corpus directory could not be listed.
    CorpusDirectory(io::Error),
    /// An entry of the corpus directory could not be listed.
    CorpusDirectoryEntry(io::Error),
    /// A file of the corpus directory could not be read.
    CorpusFile {
        /// The file's name.
        name: FileName,
        /// Why reading failed.
        cause: io::Error,
    },
}

/// What was read of a corpus directory.
pub(super) enum CorpusSnapshot {
    /// The lockfile could not be read, so the directory was not listed.
    Unreadable(InputError),
    /// The lockfile was read and the directory listed. `files` holds the first
    /// failure to read the directory or one of its files, if there was one.
    Listed {
        /// The corpus's `catalog.lock`, as text.
        lockfile: LockfileText,
        /// The corpus's other files, or the first read failure.
        files: Result<CorpusFiles, InputError>,
    },
}

/// What was read for a verify run, by mode.
pub(super) enum LoadedVerify {
    /// A corpus directory, checked against its own lockfile.
    Corpus(CorpusSnapshot),
    /// A lockfile, for in-memory regeneration.
    Regenerate {
        /// The entries asked for on the command line.
        requested: Requested,
        /// The lockfile's text, or the failure to read it.
        lockfile: Result<LockfileText, InputError>,
    },
}
