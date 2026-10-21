//! Command-line entry point.
//!
//! The only crate that touches the filesystem. Structured output goes to
//! stdout; diagnostics go to stderr. All rendering, coding, manifest, and
//! digest logic lives in the library crates; this crate only parses
//! arguments, reads and writes files, and prints.
//!
//! The crate is pre-release (version `0.0.x`): its commands and options may
//! change in any release without notice.
//!
//! # Commands
//!
//! - `generate --out DIR [--only NAME]... [--dry-run]`: render every
//!   catalog entry into `DIR` as `<name>.json` plus `<name>-media.mp4`
//!   or `<name>-media.avi`.
//! - `verify --corpus DIR`: check every file in `DIR` against
//!   `DIR/catalog.lock`, exiting `0` when all digests and lengths match.
//! - `list [--format text|json]`: print every catalog entry name.
//! - `manifest --name NAME`: print one entry's manifest to stdout.
//! - `lock --corpus DIR [--write] [--dry-run]`: print (or, with `--write`,
//!   write) the lockfile text the current code produces.

pub mod commands;
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match commands::run(commands::Argv(&args)) {
        commands::Outcome::Success => ExitCode::SUCCESS,
        commands::Outcome::Failure => ExitCode::from(1_u8),
    }
}
