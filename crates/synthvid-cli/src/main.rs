//! Command-line entry point.
//!
//! The only crate that touches the filesystem. Structured output goes to
//! stdout; diagnostics go to stderr.

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
