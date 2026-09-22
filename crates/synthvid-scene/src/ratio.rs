//! Exact rational arithmetic.
//!
//! [`Ratio`] is the numeric type every other module computes in: a normalised
//! `i64 / NonZeroI64` fraction whose operations are checked and exact, so a
//! value never drifts between platforms or releases.

use core::cmp::Ordering;
use core::fmt;
use core::num::{NonZeroI64, NonZeroU8};

use crate::trig::{cos_turns, sin_turns};

/// Exact arithmetic could not represent a result.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Overflow;

impl fmt::Display for Overflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exact arithmetic could not represent a result")
    }
}

impl core::error::Error for Overflow {}

/// Constructs a `NonZeroI64` with value 1.
///
/// Uses `NonZeroU8::MIN` (which is 1) and converts it via `From`, which is
/// infallible for unsigned non-zero types. This avoids any `Option` or fallible
/// construction.
fn nonzero_one() -> NonZeroI64 {
    NonZeroI64::from(NonZeroU8::MIN)
}

/// Greatest common divisor for two 128-bit unsigned integers.
const fn gcd_u128(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let rem = match a.checked_rem(b) {
            Some(r) => r,
            None => 0,
        };
        a = b;
        b = rem;
    }
    a
}

/// Normalises an absolute numerator and denominator with a sign flag into a canonical [`Ratio`].
///
/// # Errors
///
/// Returns `Err(Overflow)` if `abs_den` is 0 or if the resulting reduced rational cannot be
/// represented with an `i64` numerator and positive [`NonZeroI64`] denominator.
fn from_u128_parts(abs_num: u128, abs_den: u128, is_negative: bool) -> Result<Ratio, Overflow> {
    if abs_den == 0 {
        return Err(Overflow);
    }
    if abs_num == 0 {
        return Ok(Ratio {
            numer: 0,
            denom: nonzero_one(),
        });
    }

    let g = gcd_u128(abs_num, abs_den);
    if g == 0 {
        return Err(Overflow);
    }
    let red_num = abs_num.checked_div(g).ok_or(Overflow)?;
    let red_den = abs_den.checked_div(g).ok_or(Overflow)?;

    // Denominator must be strictly positive and fit in positive i64.
    let den_i64 = i64::try_from(red_den).ok().ok_or(Overflow)?;
    if den_i64 <= 0 {
        return Err(Overflow);
    }
    let non_zero_den = NonZeroI64::new(den_i64).ok_or(Overflow)?;

    // Magnitude of i64::MIN is 2^63.
    let min_i64_mag = u128::from(i64::MIN.unsigned_abs());

    let numer_i64 = if is_negative {
        match red_num.cmp(&min_i64_mag) {
            Ordering::Equal => i64::MIN,
            Ordering::Less => {
                let pos = i64::try_from(red_num).ok().ok_or(Overflow)?;
                0_i64.checked_sub(pos).ok_or(Overflow)?
            }
            Ordering::Greater => return Err(Overflow),
        }
    } else {
        i64::try_from(red_num).ok().ok_or(Overflow)?
    };

    Ok(Ratio {
        numer: numer_i64,
        denom: non_zero_den,
    })
}

/// A normalised rational number over 64-bit signed integers.
///
/// The denominator is guaranteed to be strictly positive ([`NonZeroI64`]) and
/// any sign is carried entirely in the numerator. Ratios are always kept in
/// lowest terms (`gcd(|numerator|, denominator) == 1`). Zero is uniquely
/// represented as `0 / 1`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Ratio {
    /// The numerator of the rational number in lowest terms.
    numer: i64,
    /// The denominator, strictly positive and in lowest terms.
    denom: NonZeroI64,
}

impl Ratio {
    /// Creates a normalised rational from a numerator and a non-zero denominator.
    ///
    /// Automatically reduces the rational to lowest terms and moves any sign to the numerator.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if the reduced rational cannot fit within `i64` bounds.
    pub fn new(numer: i64, denom: NonZeroI64) -> Result<Self, Overflow> {
        let num128 = i128::from(numer);
        let den128 = i128::from(denom.get());
        let is_negative = (num128 < 0) ^ (den128 < 0);
        from_u128_parts(num128.unsigned_abs(), den128.unsigned_abs(), is_negative)
    }

    /// Creates a rational representing the integer `n / 1`.
    ///
    /// `gcd(n, 1) == 1`, so `n / 1` is already in lowest terms and needs no
    /// reduction. The denominator is built without an `Option` to discharge,
    /// so this construction is total for every `i64`, including `i64::MIN`.
    #[must_use]
    pub fn from_integer(n: i64) -> Self {
        Self {
            numer: n,
            denom: nonzero_one(),
        }
    }

    /// Returns the numerator of the rational number.
    #[must_use]
    pub const fn numer(self) -> i64 {
        self.numer
    }

    /// Returns the positive, non-zero denominator of the rational number.
    #[must_use]
    pub const fn denom(self) -> NonZeroI64 {
        self.denom
    }

    /// Checked addition.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if intermediate or final values overflow `i64`.
    pub fn checked_add(self, rhs: Self) -> Result<Self, Overflow> {
        let n1 = i128::from(self.numer);
        let d1 = i128::from(self.denom.get());
        let n2 = i128::from(rhs.numer);
        let d2 = i128::from(rhs.denom.get());

        let term1 = n1.checked_mul(d2).ok_or(Overflow)?;
        let term2 = n2.checked_mul(d1).ok_or(Overflow)?;
        let num128 = term1.checked_add(term2).ok_or(Overflow)?;
        let den128 = d1.checked_mul(d2).ok_or(Overflow)?;

        let is_negative = num128 < 0;
        from_u128_parts(num128.unsigned_abs(), den128.unsigned_abs(), is_negative)
    }

    /// Checked subtraction.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if intermediate or final values overflow `i64`.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, Overflow> {
        let n1 = i128::from(self.numer);
        let d1 = i128::from(self.denom.get());
        let n2 = i128::from(rhs.numer);
        let d2 = i128::from(rhs.denom.get());

        let term1 = n1.checked_mul(d2).ok_or(Overflow)?;
        let term2 = n2.checked_mul(d1).ok_or(Overflow)?;
        let num128 = term1.checked_sub(term2).ok_or(Overflow)?;
        let den128 = d1.checked_mul(d2).ok_or(Overflow)?;

        let is_negative = num128 < 0;
        from_u128_parts(num128.unsigned_abs(), den128.unsigned_abs(), is_negative)
    }

    /// Checked multiplication.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if intermediate or final values overflow `i64`.
    pub fn checked_mul(self, rhs: Self) -> Result<Self, Overflow> {
        let n1 = i128::from(self.numer);
        let d1 = i128::from(self.denom.get());
        let n2 = i128::from(rhs.numer);
        let d2 = i128::from(rhs.denom.get());

        let num128 = n1.checked_mul(n2).ok_or(Overflow)?;
        let den128 = d1.checked_mul(d2).ok_or(Overflow)?;

        let is_negative = (num128 < 0) ^ (den128 < 0);
        from_u128_parts(num128.unsigned_abs(), den128.unsigned_abs(), is_negative)
    }

    /// Checked division.
    ///
    /// # Errors
    ///
    /// Returns [`Overflow`] if the divisor is zero or division overflows.
    pub fn checked_div(self, rhs: Self) -> Result<Self, Overflow> {
        if rhs.numer == 0 {
            return Err(Overflow);
        }
        let n1 = i128::from(self.numer);
        let d1 = i128::from(self.denom.get());
        let n2 = i128::from(rhs.numer);
        let d2 = i128::from(rhs.denom.get());

        let num128 = n1.checked_mul(d2).ok_or(Overflow)?;
        let den128 = d1.checked_mul(n2).ok_or(Overflow)?;

        let is_negative = (num128 < 0) ^ (den128 < 0);
        from_u128_parts(num128.unsigned_abs(), den128.unsigned_abs(), is_negative)
    }

    /// Checked negation.
    ///
    /// # Errors
    ///
    /// Returns [`Overflow`] if negation overflows.
    pub fn checked_neg(self) -> Result<Self, Overflow> {
        let neg_num = 0_i64.checked_sub(self.numer).ok_or(Overflow)?;
        Ok(Self {
            numer: neg_num,
            denom: self.denom,
        })
    }

    /// Computes the sine of this angle given in turns.
    ///
    /// See [`sin_turns`] for precision, error bounds, and algorithm details.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if any intermediate arithmetic overflows.
    pub fn sin_turns(self) -> Result<Self, Overflow> {
        sin_turns(self)
    }

    /// Computes the cosine of this angle given in turns.
    ///
    /// See [`cos_turns`] for precision, error bounds, and algorithm details.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if any intermediate arithmetic overflows.
    pub fn cos_turns(self) -> Result<Self, Overflow> {
        cos_turns(self)
    }
}

impl Ord for Ratio {
    fn cmp(&self, other: &Self) -> Ordering {
        // Since both denominators are strictly positive, a/b <=> c/d
        // is equivalent to a*d <=> c*b.
        // i64 * i64 products never overflow i128.
        let left = i128::from(self.numer).checked_mul(i128::from(other.denom.get()));
        let right = i128::from(other.numer).checked_mul(i128::from(self.denom.get()));
        match (left, right) {
            (Some(l), Some(r)) => l.cmp(&r),
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for Ratio {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Ratio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numer, self.denom)
    }
}

pub mod helpers;
pub(crate) use helpers::{
    ceil_ratio, floor_ratio, half_ratio, int_ratio, pixel_centre_ratio, quarter_ratio,
};

#[cfg(test)]
mod tests {

    /// `pixel_centre_ratio` skips the reducing path, so pin it to the checked
    /// construction: if the two ever disagree, derived `Eq` and `Hash` on
    /// `Ratio` stop matching the arithmetic.
    #[test]
    fn pixel_centre_ratio_is_the_odd_numerator_over_two() {
        for index in [0_u16, 1, 2, 7, 1000, u16::MAX] {
            let r = pixel_centre_ratio(index);
            let expected_numer = i64::from(index)
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .expect("2 * u16 + 1 is representable");
            assert_eq!(r.numer(), expected_numer, "numerator for {index}");
            assert_eq!(r.denom().get(), 2, "denominator for {index}");

            let two = NonZeroI64::from(NonZeroU8::MIN.saturating_add(1));
            let checked = Ratio::new(expected_numer, two).expect("representable");
            assert_eq!(r, checked, "centre of {index}");
        }
    }

    use super::*;
    use core::num::NonZeroI64;

    /// Deterministic pseudo-random number generator for tests.
    #[derive(Copy, Clone, Debug)]
    struct TestPrng {
        /// PRNG 64-bit state.
        state: u64,
    }

    impl TestPrng {
        /// Creates a PRNG from a seed.
        const fn new(seed: u64) -> Self {
            Self { state: seed }
        }

        /// Generates the next pseudo-random `u64`.
        fn next_u64(&mut self) -> u64 {
            // Standard SplitMix64 step.
            self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        /// Generates a pseudo-random `i64` in `[min, max]`.
        fn next_range(&mut self, min: i64, max: i64) -> i64 {
            let span = max.wrapping_sub(min);
            let u_span = span.unsigned_abs();
            let rand = self.next_u64();
            let rem = rand.checked_rem(u_span.wrapping_add(1)).unwrap();
            let rem_i64 = i64::try_from(rem).unwrap();
            min.wrapping_add(rem_i64)
        }
    }

    #[test]
    fn test_ratio_normalisation_equal_values() {
        let d1 = NonZeroI64::new(1).unwrap();
        let d2 = NonZeroI64::new(2).unwrap();
        let d_neg2 = NonZeroI64::new(-2).unwrap();
        let d4 = NonZeroI64::new(4).unwrap();
        let d_neg4 = NonZeroI64::new(-4).unwrap();
        let d6 = NonZeroI64::new(6).unwrap();
        let d200 = NonZeroI64::new(200).unwrap();

        let r_half1 = Ratio::new(1, d2).ok().unwrap();
        let r_half2 = Ratio::new(2, d4).ok().unwrap();
        let r_half3 = Ratio::new(3, d6).ok().unwrap();
        let r_half4 = Ratio::new(100, d200).ok().unwrap();
        let r_half_neg = Ratio::new(-2, d_neg4).ok().unwrap();

        assert_eq!(r_half1, r_half2, "1/2 must equal 2/4");
        assert_eq!(r_half1, r_half3, "1/2 must equal 3/6");
        assert_eq!(r_half1, r_half4, "1/2 must equal 100/200");
        assert_eq!(r_half1, r_half_neg, "1/2 must equal -2/-4");

        assert_eq!(r_half1.numer(), 1, "numerator of 1/2 must be 1");
        assert_eq!(r_half1.denom().get(), 2, "denominator of 1/2 must be 2");

        // Negative values sign carried in numerator.
        let r_neg_half1 = Ratio::new(-1, d2).ok().unwrap();
        let r_neg_half2 = Ratio::new(1, d_neg2).ok().unwrap();
        let r_neg_half3 = Ratio::new(-2, d4).ok().unwrap();
        assert_eq!(r_neg_half1, r_neg_half2, "-1/2 must equal 1/-2");
        assert_eq!(r_neg_half1, r_neg_half3, "-1/2 must equal -2/4");

        assert_eq!(
            r_neg_half1.numer(),
            -1,
            "numerator must carry negative sign"
        );
        assert_eq!(
            r_neg_half1.denom().get(),
            2,
            "denominator must remain positive"
        );

        // Zero representation is uniquely 0/1.
        let r_zero1 = Ratio::new(0, d1).ok().unwrap();
        let r_zero2 = Ratio::new(0, d4).ok().unwrap();
        let r_zero3 = Ratio::new(0, d_neg4).ok().unwrap();
        assert_eq!(r_zero1, r_zero2, "0/1 must equal 0/4");
        assert_eq!(r_zero1, r_zero3, "0/1 must equal 0/-4");

        assert_eq!(r_zero1.numer(), 0, "zero numerator must be 0");
        assert_eq!(r_zero1.denom().get(), 1, "zero denominator must be 1");
    }

    #[test]
    fn test_ratio_comparison() {
        let d1 = NonZeroI64::new(1).unwrap();
        let d2 = NonZeroI64::new(2).unwrap();
        let d3 = NonZeroI64::new(3).unwrap();

        let r_half = Ratio::new(1, d2).ok().unwrap();
        let r_third = Ratio::new(1, d3).ok().unwrap();
        let r_two_thirds = Ratio::new(2, d3).ok().unwrap();
        let r_neg_one = Ratio::new(-1, d1).ok().unwrap();

        assert!(r_third < r_half, "1/3 < 1/2");
        assert!(r_half < r_two_thirds, "1/2 < 2/3");
        assert!(r_neg_one < r_third, "-1 < 1/3");
        assert_eq!(r_half.cmp(&r_half), Ordering::Equal, "1/2 == 1/2");
    }

    #[test]
    fn test_ratio_overflow_returns_none() {
        let d1 = NonZeroI64::new(1).unwrap();
        let max_ratio = Ratio::new(i64::MAX, d1).ok().unwrap();
        let one_ratio = Ratio::new(1, d1).ok().unwrap();
        let min_ratio = Ratio::new(i64::MIN, d1).ok().unwrap();
        let zero_ratio = Ratio::new(0, d1).ok().unwrap();

        // Addition overflow
        assert!(
            max_ratio.checked_add(one_ratio).is_err(),
            "i64::MAX + 1 must return Err"
        );

        // Subtraction overflow
        assert!(
            min_ratio.checked_sub(one_ratio).is_err(),
            "i64::MIN - 1 must return Err"
        );

        // Multiplication overflow
        let two_ratio = Ratio::new(2, d1).ok().unwrap();
        assert!(
            max_ratio.checked_mul(two_ratio).is_err(),
            "i64::MAX * 2 must return Err"
        );

        // Division by zero returns Err
        assert!(
            one_ratio.checked_div(zero_ratio).is_err(),
            "division by zero must return Err"
        );

        // Negation of i64::MIN returns Err
        assert!(
            min_ratio.checked_neg().is_err(),
            "-i64::MIN must return Err"
        );
    }

    #[test]
    fn test_exact_arithmetic_random_seeded_cases() {
        let mut rng = TestPrng::new(0xd37e_84a1_55f2_9c0b);

        let mut success_count = 0_usize;
        for _ in 0..500 {
            let num1 = rng.next_range(-50_000, 50_000);
            let mut den1 = rng.next_range(-50_000, 50_000);
            if den1 == 0 {
                den1 = 1;
            }

            let num2 = rng.next_range(-50_000, 50_000);
            let mut den2 = rng.next_range(-50_000, 50_000);
            if den2 == 0 {
                den2 = 1;
            }

            let Some(nz_den1) = NonZeroI64::new(den1) else {
                continue;
            };
            let Some(nz_den2) = NonZeroI64::new(den2) else {
                continue;
            };

            let Ok(r1) = Ratio::new(num1, nz_den1) else {
                continue;
            };
            let Ok(r2) = Ratio::new(num2, nz_den2) else {
                continue;
            };

            // Test add & sub inverse: (r1 + r2) - r2 == r1
            if let Ok(sum) = r1.checked_add(r2) {
                let diff = sum.checked_sub(r2);
                assert_eq!(diff, Ok(r1), "(r1 + r2) - r2 must be r1");

                // Cross-check arithmetic against i128 definition:
                let expected_num = i128::from(r1.numer())
                    .wrapping_mul(i128::from(r2.denom().get()))
                    .wrapping_add(
                        i128::from(r2.numer()).wrapping_mul(i128::from(r1.denom().get())),
                    );
                let expected_den =
                    i128::from(r1.denom().get()).wrapping_mul(i128::from(r2.denom().get()));

                let actual_cross = i128::from(sum.numer()).wrapping_mul(expected_den);
                let expected_cross = expected_num.wrapping_mul(i128::from(sum.denom().get()));
                assert_eq!(
                    actual_cross, expected_cross,
                    "sum must match exact rational definition"
                );
            }

            // Test mul & div inverse: (r1 * r2) / r2 == r1 when r2 != 0
            if r2.numer() != 0 {
                if let Ok(prod) = r1.checked_mul(r2) {
                    let quot = prod.checked_div(r2);
                    assert_eq!(quot, Ok(r1), "(r1 * r2) / r2 must be r1");

                    // Cross-check multiplication against i128 definition:
                    let expected_num = i128::from(r1.numer()).wrapping_mul(i128::from(r2.numer()));
                    let expected_den =
                        i128::from(r1.denom().get()).wrapping_mul(i128::from(r2.denom().get()));

                    let actual_cross = i128::from(prod.numer()).wrapping_mul(expected_den);
                    let expected_cross = expected_num.wrapping_mul(i128::from(prod.denom().get()));
                    assert_eq!(
                        actual_cross, expected_cross,
                        "product must match exact rational definition"
                    );
                }
            }

            success_count = success_count.wrapping_add(1);
        }

        assert!(
            success_count >= 400,
            "must execute at least 400 random rational cases"
        );
    }

    #[test]
    fn test_int_ratio_exact_values() {
        // Test that int_ratio produces exactly n / 1 for any i64.
        assert_eq!(int_ratio(0).numer(), 0);
        assert_eq!(int_ratio(0).denom().get(), 1);

        assert_eq!(int_ratio(1).numer(), 1);
        assert_eq!(int_ratio(1).denom().get(), 1);

        assert_eq!(int_ratio(-1).numer(), -1);
        assert_eq!(int_ratio(-1).denom().get(), 1);

        assert_eq!(int_ratio(42).numer(), 42);
        assert_eq!(int_ratio(42).denom().get(), 1);

        assert_eq!(int_ratio(-42).numer(), -42);
        assert_eq!(int_ratio(-42).denom().get(), 1);

        // Test boundary values.
        assert_eq!(int_ratio(i64::MAX).numer(), i64::MAX);
        assert_eq!(int_ratio(i64::MAX).denom().get(), 1);

        assert_eq!(int_ratio(i64::MIN).numer(), i64::MIN);
        assert_eq!(int_ratio(i64::MIN).denom().get(), 1);
    }

    #[test]
    fn test_half_ratio_exact_value() {
        // Test that half_ratio produces exactly 1 / 2.
        assert_eq!(half_ratio().numer(), 1);
        assert_eq!(half_ratio().denom().get(), 2);
    }

    #[test]
    fn test_quarter_ratio_exact_value() {
        // Test that quarter_ratio produces exactly 1 / 4.
        assert_eq!(quarter_ratio().numer(), 1);
        assert_eq!(quarter_ratio().denom().get(), 4);
    }
}
