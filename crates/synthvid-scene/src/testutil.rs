//! Helpers shared by the unit tests of several modules.

use core::num::NonZeroI64;

use crate::frame::Frame;
use crate::ratio::Ratio;
use crate::units::{Dimensions, Height, Width};

/// Helper function to construct a [`Ratio`] for tests.
#[must_use]
pub fn make_ratio(numer: i64, denom: i64) -> Option<Ratio> {
    let nz = NonZeroI64::new(denom)?;
    Ratio::new(numer, nz)
}

/// Builds an 8x8 black frame for raster tests.
#[must_use]
pub fn black_8x8() -> Option<Frame> {
    let width = Width::new(8)?;
    let tall = Height::new(8)?;
    Frame::zeroed(Dimensions::new(width, tall))
}
