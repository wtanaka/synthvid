//! Command implementations.
//!
//! Each subcommand computes a `Report` from its arguments without any I/O,
//! and `shell::execute` carries the report out. The I/O lives in that one
//! shell.

mod argv;
mod entry_digests;
mod entry_name;
mod error_chain;
mod generate;
mod listing;
mod lock;
mod message;
mod modes;
mod outcome;
mod report;
mod shell;
mod verify;
mod verify_args;
mod verify_input;
mod verify_load;
mod verify_values;

pub use argv::Argv;
pub use outcome::Outcome;

use message::Message;
use report::{Report, Word};
use shell::execute;
use std::io;

/// The name of a command word, as the user typed it.
#[derive(Clone, Copy, Debug)]
pub struct CommandName<'name>(pub &'name str);

/// Runs the command named by the first of the command-line arguments `args`
/// with the rest. A failure to write the report fails the command.
#[must_use]
pub fn run(args: Argv<'_>) -> Outcome {
    let outcome = match args.0.split_first() {
        None => print_usage(),
        Some((command, rest)) => dispatch(CommandName(command.as_str()), Argv(rest)),
    };
    match outcome {
        Ok(Outcome::Success) => Outcome::Success,
        Ok(Outcome::Failure) | Err(_) => Outcome::Failure,
    }
}

/// Runs the subcommand that `command` names with its arguments.
fn dispatch(command: CommandName<'_>, args: Argv<'_>) -> io::Result<Outcome> {
    match command.0 {
        "generate" => cmd_generate(args),
        "verify" => cmd_verify(args),
        "list" => cmd_list(args),
        "manifest" => cmd_manifest(args),
        "lock" => cmd_lock(args),
        _ => cmd_unknown(command),
    }
}

/// Prints the usage message for a missing command.
///
/// # Errors
///
/// Returns an error when writing to stderr fails.
pub fn print_usage() -> io::Result<Outcome> {
    execute(Report::failed(Message::Usage))
}

/// Reports a command word that names no subcommand.
///
/// # Errors
///
/// Returns an error when writing to stderr fails.
pub fn cmd_unknown(command: CommandName<'_>) -> io::Result<Outcome> {
    execute(Report::failed(Message::UnknownCommand(Word(
        command.0.to_owned(),
    ))))
}

/// Runs the generate subcommand.
///
/// # Errors
///
/// Returns an error when writing to stdout or stderr fails.
pub fn cmd_generate(args: Argv<'_>) -> io::Result<Outcome> {
    execute(generate::plan_generate(args))
}

/// Runs the verify subcommand.
///
/// # Errors
///
/// Returns an error when writing to stdout or stderr fails.
pub fn cmd_verify(args: Argv<'_>) -> io::Result<Outcome> {
    execute(verify::plan_verify(args, verify_load::load))
}

/// Runs the list subcommand.
///
/// # Errors
///
/// Returns an error when writing to stdout or stderr fails.
pub fn cmd_list(args: Argv<'_>) -> io::Result<Outcome> {
    execute(listing::plan_list(args))
}

/// Runs the manifest subcommand.
///
/// # Errors
///
/// Returns an error when writing to stdout or stderr fails.
pub fn cmd_manifest(args: Argv<'_>) -> io::Result<Outcome> {
    execute(listing::plan_manifest(args))
}

/// Runs the lock subcommand.
///
/// # Errors
///
/// Returns an error when writing to stdout or stderr fails.
pub fn cmd_lock(args: Argv<'_>) -> io::Result<Outcome> {
    execute(lock::plan_lock(args))
}
