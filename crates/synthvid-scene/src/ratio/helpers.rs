//! Helper functions for constructing and analyzing [`Ratio`] values.

use core::num::{NonZeroI64, NonZeroU8};

use super::{Numerator, Overflow, Ratio};

/// Constructs a `NonZeroI64` with value 2.
///
/// Counts the set bits of `0b11`: a bitwise or into a `NonZeroU8` stays
/// non-zero, and so does its count of set bits, so there is no `Option` to
/// discharge. The count converts via `From`.
fn nonzero_two() -> NonZeroI64 {
    NonZeroI64::from((NonZeroU8::MIN | 0b10).count_ones())
}

/// Constructs a `NonZeroI64` with value 4.
///
/// Counts the set bits of `0b1111`, as [`nonzero_two`] does for `0b11`.
fn nonzero_four() -> NonZeroI64 {
    NonZeroI64::from((NonZeroU8::MIN | 0b1110).count_ones())
}

/// Returns the exact ratio `1 / 2`.
///
/// The construction is total: 1/2 is already in lowest terms (gcd(1, 2) = 1),
/// and both values fit in `i64`. Constructs the ratio directly without
/// going through the reduction path.
#[must_use]
pub fn half_ratio() -> Ratio {
    Ratio {
        numer: Numerator(1),
        denom: nonzero_two(),
    }
}

/// Returns the exact ratio `1 / 4`, the squared pixel-center threshold.
///
/// The construction is total: 1/4 is already in lowest terms (gcd(1, 4) = 1),
/// and both values fit in `i64`. Constructs the ratio directly without
/// going through the reduction path.
#[must_use]
pub fn quarter_ratio() -> Ratio {
    Ratio {
        numer: Numerator(1),
        denom: nonzero_four(),
    }
}

/// The index of a pixel along one axis: a column or a row.
///
/// Every `u16` names a pixel; the axis it lies on is the caller's to know.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct AxisPixel(pub u16);

/// Returns the exact ratio `(2 * index + 1) / 2`, the center of pixel `index`.
///
/// Doubling a `u16` and setting the low bit cannot leave the range of a `u32`
/// -- the largest result is `131_071` -- and widening that to `i64` is
/// infallible, so this needs no checked arithmetic and cannot fail. The
/// numerator is odd, so the ratio is already in lowest terms.
#[must_use]
pub fn pixel_centre_ratio(pixel: AxisPixel) -> Ratio {
    let AxisPixel(index) = pixel;
    Ratio {
        numer: Numerator(i64::from((u32::from(index) << 1) | 1)),
        denom: nonzero_two(),
    }
}

/// Returns the greatest integer less than or equal to `value`, as a ratio
/// over one.
///
/// Uses Euclidean division; the denominator is strictly positive so the
/// result always fits in `i64` and this only returns `Err(Overflow)` in theory.
///
/// # Errors
///
/// Returns [`Overflow`] if division overflows.
pub fn floor_ratio(value: Ratio) -> Result<Ratio, Overflow> {
    value
        .numer()
        .0
        .checked_div_euclid(value.denom().get())
        .map(Numerator)
        .map(Ratio::from_integer)
        .ok_or(Overflow)
}

/// Returns the smallest integer greater than or equal to `value`, as a ratio
/// over one.
///
/// Computed as the Euclidean floor plus one when there is a remainder, so
/// no negation of `i64::MIN` is ever required.
///
/// # Errors
///
/// Returns [`Overflow`] if division or increment overflows.
pub fn ceil_ratio(value: Ratio) -> Result<Ratio, Overflow> {
    let quot = value
        .numer()
        .0
        .checked_div_euclid(value.denom().get())
        .ok_or(Overflow)?;
    let remnant = value
        .numer()
        .0
        .checked_rem_euclid(value.denom().get())
        .ok_or(Overflow)?;
    if remnant == 0 {
        Ok(Ratio::from_integer(Numerator(quot)))
    } else {
        quot.checked_add(1)
            .map(Numerator)
            .map(Ratio::from_integer)
            .ok_or(Overflow)
    }
}
