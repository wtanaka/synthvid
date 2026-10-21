use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// `trig_scale_denom` spells `32_768` a second way, to get a `NonZeroI64`
/// without an `Option`. If either spelling changes, the scale silently
/// stops matching the polynomial coefficients, so pin them together.
#[test]
fn trig_scale_denom_matches_trig_scale() {
    assert_eq!(trig_scale_denom().get(), TRIG_SCALE);
}
use super::*;
use crate::testutil::Frac;

#[test]
fn sin_cos_cardinal_angles() {
    assert_eq!(check_sin_cos_cardinal_angles(), Ok(()));
}

fn check_sin_cos_cardinal_angles() -> Result<(), Fail> {
    let zero = Ratio::from_integer(Numerator(0));
    let one = Ratio::from_integer(Numerator(1));
    let neg_one = Ratio::from_integer(Numerator(-1));

    let quarter = Frac(1, 4).ratio()?;
    let half = Frac(1, 2).ratio()?;
    let three_quarters = Frac(3, 4).ratio()?;

    ensure_eq!(sin_turns(zero), Ok(zero), "sin(0) must be 0/1");
    ensure_eq!(cos_turns(zero), Ok(one), "cos(0) must be 1/1");

    ensure_eq!(sin_turns(quarter), Ok(one), "sin(1/4) must be 1/1");
    ensure_eq!(cos_turns(quarter), Ok(zero), "cos(1/4) must be 0/1");

    ensure_eq!(sin_turns(half), Ok(zero), "sin(1/2) must be 0/1");
    ensure_eq!(cos_turns(half), Ok(neg_one), "cos(1/2) must be -1/1");

    ensure_eq!(
        sin_turns(three_quarters),
        Ok(neg_one),
        "sin(3/4) must be -1/1"
    );
    ensure_eq!(cos_turns(three_quarters), Ok(zero), "cos(3/4) must be 0/1");

    // Full revolutions and negative angles
    let neg_quarter = Frac(-1, 4).ratio()?;
    let neg_half = Frac(-1, 2).ratio()?;
    let neg_three_quarters = Frac(-3, 4).ratio()?;
    let two = Ratio::from_integer(Numerator(2));

    ensure_eq!(
        sin_turns(neg_quarter),
        Ok(neg_one),
        "sin(-1/4) must be -1/1"
    );
    ensure_eq!(cos_turns(neg_quarter), Ok(zero), "cos(-1/4) must be 0/1");

    ensure_eq!(sin_turns(neg_half), Ok(zero), "sin(-1/2) must be 0/1");
    ensure_eq!(cos_turns(neg_half), Ok(neg_one), "cos(-1/2) must be -1/1");

    ensure_eq!(
        sin_turns(neg_three_quarters),
        Ok(one),
        "sin(-3/4) must be 1/1"
    );
    ensure_eq!(
        cos_turns(neg_three_quarters),
        Ok(zero),
        "cos(-3/4) must be 0/1"
    );

    ensure_eq!(sin_turns(one), Ok(zero), "sin(1) must be 0/1");
    ensure_eq!(cos_turns(one), Ok(one), "cos(1) must be 1/1");

    ensure_eq!(sin_turns(two), Ok(zero), "sin(2) must be 0/1");
    ensure_eq!(cos_turns(two), Ok(one), "cos(2) must be 1/1");

    ensure_eq!(sin_turns(neg_one), Ok(zero), "sin(-1) must be 0/1");
    ensure_eq!(cos_turns(neg_one), Ok(one), "cos(-1) must be 1/1");

    // Methods on Ratio
    ensure_eq!(
        quarter.sin_turns(),
        Ok(one),
        "quarter.sin_turns() must be 1/1"
    );
    ensure_eq!(
        quarter.cos_turns(),
        Ok(zero),
        "quarter.cos_turns() must be 0/1"
    );
    Ok(())
}

#[test]
fn sin_cos_frozen_table() {
    assert_eq!(check_sin_cos_frozen_table(), Ok(()));
}

/// One row of the frozen table: an angle in turns and the exact sine and
/// cosine the polynomial gives for it, each as numerator and denominator.
type FrozenRow = (i64, i64, i64, i64, i64, i64);

/// The frozen sine and cosine of every odd multiple of `1 / 32` turn.
static FROZEN_SIN_COS: [FrozenRow; 16] = [
    (1, 32, 6393, 0x8000, 16069, 0x4000),
    (3, 32, 18205, 0x8000, 13623, 0x4000),
    (5, 32, 13623, 0x4000, 18205, 0x8000),
    (7, 32, 16069, 0x4000, 6393, 0x8000),
    (9, 32, 16069, 0x4000, -6393, 0x8000),
    (11, 32, 13623, 0x4000, -18205, 0x8000),
    (13, 32, 18205, 0x8000, -13623, 0x4000),
    (15, 32, 6393, 0x8000, -16069, 0x4000),
    (17, 32, -6393, 0x8000, -16069, 0x4000),
    (19, 32, -18205, 0x8000, -13623, 0x4000),
    (21, 32, -13623, 0x4000, -18205, 0x8000),
    (23, 32, -16069, 0x4000, -6393, 0x8000),
    (25, 32, -16069, 0x4000, 6393, 0x8000),
    (27, 32, -13623, 0x4000, 18205, 0x8000),
    (29, 32, -18205, 0x8000, 13623, 0x4000),
    (31, 32, -6393, 0x8000, 16069, 0x4000),
];

fn check_sin_cos_frozen_table() -> Result<(), Fail> {
    for &(an, ad, sn, sd, cn, cd) in &FROZEN_SIN_COS {
        let angle = Frac(an, ad).ratio()?;
        let expected_sin = Frac(sn, sd).ratio()?;
        let expected_cos = Frac(cn, cd).ratio()?;

        let actual_sin = sin_turns(angle).or_fail()?;
        let actual_cos = cos_turns(angle).or_fail()?;

        ensure_eq!(
            actual_sin,
            expected_sin,
            "frozen sin value mismatch for angle"
        );
        ensure_eq!(
            actual_cos,
            expected_cos,
            "frozen cos value mismatch for angle"
        );
    }
    Ok(())
}

#[test]
fn pythagorean_identity_error_bound() {
    assert_eq!(check_pythagorean_identity_error_bound(), Ok(()));
}

fn check_pythagorean_identity_error_bound() -> Result<(), Fail> {
    let one = Ratio::from_integer(Numerator(1));
    // Maximum documented identity error: 1.1e-4 (11 / 100_000)
    let error_bound = Frac(11, 100_000).ratio()?;

    // Test over 128 angles across the unit circle
    for step in 0..=128_i64 {
        let angle = Frac(step, 128).ratio()?;
        let sine = sin_turns(angle).or_fail()?;
        let cosine = cos_turns(angle).or_fail()?;

        let s2 = sine.checked_mul(sine).or_fail()?;
        let c2 = cosine.checked_mul(cosine).or_fail()?;
        let sum = s2.checked_add(c2).or_fail()?;
        let diff = sum.checked_sub(one).or_fail()?;
        let abs_diff = if diff.numer().0 < 0 {
            diff.checked_neg().or_fail()?
        } else {
            diff
        };

        ensure!(
            abs_diff <= error_bound,
            "sin^2 + cos^2 must equal 1 within documented error bound 1.1e-4"
        );
    }
    Ok(())
}
