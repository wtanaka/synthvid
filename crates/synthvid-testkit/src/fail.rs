//! The failure value a check returns, and the conversion that produces it.

use core::fmt;
use core::panic::Location;

/// Why a check failed, and the line it failed on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fail(String);

impl Fail {
    /// Builds a failure that names the caller's location.
    #[must_use]
    #[track_caller]
    pub fn here(message: fmt::Arguments<'_>) -> Self {
        Self(format!("{}: {message}", Location::caller()))
    }
}

impl fmt::Display for Fail {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// Turns a missing value or an error into a [`Fail`] naming the caller.
pub trait OrFail<T> {
    /// Returns the value, or a [`Fail`] naming the line that expected it.
    ///
    /// # Errors
    ///
    /// Returns a [`Fail`] when `self` is `Err` or `None`.
    #[track_caller]
    fn or_fail(self) -> Result<T, Fail>;
}

impl<T, E: fmt::Debug> OrFail<T> for Result<T, E> {
    #[track_caller]
    fn or_fail(self) -> Result<T, Fail> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(Fail::here(format_args!("unexpected error: {error:?}"))),
        }
    }
}

impl<T> OrFail<T> for Option<T> {
    #[track_caller]
    fn or_fail(self) -> Result<T, Fail> {
        let at = Location::caller();
        self.ok_or_else(|| Fail(format!("{at}: unexpected None")))
    }
}

/// The error a step returned, for a check that expects the step to refuse.
///
/// # Errors
///
/// Returns a [`Fail`] when `result` is `Ok`, because the step was accepted.
#[track_caller]
pub fn refusal<T, E>(result: Result<T, E>) -> Result<E, Fail> {
    match result {
        Ok(_) => Err(Fail::here(format_args!(
            "the step was accepted, but a refusal was expected"
        ))),
        Err(error) => Ok(error),
    }
}

#[cfg(test)]
mod tests;
