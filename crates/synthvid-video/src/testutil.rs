//! Helpers shared by the unit tests of several modules.

use core::num::NonZeroI64;

use synthvid_scene::{Numerator, Ratio};
use synthvid_testkit::{Fail, OrFail as _};

/// A fraction written as numerator and denominator.
#[derive(Copy, Clone, Debug)]
pub struct Frac(pub i64, pub i64);

impl Frac {
    /// Builds the exact ratio this fraction names.
    ///
    /// # Errors
    ///
    /// Fails when the denominator is zero or the ratio does not reduce.
    pub fn ratio(self) -> Result<Ratio, Fail> {
        let Self(numer, denom) = self;
        Ratio::new(Numerator(numer), NonZeroI64::new(denom).or_fail()?).or_fail()
    }
}
