//! Helper functions for constructing and analyzing [`Ratio`] values.

use core::num::{NonZeroI64, NonZeroU8};

use super::{Overflow, Ratio};

/// Constructs a `NonZeroI64` with value 2.
///
/// Uses `NonZeroU8::MIN.saturating_add(1)`, which returns a `NonZeroU8`
/// directly (not an `Option`, since adding 1 to something non-zero cannot
/// produce zero), then converts via `From`.
fn nonzero_two() -> NonZeroI64 {
    NonZeroI64::from(NonZeroU8::MIN.saturating_add(1))
}

/// Constructs a `NonZeroI64` with value 4.
///
/// Uses `NonZeroU8::MIN.saturating_add(3)` then converts via `From`.
fn nonzero_four() -> NonZeroI64 {
    NonZeroI64::from(NonZeroU8::MIN.saturating_add(3))
}

/// Returns the exact ratio `value / 1`.
///
/// The construction is total: any `i64` divided by 1 is already in lowest terms
/// (gcd(n, 1) = 1), and 1 fits in `i64`. Constructs the ratio directly without
/// going through the reduction path.
#[must_use]
pub fn int_ratio(value: i64) -> Ratio {
    Ratio::from_integer(value)
}

/// Returns the exact ratio `1 / 2`.
///
/// The construction is total: 1/2 is already in lowest terms (gcd(1, 2) = 1),
/// and both values fit in `i64`. Constructs the ratio directly without
/// going through the reduction path.
#[must_use]
pub fn half_ratio() -> Ratio {
    Ratio {
        numer: 1,
        denom: nonzero_two(),
    }
}

/// Returns the exact ratio `1 / 4`, the squared pixel-centre threshold.
///
/// The construction is total: 1/4 is already in lowest terms (gcd(1, 4) = 1),
/// and both values fit in `i64`. Constructs the ratio directly without
/// going through the reduction path.
#[must_use]
pub fn quarter_ratio() -> Ratio {
    Ratio {
        numer: 1,
        denom: nonzero_four(),
    }
}

/// Returns the exact ratio `(2 * index + 1) / 2`, the centre of pixel `index`.
///
/// Doubling a `u16` and setting the low bit cannot leave the range of a `u32`
/// -- the largest result is `131_071` -- and widening that to `i64` is
/// infallible, so this needs no checked arithmetic and cannot fail. The
/// numerator is odd, so the ratio is already in lowest terms.
#[must_use]
pub fn pixel_centre_ratio(index: u16) -> Ratio {
    Ratio {
        numer: i64::from((u32::from(index) << 1) | 1),
        denom: nonzero_two(),
    }
}

/// Returns the greatest integer less than or equal to `value`.
///
/// Uses Euclidean division; the denominator is strictly positive so the
/// result always fits in `i64` and this only returns `Err(Overflow)` in theory.
///
/// # Errors
///
/// Returns [`Overflow`] if division overflows.
pub const fn floor_ratio(value: Ratio) -> Result<i64, Overflow> {
    match value.numer().checked_div_euclid(value.denom().get()) {
        Some(r) => Ok(r),
        None => Err(Overflow),
    }
}

/// Returns the smallest integer greater than or equal to `value`.
///
/// Computed as the Euclidean floor plus one when there is a remainder, so
/// no negation of `i64::MIN` is ever required.
///
/// # Errors
///
/// Returns [`Overflow`] if division or increment overflows.
pub fn ceil_ratio(value: Ratio) -> Result<i64, Overflow> {
    let quot = value
        .numer()
        .checked_div_euclid(value.denom().get())
        .ok_or(Overflow)?;
    let remnant = value
        .numer()
        .checked_rem_euclid(value.denom().get())
        .ok_or(Overflow)?;
    if remnant == 0 {
        Ok(quot)
    } else {
        quot.checked_add(1).ok_or(Overflow)
    }
}
