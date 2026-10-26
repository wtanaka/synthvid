//! The failures of exact lattice arithmetic.

use core::error::Error;
use core::fmt;

use crate::ratio::Overflow;

/// A lattice operation that cannot produce an answer.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LatticeError {
    /// Exact arithmetic could not represent an intermediate value or a result.
    Overflow,
}

impl fmt::Display for LatticeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => write!(f, "exact lattice arithmetic could not represent a result"),
        }
    }
}

impl Error for LatticeError {}

/// Every overflow of lattice arithmetic is the same failure, so the variant
/// alone carries the fact and the overflow itself has nothing further to report.
impl From<Overflow> for LatticeError {
    fn from(_: Overflow) -> Self {
        Self::Overflow
    }
}
