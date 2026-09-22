//! Platform-independent trigonometry.
//!
//! [`sin_turns`] and [`cos_turns`] take angles in turns and evaluate a fixed
//! degree-7 polynomial in integer arithmetic. The standard library's floating
//! point `sin` and `cos` are deliberately not used: their results differ
//! between targets.

use core::num::{NonZeroI64, NonZeroU32};

use crate::ratio::{Overflow, Ratio};

/// Denominator scale for trigonometric fixed-point representation (2^15 = 32,768).
const TRIG_SCALE: i64 = 32_768;

/// The value of [`TRIG_SCALE`] as a non-zero denominator, built totally.
///
/// `NonZeroU32::MIN` is one, `saturating_add` on a non-zero value cannot reach
/// zero, and widening a `NonZeroU32` to a `NonZeroI64` is infallible, so there
/// is no `Option` to discharge. `trig_scale_denom_matches_trig_scale` pins this
/// to [`TRIG_SCALE`] so the two spellings cannot drift apart.
fn trig_scale_denom() -> NonZeroI64 {
    NonZeroI64::from(NonZeroU32::MIN.saturating_add(32_767))
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
/// `P(u) = (15,708 * u - 6,459 * u^3 + 794 * u^5 - 43 * u^7) / 10,000`
///
/// Exactly satisfies `P(0) = 0`, `P(1) = 1`, and `P'(1) = 0`, ensuring
/// continuous first derivatives across quadrant transitions.
fn eval_poly_quarter(u_num: i128, u_den: i128) -> Result<i64, Overflow> {
    if u_num <= 0 {
        return Ok(0);
    }
    if u_num >= u_den {
        return Ok(TRIG_SCALE);
    }

    let scale_128 = i128::from(TRIG_SCALE);
    let half_den = u_den.checked_div(2).ok_or(Overflow)?;
    let num_x = u_num
        .checked_mul(scale_128)
        .ok_or(Overflow)?
        .checked_add(half_den)
        .ok_or(Overflow)?;
    let x = num_x.checked_div(u_den).ok_or(Overflow)?;
    let x = if x < 0 {
        0
    } else if x > scale_128 {
        scale_128
    } else {
        x
    };

    let x2 = x
        .checked_mul(x)
        .ok_or(Overflow)?
        .checked_div(scale_128)
        .ok_or(Overflow)?;
    let x3 = x2
        .checked_mul(x)
        .ok_or(Overflow)?
        .checked_div(scale_128)
        .ok_or(Overflow)?;
    let x4 = x2
        .checked_mul(x2)
        .ok_or(Overflow)?
        .checked_div(scale_128)
        .ok_or(Overflow)?;
    let x5 = x4
        .checked_mul(x)
        .ok_or(Overflow)?
        .checked_div(scale_128)
        .ok_or(Overflow)?;
    let x6 = x3
        .checked_mul(x3)
        .ok_or(Overflow)?
        .checked_div(scale_128)
        .ok_or(Overflow)?;
    let x7 = x6
        .checked_mul(x)
        .ok_or(Overflow)?
        .checked_div(scale_128)
        .ok_or(Overflow)?;

    let term1 = POLY_A1.checked_mul(x).ok_or(Overflow)?;
    let term3 = POLY_A3.checked_mul(x3).ok_or(Overflow)?;
    let term5 = POLY_A5.checked_mul(x5).ok_or(Overflow)?;
    let term7 = POLY_A7.checked_mul(x7).ok_or(Overflow)?;

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

    let val_128 = poly_val.checked_div(POLY_DENOM).ok_or(Overflow)?;
    let val_clamped = if val_128 < 0 {
        0
    } else if val_128 > scale_128 {
        scale_128
    } else {
        val_128
    };

    i64::try_from(val_clamped).map_err(|_| Overflow)
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
    let numer = i128::from(angle.numer());
    let denom = i128::from(angle.denom().get());

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

    let val = eval_poly_quarter(u_num, denom)?;
    let signed_numer = if sign < 0 {
        0_i64.checked_sub(val).ok_or(Overflow)?
    } else {
        val
    };

    let scale_denom = trig_scale_denom();
    Ratio::new(signed_numer, scale_denom)
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
    let numer = i128::from(angle.numer());
    let denom = i128::from(angle.denom().get());

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

    let val = eval_poly_quarter(u_num, denom)?;
    let signed_numer = if sign < 0 {
        0_i64.checked_sub(val).ok_or(Overflow)?
    } else {
        val
    };

    let scale_denom = trig_scale_denom();
    Ratio::new(signed_numer, scale_denom)
}

#[cfg(test)]
mod tests {

    /// `trig_scale_denom` spells `32_768` a second way, to get a `NonZeroI64`
    /// without an `Option`. If either spelling changes, the scale silently
    /// stops matching the polynomial coefficients, so pin them together.
    #[test]
    fn trig_scale_denom_matches_trig_scale() {
        assert_eq!(trig_scale_denom().get(), TRIG_SCALE);
    }
    use super::*;
    use crate::testutil::make_ratio;

    #[test]
    fn test_sin_cos_cardinal_angles() {
        let zero = Ratio::from_integer(0);
        let one = Ratio::from_integer(1);
        let neg_one = Ratio::from_integer(-1);

        let quarter = make_ratio(1, 4).unwrap();
        let half = make_ratio(1, 2).unwrap();
        let three_quarters = make_ratio(3, 4).unwrap();

        assert_eq!(sin_turns(zero).ok(), Some(zero), "sin(0) must be 0/1");
        assert_eq!(cos_turns(zero).ok(), Some(one), "cos(0) must be 1/1");

        assert_eq!(sin_turns(quarter).ok(), Some(one), "sin(1/4) must be 1/1");
        assert_eq!(cos_turns(quarter).ok(), Some(zero), "cos(1/4) must be 0/1");

        assert_eq!(sin_turns(half).ok(), Some(zero), "sin(1/2) must be 0/1");
        assert_eq!(cos_turns(half).ok(), Some(neg_one), "cos(1/2) must be -1/1");

        assert_eq!(
            sin_turns(three_quarters).ok(),
            Some(neg_one),
            "sin(3/4) must be -1/1"
        );
        assert_eq!(
            cos_turns(three_quarters).ok(),
            Some(zero),
            "cos(3/4) must be 0/1"
        );

        // Full revolutions and negative angles
        let neg_quarter = make_ratio(-1, 4).unwrap();
        let neg_half = make_ratio(-1, 2).unwrap();
        let neg_three_quarters = make_ratio(-3, 4).unwrap();
        let two = Ratio::from_integer(2);

        assert_eq!(
            sin_turns(neg_quarter).ok(),
            Some(neg_one),
            "sin(-1/4) must be -1/1"
        );
        assert_eq!(
            cos_turns(neg_quarter).ok(),
            Some(zero),
            "cos(-1/4) must be 0/1"
        );

        assert_eq!(
            sin_turns(neg_half).ok(),
            Some(zero),
            "sin(-1/2) must be 0/1"
        );
        assert_eq!(
            cos_turns(neg_half).ok(),
            Some(neg_one),
            "cos(-1/2) must be -1/1"
        );

        assert_eq!(
            sin_turns(neg_three_quarters).ok(),
            Some(one),
            "sin(-3/4) must be 1/1"
        );
        assert_eq!(
            cos_turns(neg_three_quarters).ok(),
            Some(zero),
            "cos(-3/4) must be 0/1"
        );

        assert_eq!(sin_turns(one).ok(), Some(zero), "sin(1) must be 0/1");
        assert_eq!(cos_turns(one).ok(), Some(one), "cos(1) must be 1/1");

        assert_eq!(sin_turns(two).ok(), Some(zero), "sin(2) must be 0/1");
        assert_eq!(cos_turns(two).ok(), Some(one), "cos(2) must be 1/1");

        assert_eq!(sin_turns(neg_one).ok(), Some(zero), "sin(-1) must be 0/1");
        assert_eq!(cos_turns(neg_one).ok(), Some(one), "cos(-1) must be 1/1");

        // Methods on Ratio
        assert_eq!(
            quarter.sin_turns().ok(),
            Some(one),
            "quarter.sin_turns() must be 1/1"
        );
        assert_eq!(
            quarter.cos_turns().ok(),
            Some(zero),
            "quarter.cos_turns() must be 0/1"
        );
    }

    #[test]
    fn test_sin_cos_frozen_table() {
        let frozen_entries: [(i64, i64, i64, i64, i64, i64); 16] = [
            (1, 32, 6393, 32768, 16069, 16384),
            (3, 32, 18205, 32768, 13623, 16384),
            (5, 32, 13623, 16384, 18205, 32768),
            (7, 32, 16069, 16384, 6393, 32768),
            (9, 32, 16069, 16384, -6393, 32768),
            (11, 32, 13623, 16384, -18205, 32768),
            (13, 32, 18205, 32768, -13623, 16384),
            (15, 32, 6393, 32768, -16069, 16384),
            (17, 32, -6393, 32768, -16069, 16384),
            (19, 32, -18205, 32768, -13623, 16384),
            (21, 32, -13623, 16384, -18205, 32768),
            (23, 32, -16069, 16384, -6393, 32768),
            (25, 32, -16069, 16384, 6393, 32768),
            (27, 32, -13623, 16384, 18205, 32768),
            (29, 32, -18205, 32768, 13623, 16384),
            (31, 32, -6393, 32768, 16069, 16384),
        ];

        for (an, ad, sn, sd, cn, cd) in frozen_entries {
            let angle = make_ratio(an, ad).unwrap();
            let expected_sin = make_ratio(sn, sd).unwrap();
            let expected_cos = make_ratio(cn, cd).unwrap();

            let actual_sin = sin_turns(angle).ok().unwrap();
            let actual_cos = cos_turns(angle).ok().unwrap();

            assert_eq!(
                actual_sin, expected_sin,
                "frozen sin value mismatch for angle"
            );
            assert_eq!(
                actual_cos, expected_cos,
                "frozen cos value mismatch for angle"
            );
        }
    }

    #[test]
    fn test_pythagorean_identity_error_bound() {
        let one = Ratio::from_integer(1);
        // Maximum documented identity error: 1.1e-4 (11 / 100_000)
        let error_bound = make_ratio(11, 100_000).unwrap();

        // Test over 128 angles across the unit circle
        for k in 0..=128_i64 {
            let angle = make_ratio(k, 128).unwrap();
            let s = sin_turns(angle).ok().unwrap();
            let c = cos_turns(angle).ok().unwrap();

            let s2 = s.checked_mul(s).unwrap();
            let c2 = c.checked_mul(c).unwrap();
            let sum = s2.checked_add(c2).unwrap();
            let diff = sum.checked_sub(one).unwrap();
            let abs_diff = if diff.numer() < 0 {
                diff.checked_neg().unwrap()
            } else {
                diff
            };

            assert!(
                abs_diff <= error_bound,
                "sin^2 + cos^2 must equal 1 within documented error bound 1.1e-4"
            );
        }
    }
}
