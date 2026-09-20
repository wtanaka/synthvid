//! Platform-independent trigonometry.
//!
//! [`sin_turns`] and [`cos_turns`] take angles in turns and evaluate a fixed
//! degree-7 polynomial in integer arithmetic. The standard library's floating
//! point `sin` and `cos` are deliberately not used: their results differ
//! between targets.

use core::num::NonZeroI64;

use crate::ratio::{int_ratio, Ratio};

/// Denominator scale for trigonometric fixed-point representation (2^15 = 32,768).
const TRIG_SCALE: i64 = 32_768;

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
/// # Algorithm and Error
///
/// Uses the 7th-degree polynomial:
/// `P(u) = (15,708 * u - 6,459 * u^3 + 794 * u^5 - 43 * u^7) / 10,000`
///
/// Exactly satisfies `P(0) = 0`, `P(1) = 1`, and `P'(1) = 0`, ensuring
/// continuous first derivatives across quadrant transitions.
fn eval_poly_quarter(u_num: i128, u_den: i128) -> i64 {
    if u_num <= 0 {
        return 0;
    }
    if u_num >= u_den {
        return TRIG_SCALE;
    }

    let scale_128 = i128::from(TRIG_SCALE);
    let half_den = u_den.checked_div(2).unwrap_or_default();
    let num_x = u_num
        .checked_mul(scale_128)
        .and_then(|prod| prod.checked_add(half_den))
        .unwrap_or(u_num);
    let x = num_x.checked_div(u_den).unwrap_or_default();
    let x = if x < 0 {
        0
    } else if x > scale_128 {
        scale_128
    } else {
        x
    };

    let x2 = x
        .checked_mul(x)
        .and_then(|p| p.checked_div(scale_128))
        .unwrap_or_default();
    let x3 = x2
        .checked_mul(x)
        .and_then(|p| p.checked_div(scale_128))
        .unwrap_or_default();
    let x4 = x2
        .checked_mul(x2)
        .and_then(|p| p.checked_div(scale_128))
        .unwrap_or_default();
    let x5 = x4
        .checked_mul(x)
        .and_then(|p| p.checked_div(scale_128))
        .unwrap_or_default();
    let x6 = x3
        .checked_mul(x3)
        .and_then(|p| p.checked_div(scale_128))
        .unwrap_or_default();
    let x7 = x6
        .checked_mul(x)
        .and_then(|p| p.checked_div(scale_128))
        .unwrap_or_default();

    let term1 = POLY_A1.checked_mul(x).unwrap_or_default();
    let term3 = POLY_A3.checked_mul(x3).unwrap_or_default();
    let term5 = POLY_A5.checked_mul(x5).unwrap_or_default();
    let term7 = POLY_A7.checked_mul(x7).unwrap_or_default();

    let half_poly = POLY_DENOM.checked_div(2).unwrap_or_default();
    let poly_val = term1
        .checked_sub(term3)
        .and_then(|v| v.checked_add(term5))
        .and_then(|v| v.checked_sub(term7))
        .and_then(|v| v.checked_add(half_poly))
        .unwrap_or(term1);

    let val_128 = poly_val.checked_div(POLY_DENOM).unwrap_or_default();
    let val_clamped = if val_128 < 0 {
        0
    } else if val_128 > scale_128 {
        scale_128
    } else {
        val_128
    };

    i64::try_from(val_clamped).unwrap_or_default()
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
#[must_use]
pub fn sin_turns(angle: Ratio) -> Ratio {
    let numer = i128::from(angle.numer());
    let denom = i128::from(angle.denom().get());

    let rem_raw = numer.checked_rem(denom).unwrap_or_default();
    let rem = if rem_raw < 0 {
        rem_raw.checked_add(denom).unwrap_or_default()
    } else {
        rem_raw
    };

    let rem4 = rem.checked_mul(4).unwrap_or(rem);
    let d2 = denom.checked_mul(2).unwrap_or(denom);
    let d3 = denom.checked_mul(3).unwrap_or(denom);
    let d4 = denom.checked_mul(4).unwrap_or(denom);

    let (u_num, sign) = if rem4 < denom {
        (rem4, 1_i64)
    } else if rem4 < d2 {
        let num = d2.checked_sub(rem4).unwrap_or_default();
        (num, 1_i64)
    } else if rem4 < d3 {
        let num = rem4.checked_sub(d2).unwrap_or_default();
        (num, -1_i64)
    } else {
        let num = d4.checked_sub(rem4).unwrap_or_default();
        (num, -1_i64)
    };

    let val = eval_poly_quarter(u_num, denom);
    let signed_numer = if sign < 0 {
        0_i64.checked_sub(val).unwrap_or_default()
    } else {
        val
    };

    let fallback_denom = NonZeroI64::new(1).unwrap_or(NonZeroI64::MIN);
    let scale_denom = NonZeroI64::new(TRIG_SCALE).unwrap_or(fallback_denom);
    Ratio::new(signed_numer, scale_denom).unwrap_or_else(|| int_ratio(0))
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
#[must_use]
pub fn cos_turns(angle: Ratio) -> Ratio {
    let numer = i128::from(angle.numer());
    let denom = i128::from(angle.denom().get());

    let rem_raw = numer.checked_rem(denom).unwrap_or_default();
    let rem = if rem_raw < 0 {
        rem_raw.checked_add(denom).unwrap_or_default()
    } else {
        rem_raw
    };

    let rem4 = rem.checked_mul(4).unwrap_or(rem);
    let d2 = denom.checked_mul(2).unwrap_or(denom);
    let d3 = denom.checked_mul(3).unwrap_or(denom);

    let (u_num, sign) = if rem4 < denom {
        let num = denom.checked_sub(rem4).unwrap_or_default();
        (num, 1_i64)
    } else if rem4 < d2 {
        let num = rem4.checked_sub(denom).unwrap_or_default();
        (num, -1_i64)
    } else if rem4 < d3 {
        let num = d3.checked_sub(rem4).unwrap_or_default();
        (num, -1_i64)
    } else {
        let num = rem4.checked_sub(d3).unwrap_or_default();
        (num, 1_i64)
    };

    let val = eval_poly_quarter(u_num, denom);
    let signed_numer = if sign < 0 {
        0_i64.checked_sub(val).unwrap_or_default()
    } else {
        val
    };

    let fallback_denom = NonZeroI64::new(1).unwrap_or(NonZeroI64::MIN);
    let scale_denom = NonZeroI64::new(TRIG_SCALE).unwrap_or(fallback_denom);
    Ratio::new(signed_numer, scale_denom).unwrap_or_else(|| int_ratio(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::make_ratio;

    #[test]
    fn test_sin_cos_cardinal_angles() {
        let Some(zero) = Ratio::from_integer(0) else {
            return;
        };
        let Some(one) = Ratio::from_integer(1) else {
            return;
        };
        let Some(neg_one) = Ratio::from_integer(-1) else {
            return;
        };

        let Some(quarter) = make_ratio(1, 4) else {
            return;
        };
        let Some(half) = make_ratio(1, 2) else {
            return;
        };
        let Some(three_quarters) = make_ratio(3, 4) else {
            return;
        };

        assert_eq!(sin_turns(zero), zero, "sin(0) must be 0/1");
        assert_eq!(cos_turns(zero), one, "cos(0) must be 1/1");

        assert_eq!(sin_turns(quarter), one, "sin(1/4) must be 1/1");
        assert_eq!(cos_turns(quarter), zero, "cos(1/4) must be 0/1");

        assert_eq!(sin_turns(half), zero, "sin(1/2) must be 0/1");
        assert_eq!(cos_turns(half), neg_one, "cos(1/2) must be -1/1");

        assert_eq!(sin_turns(three_quarters), neg_one, "sin(3/4) must be -1/1");
        assert_eq!(cos_turns(three_quarters), zero, "cos(3/4) must be 0/1");

        // Full revolutions and negative angles
        let Some(neg_quarter) = make_ratio(-1, 4) else {
            return;
        };
        let Some(neg_half) = make_ratio(-1, 2) else {
            return;
        };
        let Some(neg_three_quarters) = make_ratio(-3, 4) else {
            return;
        };
        let Some(two) = Ratio::from_integer(2) else {
            return;
        };

        assert_eq!(sin_turns(neg_quarter), neg_one, "sin(-1/4) must be -1/1");
        assert_eq!(cos_turns(neg_quarter), zero, "cos(-1/4) must be 0/1");

        assert_eq!(sin_turns(neg_half), zero, "sin(-1/2) must be 0/1");
        assert_eq!(cos_turns(neg_half), neg_one, "cos(-1/2) must be -1/1");

        assert_eq!(sin_turns(neg_three_quarters), one, "sin(-3/4) must be 1/1");
        assert_eq!(cos_turns(neg_three_quarters), zero, "cos(-3/4) must be 0/1");

        assert_eq!(sin_turns(one), zero, "sin(1) must be 0/1");
        assert_eq!(cos_turns(one), one, "cos(1) must be 1/1");

        assert_eq!(sin_turns(two), zero, "sin(2) must be 0/1");
        assert_eq!(cos_turns(two), one, "cos(2) must be 1/1");

        assert_eq!(sin_turns(neg_one), zero, "sin(-1) must be 0/1");
        assert_eq!(cos_turns(neg_one), one, "cos(-1) must be 1/1");

        // Methods on Ratio
        assert_eq!(quarter.sin_turns(), one, "quarter.sin_turns() must be 1/1");
        assert_eq!(quarter.cos_turns(), zero, "quarter.cos_turns() must be 0/1");
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
            let Some(angle) = make_ratio(an, ad) else {
                return;
            };
            let Some(expected_sin) = make_ratio(sn, sd) else {
                return;
            };
            let Some(expected_cos) = make_ratio(cn, cd) else {
                return;
            };

            let actual_sin = sin_turns(angle);
            let actual_cos = cos_turns(angle);

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
        let Some(one) = Ratio::from_integer(1) else {
            return;
        };
        // Maximum documented identity error: 1.1e-4 (11 / 100_000)
        let Some(error_bound) = make_ratio(11, 100_000) else {
            return;
        };

        // Test over 128 angles across the unit circle
        for k in 0..=128_i64 {
            let Some(angle) = make_ratio(k, 128) else {
                return;
            };
            let s = sin_turns(angle);
            let c = cos_turns(angle);

            let Some(s2) = s.checked_mul(s) else {
                return;
            };
            let Some(c2) = c.checked_mul(c) else {
                return;
            };
            let Some(sum) = s2.checked_add(c2) else {
                return;
            };
            let Some(diff) = sum.checked_sub(one) else {
                return;
            };
            let abs_diff = if diff.numer() < 0 {
                let Some(neg) = diff.checked_neg() else {
                    return;
                };
                neg
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
