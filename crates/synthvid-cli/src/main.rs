//! Command-line entry point.
//!
//! The only crate that touches the filesystem. Structured output goes to
//! stdout; diagnostics go to stderr.

fn main() {
    use std::io::Write;
    writeln!(std::io::stdout(), "{{}}").ok();
}
