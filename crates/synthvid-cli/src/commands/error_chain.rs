//! An error shown together with its causes.

use core::error::Error;
use core::fmt::{Display, Formatter, Result as FmtResult};

/// An error and its causes, each once, separated by colons.
pub(super) struct ErrorChain<'error>(pub(super) &'error (dyn Error + 'static));

impl Display for ErrorChain<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)?;
        let mut cause = self.0.source();
        while let Some(inner) = cause {
            write!(f, ": {inner}")?;
            cause = inner.source();
        }
        Ok(())
    }
}
