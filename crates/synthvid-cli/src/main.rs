//! Command-line entry point.
//!
//! The only crate that touches the filesystem. Structured output goes to
//! stdout; diagnostics go to stderr. All rendering, coding, manifest, and
//! digest logic lives in the library crates; this crate only parses
//! arguments, reads and writes files, and prints.
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

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let Some(command) = args.get(1) else {
        commands::print_usage();
        std::process::exit(1);
    };

    let cmd_args = args.get(2..).unwrap_or(&[]);
    let exit_code = match command.as_str() {
        "generate" => commands::cmd_generate(cmd_args),
        "verify" => commands::cmd_verify(cmd_args),
        "list" => commands::cmd_list(cmd_args),
        "manifest" => commands::cmd_manifest(cmd_args),
        "lock" => commands::cmd_lock(cmd_args),
        _ => {
            use std::io::Write;
            writeln!(std::io::stderr(), "unknown command: {command}").ok();
            1
        }
    };

    std::process::exit(exit_code);
}
