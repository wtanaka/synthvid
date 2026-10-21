//! The result of a subcommand.

/// Whether a subcommand succeeded. The process exits with status `0` for
/// success and `1` for failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// The command did what it was asked, and found nothing wrong.
    Success,
    /// The command failed, or found a difference or an unreadable input.
    Failure,
}
