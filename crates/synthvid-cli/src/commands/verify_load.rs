//! The reads the verify command performs before it plans.
//!
//! These functions are the only place the verify command touches the
//! filesystem for its inputs. Each returns what it read as a value, and the
//! pure core in `verify.rs` never calls them.

use super::report::{FileBytes, FileName};
use super::verify_args::VerifyMode;
use super::verify_input::{
    CorpusFile, CorpusFiles, CorpusSnapshot, InputError, LoadedVerify, LockfileText,
};
use std::fs;
use std::path::Path;

/// Performs the reads that a verify run's mode calls for.
pub(super) fn load(mode: VerifyMode) -> LoadedVerify {
    match mode {
        VerifyMode::Corpus(dir) => LoadedVerify::Corpus(read_corpus(&dir)),
        VerifyMode::Regenerate {
            lock_file,
            requested,
        } => LoadedVerify::Regenerate {
            requested,
            lockfile: read_lockfile(&lock_file),
        },
    }
}

/// Reads a lockfile's text.
fn read_lockfile(path: &Path) -> Result<LockfileText, InputError> {
    fs::read_to_string(path)
        .map(LockfileText)
        .map_err(InputError::LockfileRead)
}

/// Reads a corpus directory's `catalog.lock`, then lists the directory. A
/// corpus whose lockfile cannot be read is not listed.
fn read_corpus(dir: &Path) -> CorpusSnapshot {
    match read_lockfile(&dir.join("catalog.lock")) {
        Ok(lockfile) => CorpusSnapshot::Listed {
            lockfile,
            files: list_corpus(dir),
        },
        Err(cause) => CorpusSnapshot::Unreadable(cause),
    }
}

/// Lists a directory and reads every file in it except `catalog.lock`, in the
/// order the directory lists them.
fn list_corpus(dir: &Path) -> Result<CorpusFiles, InputError> {
    let mut files = Vec::new();
    for listed in fs::read_dir(dir).map_err(InputError::CorpusDirectory)? {
        let dir_entry = listed.map_err(InputError::CorpusDirectoryEntry)?;
        let name = dir_entry.file_name().to_string_lossy().into_owned();
        if name == "catalog.lock" {
            continue;
        }
        let bytes = fs::read(dir_entry.path()).map_err(|cause| InputError::CorpusFile {
            name: FileName(name.clone()),
            cause,
        })?;
        files.push(CorpusFile {
            name: FileName(name),
            bytes: FileBytes(bytes),
        });
    }
    Ok(CorpusFiles(files))
}
