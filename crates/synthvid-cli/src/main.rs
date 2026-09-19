//! Command-line entry point.
//!
//! The only crate that touches the filesystem. Structured output goes to
//! stdout; diagnostics go to stderr.
#![expect(
    clippy::print_stdout,
    reason = "the command-line interface writes structured output to stdout"
)]

fn main() {
    println!("{{}}");
}
