//! Small named values the verify subcommand reads and reports.

use core::fmt::{Display, Formatter, Result as FmtResult};

/// How many entries a regeneration check compared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EntryCount(pub(super) usize);

impl Display for EntryCount {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

/// The value that follows a flag, exactly as typed.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct FlagValue(pub(super) String);
