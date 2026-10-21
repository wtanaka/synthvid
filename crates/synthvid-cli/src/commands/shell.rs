//! The I/O shell: carries out a report's steps, in order.
//!
//! This is the only part of the command code that writes to stdout or stderr
//! or touches the filesystem. It decides nothing about what a command says or
//! writes; that is already settled in the report.

use super::message::{FsStep, Message};
use super::report::{Artifact, OutText, Publication, Report, Step};
use super::Outcome;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Carries out every step of a report on the process's stdout and stderr, in
/// order, and returns the command's outcome. The outcome is a failure when the
/// report's own outcome is one, or when any step fails.
///
/// # Errors
///
/// Returns an error when writing to stdout or stderr fails.
pub(super) fn execute(report: Report) -> io::Result<Outcome> {
    execute_with(report, &mut io::stdout(), &mut io::stderr())
}

/// Carries out every step of a report, writing stdout text to `out` and stderr
/// lines to `err`. [`execute`] is this with the process's own streams.
///
/// # Errors
///
/// Returns an error when writing to `out` or `err` fails.
pub(super) fn execute_with<O, E>(report: Report, out: &mut O, err: &mut E) -> io::Result<Outcome>
where
    O: Write,
    E: Write,
{
    let mut outcome = report.outcome;
    for step in report.steps {
        let step_outcome = match step {
            Step::Stdout(text) => {
                write_stdout(out, &text)?;
                Outcome::Success
            }
            Step::ManifestStdout(text) => match write_stdout(out, &text) {
                Ok(()) => Outcome::Success,
                Err(cause) => failed(err, FsStep::Write(Artifact::Manifest), cause)?,
            },
            Step::Stderr(message) => {
                write_stderr(err, &message)?;
                Outcome::Success
            }
            Step::CreateDir(path) => match create_dir(err, &path)? {
                Outcome::Success => Outcome::Success,
                Outcome::Failure => return Ok(Outcome::Failure),
            },
            Step::Publish { files, announce } => publish_group(err, &files, announce.as_ref())?,
        };
        outcome.absorb(step_outcome);
    }
    Ok(outcome)
}

impl Outcome {
    /// Becomes a failure when `other` is one; otherwise leaves it unchanged.
    const fn absorb(&mut self, other: Self) {
        match other {
            Self::Success => {}
            Self::Failure => *self = Self::Failure,
        }
    }
}

/// Writes stdout text. The only write of stdout text in the command code.
fn write_stdout<W>(out: &mut W, text: &OutText) -> io::Result<()>
where
    W: Write,
{
    write!(out, "{}", text.0)
}

/// Writes one stderr line. The only write of stderr lines in the command code.
fn write_stderr<E>(err: &mut E, message: &Message) -> io::Result<()>
where
    E: Write,
{
    writeln!(err, "{message}")
}

/// Reports a failed filesystem or stdout step, and fails the command.
fn failed<E>(err: &mut E, step: FsStep, cause: io::Error) -> io::Result<Outcome>
where
    E: Write,
{
    write_stderr(err, &Message::Failed { step, cause })?;
    Ok(Outcome::Failure)
}

/// Creates the output directory and its parents.
fn create_dir<E>(err: &mut E, path: &Path) -> io::Result<Outcome>
where
    E: Write,
{
    match fs::create_dir_all(path) {
        Ok(()) => Ok(Outcome::Success),
        Err(cause) => failed(err, FsStep::CreateDir, cause),
    }
}

/// Publishes each file in order, then prints the announcement if all of them
/// were published.
fn publish_group<E>(
    err: &mut E,
    files: &[Publication],
    announce: Option<&Message>,
) -> io::Result<Outcome>
where
    E: Write,
{
    for file in files {
        if let Err(message) = publish_one(file) {
            write_stderr(err, &message)?;
            return Ok(Outcome::Failure);
        }
    }
    if let Some(message) = announce {
        write_stderr(err, message)?;
    }
    Ok(Outcome::Success)
}

/// Writes a file's bytes to its temporary path, then renames it over the
/// destination. A failure removes the temporary file and is returned as the
/// message that reports it.
fn publish_one(file: &Publication) -> Result<(), Box<Message>> {
    let temp = file.temp_path();
    if let Err(cause) = fs::write(&temp, &file.bytes.0) {
        discard_cleanup(fs::remove_file(&temp));
        return Err(Box::new(Message::Failed {
            step: FsStep::Write(file.artifact),
            cause,
        }));
    }
    if let Err(cause) = fs::rename(&temp, &file.dest) {
        discard_cleanup(fs::remove_file(&temp));
        return Err(Box::new(Message::Failed {
            step: FsStep::Rename(file.artifact),
            cause,
        }));
    }
    Ok(())
}

/// Discards the outcome of a cleanup that follows a failure already reported:
/// a second report adds nothing.
fn discard_cleanup(outcome: io::Result<()>) {
    drop(outcome);
}
