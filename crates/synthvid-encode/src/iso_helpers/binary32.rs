//! The binary32 bits of a rational, computed in integer arithmetic.
//!
//! A float32 never enters the computation, so the same bits come out on every platform.

use crate::iso::IsoError;
use crate::iso_types::U32Be;
use core::num::{NonZeroU128, NonZeroU32};

#[cfg(test)]
mod tests;

/// The smallest significand of a normal binary32 value, `2^23`: its hidden bit alone.
const SIGNIFICAND_MIN: u128 = 0x80_0000;

/// One past the largest 24-bit significand, `2^24`.
const SIGNIFICAND_LIMIT: u128 = 0x100_0000;

/// The numerator of the rational to convert. It is non-zero: a zero has no binary32 search
/// result, so the type refuses it.
#[derive(Clone, Copy)]
pub(super) struct Numerator(pub(super) NonZeroU32);

/// A power of two applied to the rational: `2^exponent`.
#[derive(Clone, Copy)]
struct Exponent(i32);

/// The integer quotient of a scaled division: the candidate 24-bit significand.
struct Quotient(u128);

/// The remainder of the same division, which decides the rounding.
struct Remainder(u128);

/// The divisor the division used, after any power of two was folded into it.
struct Divisor(u128);

/// The result of `numer * 2^exponent / denom`, with the divisor actually used.
struct Division {
    /// The integer part of the scaled rational.
    quotient: Quotient,
    /// What the integer division left over.
    remainder: Remainder,
    /// The denominator, scaled by the power of two when the exponent is negative.
    divisor: Divisor,
}

/// The binary32 word nearest to `numer / denom`, rounding to nearest with ties to even.
///
/// Integer arithmetic only, so the bits are the same on every platform, and float32
/// never enters the file's arithmetic. A binary32 significand is 24 bits, so exactly one
/// exponent `k` puts the floor of `numer * 2^k / denom` in `[2^23, 2^24)`; the search
/// covers `-40..=60`, which holds every positive 32-bit fraction, since such a value lies
/// in `(2^-32, 2^32)`. The remainder of that division decides the rounding.
///
/// The numerator is non-zero by type, since a zero numerator has no such `k`.
pub(super) fn binary32_bits(numer: Numerator, denom: NonZeroU32) -> Result<U32Be, IsoError> {
    for exponent in -40_i32..=60_i32 {
        let scale = Exponent(exponent);
        let division = scaled_division(numer, denom, scale)?;
        if (SIGNIFICAND_MIN..SIGNIFICAND_LIMIT).contains(&division.quotient.0) {
            return pack_binary32(&division, scale);
        }
    }
    Err(IsoError::CaptureRateTooLarge)
}

/// The quotient and remainder of `numer * 2^exponent / denom`, with the divisor used.
///
/// The power of two multiplies the numerator when the exponent is positive and the
/// denominator when it is negative, so the division stays exact integer arithmetic.
fn scaled_division(
    numer: Numerator,
    denom: NonZeroU32,
    exponent: Exponent,
) -> Result<Division, IsoError> {
    let numer_wide = NonZeroU128::from(numer.0).get();
    let denom_wide = NonZeroU128::from(denom).get();
    let power = 1_u128
        .checked_shl(exponent.0.unsigned_abs())
        .ok_or(IsoError::CaptureRateTooLarge)?;
    let (dividend, divisor) = if exponent.0 >= 0_i32 {
        (
            numer_wide
                .checked_mul(power)
                .ok_or(IsoError::CaptureRateTooLarge)?,
            denom_wide,
        )
    } else {
        (
            numer_wide,
            denom_wide
                .checked_mul(power)
                .ok_or(IsoError::CaptureRateTooLarge)?,
        )
    };
    let quotient = dividend
        .checked_div(divisor)
        .ok_or(IsoError::CaptureRateTooLarge)?;
    let remainder = dividend
        .checked_rem(divisor)
        .ok_or(IsoError::CaptureRateTooLarge)?;
    Ok(Division {
        quotient: Quotient(quotient),
        remainder: Remainder(remainder),
        divisor: Divisor(divisor),
    })
}

/// Packs the 24-bit significand of `division` at `exponent` into a binary32 word, rounding
/// up when the remainder exceeds half the divisor, or equals it and the quotient is odd.
fn pack_binary32(division: &Division, exponent: Exponent) -> Result<U32Be, IsoError> {
    let quotient = division.quotient.0;
    let remainder = division.remainder.0;
    let divisor = division.divisor.0;
    let doubled = remainder
        .checked_add(remainder)
        .ok_or(IsoError::CaptureRateTooLarge)?;
    let round_up = doubled > divisor || (doubled == divisor && !quotient.is_multiple_of(2));
    let rounded = if round_up {
        quotient
            .checked_add(1)
            .ok_or(IsoError::CaptureRateTooLarge)?
    } else {
        quotient
    };

    // A carry to 2^24 is 2^23 one binade up, so the scale drops by one.
    let (significand, scale) = if rounded == SIGNIFICAND_LIMIT {
        (
            SIGNIFICAND_MIN,
            exponent
                .0
                .checked_sub(1_i32)
                .ok_or(IsoError::CaptureRateTooLarge)?,
        )
    } else {
        (rounded, exponent.0)
    };

    // The value is significand * 2^-scale, so its biased exponent is 127 + 23 - scale.
    // The sign bit is zero: a rate is positive.
    let biased_signed = 150_i32
        .checked_sub(scale)
        .ok_or(IsoError::CaptureRateTooLarge)?;
    let biased = u32::try_from(biased_signed).map_err(IsoError::capture_rate_too_large)?;
    let fraction_wide = significand
        .checked_sub(SIGNIFICAND_MIN)
        .ok_or(IsoError::CaptureRateTooLarge)?;
    let fraction = u32::try_from(fraction_wide).map_err(IsoError::capture_rate_too_large)?;
    let exponent_field = biased
        .checked_mul(0x80_0000)
        .ok_or(IsoError::CaptureRateTooLarge)?;
    let word = exponent_field
        .checked_add(fraction)
        .ok_or(IsoError::CaptureRateTooLarge)?;
    Ok(U32Be(word))
}
