//! Helpers shared by the unit tests of several modules.

use core::num::NonZeroI64;

use synthvid_testkit::{Fail, OrFail as _};

use crate::frame::Frame;
use crate::geom::Point;
use crate::ratio::{Numerator, Ratio};
use crate::units::{Dimensions, Height, Width};

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

/// A point written as its `across` and `down` fractions.
#[derive(Copy, Clone, Debug)]
pub struct At(pub Frac, pub Frac);

impl At {
    /// Builds the exact point these fractions name.
    ///
    /// # Errors
    ///
    /// Fails when either fraction does not name a ratio.
    pub fn point(self) -> Result<Point, Fail> {
        let Self(across, down) = self;
        Ok(Point {
            across: across.ratio()?,
            down: down.ratio()?,
        })
    }
}

/// Builds an 8x8 black frame for raster tests.
///
/// # Errors
///
/// Fails if the frame cannot be built.
pub fn black_8x8() -> Result<Frame, Fail> {
    let width = Width::new(8).or_fail()?;
    let tall = Height::new(8).or_fail()?;
    Frame::zeroed(Dimensions::new(width, tall)).or_fail()
}
