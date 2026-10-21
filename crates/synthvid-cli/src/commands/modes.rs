//! Closed choices the subcommands take in place of `bool` flags.
//!
//! A `bool` parameter says nothing at the call site, and two of them can be
//! passed in each other's place without a compiler error. Each choice here is
//! named by what it does instead.

/// Whether a generate run writes its files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Execution {
    /// Write the files.
    Run,
    /// Report what would be written, and write nothing.
    DryRun,
}

/// What the lock subcommand does with the lockfile it computes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LockAction {
    /// Print the lockfile to stdout.
    Print,
    /// Write the lockfile into the corpus directory.
    Write,
    /// Write nothing, because `--write` was combined with `--dry-run`.
    DryRun,
}

/// Whether an entry is covered by a verification's selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Coverage {
    /// The selection includes the entry.
    Covered,
    /// The selection leaves the entry out.
    Uncovered,
}
