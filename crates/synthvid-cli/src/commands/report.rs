//! The values a command computes, before any of them is carried out.
//!
//! A command's logic returns a [`Report`]: the steps it wants performed, in
//! order, and the outcome it reaches when they all succeed. Nothing here
//! touches stdout, stderr, or the filesystem; the shell in `shell.rs` performs
//! the steps.

use super::message::Message;
use super::Outcome;
use std::path::PathBuf;

/// Text to be written to stdout, exactly as it is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct OutText(pub(super) String);

/// A word the user typed, kept exactly as typed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Word(pub(super) String);

/// The name of a file a command writes into an output directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileName(pub(super) String);

/// The bytes of a file a command writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileBytes(pub(super) Vec<u8>);

/// A flag that a command's arguments may name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Flag {
    /// `--out DIR`.
    Out,
    /// `--only NAME`.
    Only,
    /// `--corpus DIR`.
    Corpus,
    /// `--format FORMAT`.
    Format,
    /// `--name NAME`.
    Name,
}

/// The kinds of file a command publishes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Artifact {
    /// An entry's manifest.
    Manifest,
    /// An entry's media file.
    Media,
    /// The lockfile.
    Lockfile,
}

/// A file to be published: its bytes are written to a temporary file beside
/// `dest`, then renamed over `dest`.
#[derive(Debug)]
pub(super) struct Publication {
    /// Which kind of file this is; it names the file in any failure message.
    pub(super) artifact: Artifact,
    /// The final path of the file.
    pub(super) dest: PathBuf,
    /// The contents of the file.
    pub(super) bytes: FileBytes,
}

impl Publication {
    /// The temporary path the bytes are written to before the rename: `DEST.tmp`.
    pub(super) fn temp_path(&self) -> PathBuf {
        PathBuf::from(format!("{}.tmp", self.dest.display()))
    }
}

/// One thing the shell carries out. Steps run in the order they are listed.
#[derive(Debug)]
pub(super) enum Step {
    /// Text for stdout. A failed write ends the command with an error.
    Stdout(OutText),
    /// The manifest text for stdout. A failed write is reported as a failed
    /// manifest write, and the command fails.
    ManifestStdout(OutText),
    /// A line for stderr.
    Stderr(Message),
    /// Create the directory and any missing parents. A failure is reported and
    /// the command fails without carrying out any later step.
    CreateDir(PathBuf),
    /// Publish each file in order. The first failure is reported and skips the
    /// rest of the group, including `announce`. When every file is published,
    /// `announce` is printed on stderr, if there is one.
    Publish {
        /// The files, in the order they are published.
        files: Vec<Publication>,
        /// The stderr line printed once every file has been published.
        announce: Option<Message>,
    },
}

/// What a command asks to have done, and the outcome it reaches when all of
/// it succeeds. A step that fails at run time turns the outcome into a failure.
#[derive(Debug)]
pub(super) struct Report {
    /// The steps, in order.
    pub(super) steps: Vec<Step>,
    /// The outcome when every step succeeds.
    pub(super) outcome: Outcome,
}

impl Report {
    /// A report with the given steps and outcome.
    pub(super) const fn new(steps: Vec<Step>, outcome: Outcome) -> Self {
        Self { steps, outcome }
    }

    /// A report that prints one diagnostic on stderr and fails.
    pub(super) fn failed(message: Message) -> Self {
        Self {
            steps: vec![Step::Stderr(message)],
            outcome: Outcome::Failure,
        }
    }
}

#[cfg(test)]
impl Report {
    /// One line per step, in order, naming what the shell would do. Tests
    /// assert on this sequence instead of on the shell's output.
    pub(super) fn trace(&self) -> Trace {
        let lines = self
            .steps
            .iter()
            .map(|step| match step {
                Step::Stdout(out) => format!("stdout {:?}", out.0),
                Step::ManifestStdout(out) => format!("manifest-stdout {:?}", out.0),
                Step::Stderr(message) => format!("stderr {message}"),
                Step::CreateDir(path) => format!("mkdir {}", path.display()),
                Step::Publish { files, announce } => format!(
                    "publish {} file(s), announce: {}",
                    files.len(),
                    announce.is_some()
                ),
            })
            .collect::<Vec<String>>();
        Trace(lines)
    }
}

/// The lines [`Report::trace`] produces, one per step.
#[cfg(test)]
pub(super) struct Trace(pub(super) Vec<String>);
