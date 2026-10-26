use core::cmp::Ordering;
use core::num::{NonZeroI128, NonZeroI64, NonZeroU16, NonZeroU32, NonZeroU8};

use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

use super::helpers::{compare_doubled_with_product, AxisPixel, DoubledOrder};
use super::{half_ratio, pixel_center_ratio, quarter_ratio, Numerator, Overflow, Ratio};
use crate::rng::Rng;
use crate::units::Seed;

/// `pixel_center_ratio` skips the reducing path, so pin it to the checked
/// construction: if the two ever disagree, derived `Eq` and `Hash` on
/// `Ratio` stop matching the arithmetic.
#[test]
fn pixel_center_ratio_is_the_odd_numerator_over_two() {
    assert_eq!(
        check_pixel_center_ratio_is_the_odd_numerator_over_two(),
        Ok(())
    );
}

fn check_pixel_center_ratio_is_the_odd_numerator_over_two() -> Result<(), Fail> {
    let two = NonZeroI64::new(2).or_fail()?;
    for index in [0_u16, 1, 2, 7, 1000, u16::MAX] {
        let center_ratio = pixel_center_ratio(AxisPixel(index));
        let expected_numer = i64::from(index)
            .checked_mul(2)
            .and_then(|doubled| doubled.checked_add(1))
            .or_fail()?;
        ensure_eq!(
            center_ratio.numer().0,
            expected_numer,
            "numerator for {index}"
        );
        ensure_eq!(center_ratio.denom().get(), 2, "denominator for {index}");

        let checked = Ratio::new(Numerator(expected_numer), two).or_fail()?;
        ensure_eq!(center_ratio, checked, "center of {index}");
    }
    Ok(())
}

/// Draws one test component uniformly from `[-50_000, 50_000]`, the way a
/// `SplitMix64` stream reduced modulo the span width does.
fn draw_component(rng: &mut Rng) -> Result<Component, Fail> {
    let rem = rng.next_u64().0.checked_rem(100_001).or_fail()?;
    let shifted = i64::try_from(rem).or_fail()?;
    Ok(Component(shifted.checked_sub(50_000).or_fail()?))
}

/// A drawn test component, which may still be zero.
#[derive(Clone, Copy, Debug)]
struct Component(i64);

/// Draws a numerator and then a denominator, replacing a zero denominator
/// with one, and builds the ratio they describe.
fn draw_ratio(rng: &mut Rng) -> Result<Ratio, Fail> {
    let Component(numer) = draw_component(rng)?;
    let Component(drawn_den) = draw_component(rng)?;
    let den = match NonZeroI64::new(drawn_den) {
        Some(den) => den,
        None => NonZeroI64::new(1).or_fail()?,
    };
    Ratio::new(Numerator(numer), den).or_fail()
}

#[test]
fn ratio_normalization_equal_values() {
    assert_eq!(check_ratio_normalization_equal_values(), Ok(()));
}

fn check_ratio_normalization_equal_values() -> Result<(), Fail> {
    let d1 = NonZeroI64::new(1).or_fail()?;
    let d2 = NonZeroI64::new(2).or_fail()?;
    let d_neg2 = NonZeroI64::new(-2).or_fail()?;
    let d4 = NonZeroI64::new(4).or_fail()?;
    let d_neg4 = NonZeroI64::new(-4).or_fail()?;
    let d6 = NonZeroI64::new(6).or_fail()?;
    let d200 = NonZeroI64::new(200).or_fail()?;

    let r_half1 = Ratio::new(Numerator(1), d2).or_fail()?;
    let r_half2 = Ratio::new(Numerator(2), d4).or_fail()?;
    let r_half3 = Ratio::new(Numerator(3), d6).or_fail()?;
    let r_half4 = Ratio::new(Numerator(100), d200).or_fail()?;
    let r_half_neg = Ratio::new(Numerator(-2), d_neg4).or_fail()?;

    ensure_eq!(r_half1, r_half2, "1/2 must equal 2/4");
    ensure_eq!(r_half1, r_half3, "1/2 must equal 3/6");
    ensure_eq!(r_half1, r_half4, "1/2 must equal 100/200");
    ensure_eq!(r_half1, r_half_neg, "1/2 must equal -2/-4");

    ensure_eq!(r_half1.numer().0, 1, "numerator of 1/2 must be 1");
    ensure_eq!(r_half1.denom().get(), 2, "denominator of 1/2 must be 2");

    // Negative values sign carried in numerator.
    let r_neg_half1 = Ratio::new(Numerator(-1), d2).or_fail()?;
    let r_neg_half2 = Ratio::new(Numerator(1), d_neg2).or_fail()?;
    let r_neg_half3 = Ratio::new(Numerator(-2), d4).or_fail()?;
    ensure_eq!(r_neg_half1, r_neg_half2, "-1/2 must equal 1/-2");
    ensure_eq!(r_neg_half1, r_neg_half3, "-1/2 must equal -2/4");

    ensure_eq!(
        r_neg_half1.numer().0,
        -1,
        "numerator must carry negative sign"
    );
    ensure_eq!(
        r_neg_half1.denom().get(),
        2,
        "denominator must remain positive"
    );

    // Zero representation is uniquely 0/1.
    let r_zero1 = Ratio::new(Numerator(0), d1).or_fail()?;
    let r_zero2 = Ratio::new(Numerator(0), d4).or_fail()?;
    let r_zero3 = Ratio::new(Numerator(0), d_neg4).or_fail()?;
    ensure_eq!(r_zero1, r_zero2, "0/1 must equal 0/4");
    ensure_eq!(r_zero1, r_zero3, "0/1 must equal 0/-4");

    ensure_eq!(r_zero1.numer().0, 0, "zero numerator must be 0");
    ensure_eq!(r_zero1.denom().get(), 1, "zero denominator must be 1");
    Ok(())
}

#[test]
fn ratio_comparison() {
    assert_eq!(check_ratio_comparison(), Ok(()));
}

fn check_ratio_comparison() -> Result<(), Fail> {
    let d1 = NonZeroI64::new(1).or_fail()?;
    let d2 = NonZeroI64::new(2).or_fail()?;
    let d3 = NonZeroI64::new(3).or_fail()?;

    let r_half = Ratio::new(Numerator(1), d2).or_fail()?;
    let r_third = Ratio::new(Numerator(1), d3).or_fail()?;
    let r_two_thirds = Ratio::new(Numerator(2), d3).or_fail()?;
    let r_neg_one = Ratio::new(Numerator(-1), d1).or_fail()?;

    ensure!(r_third < r_half, "1/3 < 1/2");
    ensure!(r_half < r_two_thirds, "1/2 < 2/3");
    ensure!(r_neg_one < r_third, "-1 < 1/3");
    ensure_eq!(r_half.cmp(&r_half), Ordering::Equal, "1/2 == 1/2");
    Ok(())
}

#[test]
fn ratio_overflow_returns_none() {
    assert_eq!(check_ratio_overflow_returns_none(), Ok(()));
}

fn check_ratio_overflow_returns_none() -> Result<(), Fail> {
    let d1 = NonZeroI64::new(1).or_fail()?;
    let max_ratio = Ratio::new(Numerator(i64::MAX), d1).or_fail()?;
    let one_ratio = Ratio::new(Numerator(1), d1).or_fail()?;
    let min_ratio = Ratio::new(Numerator(i64::MIN), d1).or_fail()?;
    let zero_ratio = Ratio::new(Numerator(0), d1).or_fail()?;

    // Addition overflow
    ensure_eq!(
        max_ratio.checked_add(one_ratio),
        Err(Overflow),
        "i64::MAX + 1 must return Err"
    );

    // Subtraction overflow
    ensure_eq!(
        min_ratio.checked_sub(one_ratio),
        Err(Overflow),
        "i64::MIN - 1 must return Err"
    );

    // Multiplication overflow
    let two_ratio = Ratio::new(Numerator(2), d1).or_fail()?;
    ensure_eq!(
        max_ratio.checked_mul(two_ratio),
        Err(Overflow),
        "i64::MAX * 2 must return Err"
    );

    // Division by zero returns Err
    ensure_eq!(
        one_ratio.checked_div(zero_ratio),
        Err(Overflow),
        "division by zero must return Err"
    );

    // Negation of i64::MIN returns Err
    ensure_eq!(
        min_ratio.checked_neg(),
        Err(Overflow),
        "-i64::MIN must return Err"
    );
    Ok(())
}

/// Cross-multiplies `actual` against the exact fraction `expected_num /
/// expected_den` and requires the two products to agree.
fn check_cross(actual: Ratio, expected: Fraction128) -> Result<(), Fail> {
    let Fraction128(num, den) = expected;
    let actual_cross = i128::from(actual.numer().0).checked_mul(den).or_fail()?;
    let expected_cross = num
        .checked_mul(NonZeroI128::from(actual.denom()).get())
        .or_fail()?;
    ensure_eq!(
        actual_cross,
        expected_cross,
        "result must match exact rational definition"
    );
    Ok(())
}

/// An unreduced fraction (numerator, denominator) computed independently in
/// 128-bit integers.
#[derive(Clone, Copy, Debug)]
struct Fraction128(i128, i128);

/// Checks `(r1 + r2) - r2 == r1` and the sum against its 128-bit definition.
fn check_add_inverse(pair: [Ratio; 2]) -> Result<(), Fail> {
    let [r1, r2] = pair;
    match r1.checked_add(r2) {
        Ok(sum) => {
            ensure_eq!(sum.checked_sub(r2), Ok(r1), "(r1 + r2) - r2 must be r1");
            let n1 = i128::from(r1.numer().0);
            let d1 = NonZeroI128::from(r1.denom()).get();
            let n2 = i128::from(r2.numer().0);
            let d2 = NonZeroI128::from(r2.denom()).get();
            let num = n1
                .checked_mul(d2)
                .or_fail()?
                .checked_add(n2.checked_mul(d1).or_fail()?)
                .or_fail()?;
            let den = d1.checked_mul(d2).or_fail()?;
            check_cross(sum, Fraction128(num, den))
        }
        Err(Overflow) => Ok(()),
    }
}

/// Checks `(r1 * r2) / r2 == r1` for a non-zero `r2` and the product against
/// its 128-bit definition.
fn check_mul_inverse(pair: [Ratio; 2]) -> Result<(), Fail> {
    let [r1, r2] = pair;
    if r2.numer().0 == 0 {
        return Ok(());
    }
    match r1.checked_mul(r2) {
        Ok(prod) => {
            ensure_eq!(prod.checked_div(r2), Ok(r1), "(r1 * r2) / r2 must be r1");
            let num = i128::from(r1.numer().0)
                .checked_mul(i128::from(r2.numer().0))
                .or_fail()?;
            let den = NonZeroI128::from(r1.denom())
                .get()
                .checked_mul(NonZeroI128::from(r2.denom()).get())
                .or_fail()?;
            check_cross(prod, Fraction128(num, den))
        }
        Err(Overflow) => Ok(()),
    }
}

#[test]
fn exact_arithmetic_random_seeded_cases() {
    assert_eq!(check_exact_arithmetic_random_seeded_cases(), Ok(()));
}

fn check_exact_arithmetic_random_seeded_cases() -> Result<(), Fail> {
    let mut rng = Rng::from_seed(Seed::new(0xd37e_84a1_55f2_9c0b));

    let mut success_count = 0_usize;
    for _ in 0_u32..500 {
        let r1 = draw_ratio(&mut rng)?;
        let r2 = draw_ratio(&mut rng)?;
        check_add_inverse([r1, r2])?;
        check_mul_inverse([r1, r2])?;
        success_count = success_count.checked_add(1).or_fail()?;
    }

    ensure!(
        success_count >= 400,
        "must execute at least 400 random rational cases"
    );
    Ok(())
}

#[test]
fn int_ratio_exact_values() {
    // Test that int_ratio produces exactly n / 1 for any i64.
    assert_eq!(Ratio::from_integer(Numerator(0)).numer().0, 0);
    assert_eq!(Ratio::from_integer(Numerator(0)).denom().get(), 1);

    assert_eq!(Ratio::from_integer(Numerator(1)).numer().0, 1);
    assert_eq!(Ratio::from_integer(Numerator(1)).denom().get(), 1);

    assert_eq!(Ratio::from_integer(Numerator(-1)).numer().0, -1);
    assert_eq!(Ratio::from_integer(Numerator(-1)).denom().get(), 1);

    assert_eq!(Ratio::from_integer(Numerator(42)).numer().0, 42);
    assert_eq!(Ratio::from_integer(Numerator(42)).denom().get(), 1);

    assert_eq!(Ratio::from_integer(Numerator(-42)).numer().0, -42);
    assert_eq!(Ratio::from_integer(Numerator(-42)).denom().get(), 1);

    // Test boundary values.
    assert_eq!(Ratio::from_integer(Numerator(i64::MAX)).numer().0, i64::MAX);
    assert_eq!(Ratio::from_integer(Numerator(i64::MAX)).denom().get(), 1);

    assert_eq!(Ratio::from_integer(Numerator(i64::MIN)).numer().0, i64::MIN);
    assert_eq!(Ratio::from_integer(Numerator(i64::MIN)).denom().get(), 1);
}

#[test]
fn half_ratio_exact_value() {
    // Test that half_ratio produces exactly 1 / 2.
    assert_eq!(half_ratio().numer().0, 1);
    assert_eq!(half_ratio().denom().get(), 2);
}

#[test]
fn quarter_ratio_exact_value() {
    // Test that quarter_ratio produces exactly 1 / 4.
    assert_eq!(quarter_ratio().numer().0, 1);
    assert_eq!(quarter_ratio().denom().get(), 4);
}

#[test]
fn doubled_distance_is_compared_with_the_focal_product_exactly() {
    assert_eq!(
        check_doubled_distance_is_compared_with_the_focal_product_exactly(),
        Ok(())
    );
}

fn check_doubled_distance_is_compared_with_the_focal_product_exactly() -> Result<(), Fail> {
    // The product of the factor 5 and the length 1080 is 5400. Twice 2699 is 5398, below it, and
    // twice 2700 is 5400, which is not below it.
    let factor = NonZeroU8::new(5).or_fail()?;
    let length = NonZeroU16::new(1080).or_fail()?;
    ensure_eq!(
        compare_doubled_with_product(NonZeroU32::new(2699).or_fail()?, factor, length),
        DoubledOrder::Below
    );
    ensure_eq!(
        compare_doubled_with_product(NonZeroU32::new(2700).or_fail()?, factor, length),
        DoubledOrder::AtOrAbove
    );
    // The product of the factor 5 and the odd length 1081 is 5405. Twice 2702 is 5404, below it,
    // and twice 2703 is 5406, which is not below it.
    let odd_length = NonZeroU16::new(1081).or_fail()?;
    ensure_eq!(
        compare_doubled_with_product(NonZeroU32::new(2702).or_fail()?, factor, odd_length),
        DoubledOrder::Below
    );
    ensure_eq!(
        compare_doubled_with_product(NonZeroU32::new(2703).or_fail()?, factor, odd_length),
        DoubledOrder::AtOrAbove
    );
    // Twice the largest distance, 2^33 - 2, is above the largest product, 255 * 65535.
    ensure_eq!(
        compare_doubled_with_product(NonZeroU32::MAX, NonZeroU8::MAX, NonZeroU16::MAX),
        DoubledOrder::AtOrAbove
    );
    Ok(())
}
