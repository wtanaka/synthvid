//! Shared helper for this crate's oracle test modules.

use std::process::{Command, Stdio};

/// Checks if a command is available in the environment. Shared by both
/// oracle test modules, since neither `jq`/`python3` (JSON) nor `djpeg`
/// (JPEG) checks differ in how they probe for their external tool.
pub(crate) fn is_command_available(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
