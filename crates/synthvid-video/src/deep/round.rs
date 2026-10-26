//! Exact 128-bit intermediates for projection, and the single rounding that ends each one.
//!
//! A projected offset and a lens shift each multiply several factors before they divide once, so
//! every quotient is taken from an exact numerator and denominator. Each such quotient is rounded
//! once, and an image point is rounded in two places: its projected offset, and the shift that
//! its sample is tested at. [`round_half_up`] is the only place a quotient is rounded, and it
//! rounds to the nearest integer with ties toward positive infinity. Every
//! operation is checked on `i128`, and a result that does not fit becomes
//! [`ProjectionError::Overflow`] rather than a wrapped value.

use core::num::NonZeroI128;

use synthvid_scene::Overflow;

use super::error::ProjectionError;

/// An exact integer intermediate of projection, held in 128 bits.
///
/// Every `i128` is a legal intermediate, so the field is public.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct WideInt(pub i128);

/// A non-zero exact denominator.
///
/// The field is public, so a value of this type cannot be zero, and a quotient by it always
/// exists.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct NonZeroWideInt(pub NonZeroI128);

impl NonZeroWideInt {
    /// Takes a denominator from an exact integer.
    ///
    /// # Errors
    ///
    /// Returns `Err(ProjectionError::Overflow)` when `value` is zero, because a quotient by zero
    /// has no value to project onto the lattice.
    pub fn new(value: WideInt) -> Result<Self, ProjectionError> {
        NonZeroI128::new(value.0)
            .map(Self)
            .ok_or(ProjectionError::Overflow)
    }
}

impl WideInt {
    /// Returns `self + other`.
    ///
    /// # Errors
    ///
    /// Returns `Err(ProjectionError::Overflow)` if the sum leaves the `i128` range.
    pub(super) fn plus(self, other: Self) -> Result<Self, ProjectionError> {
        Ok(Self(self.0.checked_add(other.0).ok_or(Overflow)?))
    }

    /// Returns `self - other`.
    ///
    /// # Errors
    ///
    /// Returns `Err(ProjectionError::Overflow)` if the difference leaves the `i128` range.
    pub(super) fn minus(self, other: Self) -> Result<Self, ProjectionError> {
        Ok(Self(self.0.checked_sub(other.0).ok_or(Overflow)?))
    }

    /// Returns `self * other`.
    ///
    /// # Errors
    ///
    /// Returns `Err(ProjectionError::Overflow)` if the product leaves the `i128` range.
    pub(super) fn times(self, other: Self) -> Result<Self, ProjectionError> {
        Ok(Self(self.0.checked_mul(other.0).ok_or(Overflow)?))
    }

    /// Returns `-self`.
    ///
    /// # Errors
    ///
    /// Returns `Err(ProjectionError::Overflow)` if the negation leaves the `i128` range.
    pub(super) fn negate(self) -> Result<Self, ProjectionError> {
        Ok(Self(self.0.checked_neg().ok_or(Overflow)?))
    }
}

/// The exact fraction that [`round_half_up`] rounds to an integer.
///
/// The numerator is any exact integer and the denominator is non-zero, so a fraction cannot be
/// formed by division by zero.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct RoundingInput {
    /// The exact dividend.
    pub numerator: WideInt,
    /// The exact divisor, which is never zero.
    pub denominator: NonZeroWideInt,
}

/// Rounds an exact fraction to the nearest integer, with ties toward positive infinity.
///
/// The result is `floor((2 * numerator + denominator) / (2 * denominator))` for a positive
/// denominator. A negative denominator is first moved into the numerator, so the fraction keeps
/// its value. Each of 1.5 and -1.5 therefore rounds to 2 and -1 respectively, and 7/3 rounds to
/// 2 while -7/3 rounds to -2.
///
/// # Errors
///
/// Returns `Err(ProjectionError::Overflow)` if an intermediate value leaves the `i128` range.
pub fn round_half_up(input: RoundingInput) -> Result<WideInt, ProjectionError> {
    let RoundingInput {
        numerator: dividend,
        denominator: NonZeroWideInt(divisor),
    } = input;
    let (signed_dividend, positive_divisor) = if divisor.get() < 0 {
        (dividend.negate()?, WideInt(divisor.get()).negate()?)
    } else {
        (dividend, WideInt(divisor.get()))
    };
    let doubled_dividend = signed_dividend.times(WideInt(2))?.plus(positive_divisor)?;
    let doubled_divisor = NonZeroWideInt::new(positive_divisor.times(WideInt(2))?)?;
    let quotient = doubled_dividend
        .0
        .checked_div_euclid(doubled_divisor.0.get())
        .ok_or(Overflow)?;
    Ok(WideInt(quotient))
}

#[cfg(test)]
mod tests;
