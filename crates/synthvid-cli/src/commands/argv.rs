//! The arguments a subcommand receives.

/// The words a subcommand receives after its own name.
#[derive(Clone, Copy, Debug)]
pub struct Argv<'args>(pub &'args [String]);
