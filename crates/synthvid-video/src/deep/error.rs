//! The failures of projection arithmetic.

use core::error::Error;
use core::fmt;

use synthvid_scene::Overflow;

/// A projection that cannot produce a lattice coordinate.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ProjectionError {
    /// Exact arithmetic could not represent an intermediate value or a result.
    Overflow,
    /// A projected lattice coordinate has a magnitude beyond 2^28 lattice units, which is
    /// 2^20 sensels. Such a coordinate is refused rather than clamped.
    BeyondGuardBand,
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => write!(f, "projection arithmetic could not represent a result"),
            Self::BeyondGuardBand => {
                write!(
                    f,
                    "projected coordinate lies beyond the guard band of 2^20 sensels"
                )
            }
        }
    }
}

impl Error for ProjectionError {}

/// Every overflow of projection arithmetic is the same failure, so the variant alone carries the
/// fact and the overflow itself has nothing further to report.
impl From<Overflow> for ProjectionError {
    fn from(_: Overflow) -> Self {
        Self::Overflow
    }
}
