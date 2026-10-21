//! Shared helpers for this crate's oracle test modules.

use core::fmt;
use std::process::{Command, Stdio};

/// The name of an external program, as `which` looks it up.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Program(pub(crate) &'static str);

/// Whether an external program can be run here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Availability {
    /// `which` found the program.
    Installed,
    /// `which` did not find the program, or could not be run.
    Missing,
}

/// What an external oracle reported when it rejected its input, or why it
/// could not be run.
pub(crate) struct OracleFailure(pub(crate) String);

impl fmt::Debug for OracleFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OracleFailure").field(&self.0).finish()
    }
}

/// Checks if a command is available in the environment. Shared by both
/// oracle test modules, since neither `jq`/`python3` (JSON) nor `djpeg`
/// (JPEG) checks differ in how they probe for their external tool.
pub(crate) fn availability(program: Program) -> Availability {
    let found = Command::new("which")
        .arg(program.0)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if found {
        Availability::Installed
    } else {
        Availability::Missing
    }
}
