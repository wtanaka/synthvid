//! Platform-independent trigonometry.
//!
//! [`sin_turns`] and [`cos_turns`] take angles in turns and evaluate a fixed
//! degree-7 polynomial in integer arithmetic. The standard library's floating
//! point `sin` and `cos` are deliberately not used: their results differ
//! between targets.

use core::num::{NonZeroI128, NonZeroI64, NonZeroU16};

use crate::ratio::{Numerator, Overflow, Ratio};

/// Denominator scale for trigonometric fixed-point representation (2^15 = 32,768).
const TRIG_SCALE: i64 = 0x8000;

/// The value of [`TRIG_SCALE`] as a non-zero denominator, built totally.
///
/// The midpoint of `NonZeroU16::MAX` (`65_535`) and `NonZeroU16::MIN` (`1`)
/// is exactly `32_768`; a midpoint of two non-zero values is itself non-zero,
/// and widening a `NonZeroU16` to a `NonZeroI64` is infallible, so there is no
/// `Option` to discharge. `trig_scale_denom_matches_trig_scale` pins this
/// to [`TRIG_SCALE`] so the two spellings cannot drift apart.
fn trig_scale_denom() -> NonZeroI64 {
    NonZeroI64::from(NonZeroU16::MAX.midpoint(NonZeroU16::MIN))
}

/// Denominator of the degree-7 polynomial coefficients.
const POLY_DENOM: i128 = 10_000;

/// Coefficient for the linear term u of the polynomial.
const POLY_A1: i128 = 15_708;

/// Coefficient for the cubic term u^3 of the polynomial.
const POLY_A3: i128 = 6_459;

/// Coefficient for the quintic term u^5 of the polynomial.
const POLY_A5: i128 = 794;

/// Coefficient for the septic term u^7 of the polynomial.
const POLY_A7: i128 = 43;

/// The numerator of a position `u` within a quarter turn.
#[derive(Copy, Clone, Debug)]
struct QuarterNumer(i128);

/// The denominator of a position `u` within a quarter turn: the denominator
/// of the whole angle.
#[derive(Copy, Clone, Debug)]
struct QuarterDenom(i128);

/// A sine value in `[0, TRIG_SCALE]`, scaled by [`TRIG_SCALE`].
#[derive(Copy, Clone, Debug)]
struct ScaledSine(i64);

/// Evaluates the fixed-point polynomial approximation of `sin(pi/2 * u)` for
/// `u = u_num / u_den` in `[0, 1]`.
///
/// Returns an integer value scaled by [`TRIG_SCALE`] (in `[0, TRIG_SCALE]`).
///
/// # Errors
///
/// Returns `Err(Overflow)` if any arithmetic operation overflows.
///
/// # Algorithm and Error
///
/// Uses the 7th-degree polynomial:
/// `P(u) = (15,708 * u - 6,459 * u^3 + 794 * u^5 - 43 * u^7) / 10,000`.
///
/// Exactly satisfies `P(0) = 0`, `P(1) = 1`, and `P'(1) = 0`, ensuring
/// continuous first derivatives across quadrant transitions.
fn eval_poly_quarter(
    quarter_numer: QuarterNumer,
    quarter_denom: QuarterDenom,
) -> Result<ScaledSine, Overflow> {
    let QuarterNumer(u_num) = quarter_numer;
    let QuarterDenom(u_den) = quarter_denom;
    if u_num <= 0 {
        return Ok(ScaledSine(0));
    }
    if u_num >= u_den {
        return Ok(ScaledSine(TRIG_SCALE));
    }

    let scale_128 = i128::from(TRIG_SCALE);
    let half_den = u_den.checked_div(2).ok_or(Overflow)?;
    let num_u = u_num
        .checked_mul(scale_128)
        .ok_or(Overflow)?
        .checked_add(half_den)
        .ok_or(Overflow)?;
    let scaled_u = clamp_to_scale(FixedPoint(num_u.checked_div(u_den).ok_or(Overflow)?));
    let powers = OddPowers::of(scaled_u)?;

    let term1 = POLY_A1.checked_mul(powers.first.0).ok_or(Overflow)?;
    let term3 = POLY_A3.checked_mul(powers.third.0).ok_or(Overflow)?;
    let term5 = POLY_A5.checked_mul(powers.fifth.0).ok_or(Overflow)?;
    let term7 = POLY_A7.checked_mul(powers.seventh.0).ok_or(Overflow)?;

    let half_poly = POLY_DENOM.checked_div(2).ok_or(Overflow)?;
    let poly_val = term1
        .checked_sub(term3)
        .ok_or(Overflow)?
        .checked_add(term5)
        .ok_or(Overflow)?
        .checked_sub(term7)
        .ok_or(Overflow)?
        .checked_add(half_poly)
        .ok_or(Overflow)?;

    let FixedPoint(val_clamped) = clamp_to_scale(FixedPoint(
        poly_val.checked_div(POLY_DENOM).ok_or(Overflow)?,
    ));

    i64::try_from(val_clamped)
        .map(ScaledSine)
        .map_err(Overflow::from)
}

/// A fixed-point value scaled by [`TRIG_SCALE`], widened for intermediate
/// products.
#[derive(Copy, Clone, Debug)]
struct FixedPoint(i128);

/// Clamps a fixed-point value to `[0, TRIG_SCALE]`.
fn clamp_to_scale(value: FixedPoint) -> FixedPoint {
    let scale_128 = i128::from(TRIG_SCALE);
    if value.0 < 0 {
        FixedPoint(0)
    } else if value.0 > scale_128 {
        FixedPoint(scale_128)
    } else {
        value
    }
}

/// The odd powers of a fixed-point value up to the seventh, each rescaled by
/// [`TRIG_SCALE`] after every multiplication.
#[derive(Copy, Clone, Debug)]
struct OddPowers {
    /// The value itself.
    first: FixedPoint,
    /// The value cubed.
    third: FixedPoint,
    /// The value to the fifth power.
    fifth: FixedPoint,
    /// The value to the seventh power.
    seventh: FixedPoint,
}

impl OddPowers {
    /// Computes the odd powers of `base`, truncating after each product in
    /// the order `x^2`, `x^3`, `x^4`, `x^5`, `x^6`, `x^7`.
    fn of(base: FixedPoint) -> Result<Self, Overflow> {
        let scale_128 = i128::from(TRIG_SCALE);
        let rescaled = |left: FixedPoint, right: FixedPoint| -> Result<FixedPoint, Overflow> {
            left.0
                .checked_mul(right.0)
                .ok_or(Overflow)?
                .checked_div(scale_128)
                .ok_or(Overflow)
                .map(FixedPoint)
        };
        let second = rescaled(base, base)?;
        let third = rescaled(second, base)?;
        let fourth = rescaled(second, second)?;
        let fifth = rescaled(fourth, base)?;
        let sixth = rescaled(third, third)?;
        let seventh = rescaled(sixth, base)?;
        Ok(Self {
            first: base,
            third,
            fifth,
            seventh,
        })
    }
}

/// Computes the sine of an angle given in turns (where 1 turn = 1 full revolution).
///
/// Because the angle is specified in turns rather than radians, exact fractions
/// of a circle (such as quarter turns or eighth turns) are represented exactly
/// without transcendental rounding.
///
/// # Determinism and Error Bound
///
/// This function uses a deterministic integer fixed-point 7th-degree polynomial
/// approximation producing bit-identical results on all platforms and architectures.
///
/// - Exactly matches expected values at cardinal angles:
///   `sin_turns(0) = 0`, `sin_turns(1/4) = 1`, `sin_turns(1/2) = 0`, `sin_turns(3/4) = -1`.
/// - The maximum absolute error compared to mathematical sine across the circle is
///   bounded by `6e-5` (`0.00006`).
/// - Satisfies the Pythagorean identity `sin² + cos² = 1` within `1.1e-4` (`0.00011`).
///
/// # Errors
///
/// Returns `Err(Overflow)` if any intermediate arithmetic overflows.
pub fn sin_turns(angle: Ratio) -> Result<Ratio, Overflow> {
    let numer = i128::from(angle.numer().0);
    let denom = NonZeroI128::from(angle.denom()).get();

    let rem_raw = numer.checked_rem(denom).ok_or(Overflow)?;
    let rem = if rem_raw < 0 {
        rem_raw.checked_add(denom).ok_or(Overflow)?
    } else {
        rem_raw
    };

    let rem4 = rem.checked_mul(4).ok_or(Overflow)?;
    let d2 = denom.checked_mul(2).ok_or(Overflow)?;
    let d3 = denom.checked_mul(3).ok_or(Overflow)?;
    let d4 = denom.checked_mul(4).ok_or(Overflow)?;

    let (u_num, sign) = if rem4 < denom {
        (rem4, 1_i64)
    } else if rem4 < d2 {
        let num = d2.checked_sub(rem4).ok_or(Overflow)?;
        (num, 1_i64)
    } else if rem4 < d3 {
        let num = rem4.checked_sub(d2).ok_or(Overflow)?;
        (num, -1_i64)
    } else {
        let num = d4.checked_sub(rem4).ok_or(Overflow)?;
        (num, -1_i64)
    };

    let ScaledSine(val) = eval_poly_quarter(QuarterNumer(u_num), QuarterDenom(denom))?;
    let signed_numer = if sign < 0 {
        0_i64.checked_sub(val).ok_or(Overflow)?
    } else {
        val
    };

    let scale_denom = trig_scale_denom();
    Ratio::new(Numerator(signed_numer), scale_denom)
}

/// Computes the cosine of an angle given in turns (where 1 turn = 1 full revolution).
///
/// Because the angle is specified in turns rather than radians, exact fractions
/// of a circle (such as quarter turns or eighth turns) are represented exactly
/// without transcendental rounding.
///
/// # Determinism and Error Bound
///
/// This function uses a deterministic integer fixed-point 7th-degree polynomial
/// approximation producing bit-identical results on all platforms and architectures.
///
/// - Exactly matches expected values at cardinal angles:
///   `cos_turns(0) = 1`, `cos_turns(1/4) = 0`, `cos_turns(1/2) = -1`, `cos_turns(3/4) = 0`.
/// - The maximum absolute error compared to mathematical cosine across the circle is
///   bounded by `6e-5` (`0.00006`).
/// - Satisfies the Pythagorean identity `sin² + cos² = 1` within `1.1e-4` (`0.00011`).
///
/// # Errors
///
/// Returns `Err(Overflow)` if any intermediate arithmetic overflows.
pub fn cos_turns(angle: Ratio) -> Result<Ratio, Overflow> {
    let numer = i128::from(angle.numer().0);
    let denom = NonZeroI128::from(angle.denom()).get();

    let rem_raw = numer.checked_rem(denom).ok_or(Overflow)?;
    let rem = if rem_raw < 0 {
        rem_raw.checked_add(denom).ok_or(Overflow)?
    } else {
        rem_raw
    };

    let rem4 = rem.checked_mul(4).ok_or(Overflow)?;
    let d2 = denom.checked_mul(2).ok_or(Overflow)?;
    let d3 = denom.checked_mul(3).ok_or(Overflow)?;

    let (u_num, sign) = if rem4 < denom {
        let num = denom.checked_sub(rem4).ok_or(Overflow)?;
        (num, 1_i64)
    } else if rem4 < d2 {
        let num = rem4.checked_sub(denom).ok_or(Overflow)?;
        (num, -1_i64)
    } else if rem4 < d3 {
        let num = d3.checked_sub(rem4).ok_or(Overflow)?;
        (num, -1_i64)
    } else {
        let num = rem4.checked_sub(d3).ok_or(Overflow)?;
        (num, 1_i64)
    };

    let ScaledSine(val) = eval_poly_quarter(QuarterNumer(u_num), QuarterDenom(denom))?;
    let signed_numer = if sign < 0 {
        0_i64.checked_sub(val).ok_or(Overflow)?
    } else {
        val
    };

    let scale_denom = trig_scale_denom();
    Ratio::new(Numerator(signed_numer), scale_denom)
}

#[cfg(test)]
mod tests;
