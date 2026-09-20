//! Exact rational arithmetic.
//!
//! [`Ratio`] is the numeric type every other module computes in: a normalised
//! `i64 / NonZeroI64` fraction whose operations are checked and exact, so a
//! value never drifts between platforms or releases.

use core::cmp::Ordering;
use core::fmt;
use core::num::NonZeroI64;

use crate::trig::{cos_turns, sin_turns};

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
/// Returns `None` if `abs_den` is 0 or if the resulting reduced rational cannot be
/// represented with an `i64` numerator and positive [`NonZeroI64`] denominator.
fn from_u128_parts(abs_num: u128, abs_den: u128, is_negative: bool) -> Option<Ratio> {
    if abs_den == 0 {
        return None;
    }
    if abs_num == 0 {
        let one = NonZeroI64::new(1)?;
        return Some(Ratio {
            numer: 0,
            denom: one,
        });
    }

    let g = gcd_u128(abs_num, abs_den);
    if g == 0 {
        return None;
    }
    let red_num = abs_num.checked_div(g)?;
    let red_den = abs_den.checked_div(g)?;

    // Denominator must be strictly positive and fit in positive i64.
    let den_i64 = i64::try_from(red_den).ok()?;
    if den_i64 <= 0 {
        return None;
    }
    let non_zero_den = NonZeroI64::new(den_i64)?;

    // Magnitude of i64::MIN is 2^63.
    let min_i64_mag = u128::from(i64::MIN.unsigned_abs());

    let numer_i64 = if is_negative {
        match red_num.cmp(&min_i64_mag) {
            Ordering::Equal => i64::MIN,
            Ordering::Less => {
                let pos = i64::try_from(red_num).ok()?;
                0_i64.checked_sub(pos)?
            }
            Ordering::Greater => return None,
        }
    } else {
        i64::try_from(red_num).ok()?
    };

    Some(Ratio {
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
    /// Returns `None` if the reduced rational cannot fit within `i64` bounds.
    #[must_use]
    pub fn new(numer: i64, denom: NonZeroI64) -> Option<Self> {
        let num128 = i128::from(numer);
        let den128 = i128::from(denom.get());
        let is_negative = (num128 < 0) ^ (den128 < 0);
        from_u128_parts(num128.unsigned_abs(), den128.unsigned_abs(), is_negative)
    }

    /// Creates a normalised rational representing an integer `n / 1`.
    #[must_use]
    pub fn from_integer(n: i64) -> Option<Self> {
        let denom = NonZeroI64::new(1)?;
        Self::new(n, denom)
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

    /// Checked addition. Returns `None` if intermediate or final values overflow `i64`.
    #[must_use]
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        let n1 = i128::from(self.numer);
        let d1 = i128::from(self.denom.get());
        let n2 = i128::from(rhs.numer);
        let d2 = i128::from(rhs.denom.get());

        let term1 = n1.checked_mul(d2)?;
        let term2 = n2.checked_mul(d1)?;
        let num128 = term1.checked_add(term2)?;
        let den128 = d1.checked_mul(d2)?;

        let is_negative = num128 < 0;
        from_u128_parts(num128.unsigned_abs(), den128.unsigned_abs(), is_negative)
    }

    /// Checked subtraction. Returns `None` if intermediate or final values overflow `i64`.
    #[must_use]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        let n1 = i128::from(self.numer);
        let d1 = i128::from(self.denom.get());
        let n2 = i128::from(rhs.numer);
        let d2 = i128::from(rhs.denom.get());

        let term1 = n1.checked_mul(d2)?;
        let term2 = n2.checked_mul(d1)?;
        let num128 = term1.checked_sub(term2)?;
        let den128 = d1.checked_mul(d2)?;

        let is_negative = num128 < 0;
        from_u128_parts(num128.unsigned_abs(), den128.unsigned_abs(), is_negative)
    }

    /// Checked multiplication. Returns `None` if intermediate or final values overflow `i64`.
    #[must_use]
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        let n1 = i128::from(self.numer);
        let d1 = i128::from(self.denom.get());
        let n2 = i128::from(rhs.numer);
        let d2 = i128::from(rhs.denom.get());

        let num128 = n1.checked_mul(n2)?;
        let den128 = d1.checked_mul(d2)?;

        let is_negative = (num128 < 0) ^ (den128 < 0);
        from_u128_parts(num128.unsigned_abs(), den128.unsigned_abs(), is_negative)
    }

    /// Checked division. Returns `None` on division by zero or if overflow occurs.
    #[must_use]
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.numer == 0 {
            return None;
        }
        let n1 = i128::from(self.numer);
        let d1 = i128::from(self.denom.get());
        let n2 = i128::from(rhs.numer);
        let d2 = i128::from(rhs.denom.get());

        let num128 = n1.checked_mul(d2)?;
        let den128 = d1.checked_mul(n2)?;

        let is_negative = (num128 < 0) ^ (den128 < 0);
        from_u128_parts(num128.unsigned_abs(), den128.unsigned_abs(), is_negative)
    }

    /// Checked negation. Returns `None` if negating the numerator overflows `i64::MIN`.
    #[must_use]
    pub fn checked_neg(self) -> Option<Self> {
        let neg_num = 0_i64.checked_sub(self.numer)?;
        Some(Self {
            numer: neg_num,
            denom: self.denom,
        })
    }

    /// Converts this ratio to an `f64` representation for rendering.
    ///
    /// This conversion is lossy for large numbers and is intended exclusively
    /// for the render path.
    #[must_use]
    #[expect(
        clippy::as_conversions,
        clippy::cast_precision_loss,
        reason = "conversion to f64 for the render path only"
    )]
    pub fn to_f64(self) -> f64 {
        (self.numer as f64) / (self.denom.get() as f64)
    }

    /// Computes the sine of this angle given in turns.
    ///
    /// See [`sin_turns`] for precision, error bounds, and algorithm details.
    #[must_use]
    pub fn sin_turns(self) -> Self {
        sin_turns(self)
    }

    /// Computes the cosine of this angle given in turns.
    ///
    /// See [`cos_turns`] for precision, error bounds, and algorithm details.
    #[must_use]
    pub fn cos_turns(self) -> Self {
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

/// Returns the exact ratio `value / 1`.
///
/// The construction cannot fail for any `i64`; the fallback is unreachable
/// and exists only because [`Ratio::from_integer`] returns [`Option`].
pub(crate) fn int_ratio(value: i64) -> Ratio {
    let fallback_denom = NonZeroI64::new(1).unwrap_or(NonZeroI64::MIN);
    let fallback = Ratio {
        numer: 0,
        denom: fallback_denom,
    };
    Ratio::from_integer(value).unwrap_or(fallback)
}

/// Returns the exact ratio `1 / 2`.
///
/// The fallback is unreachable; see [`int_ratio`].
pub(crate) fn half_ratio() -> Ratio {
    let one = int_ratio(1);
    let two = int_ratio(2);
    let fallback = int_ratio(0);
    one.checked_div(two).unwrap_or(fallback)
}

/// Returns the exact ratio `1 / 4`, the squared pixel-centre threshold.
///
/// The fallback is unreachable; see [`int_ratio`].
pub(crate) fn quarter_ratio() -> Ratio {
    let one = int_ratio(1);
    let four = int_ratio(4);
    let fallback = int_ratio(0);
    one.checked_div(four).unwrap_or(fallback)
}

/// Returns the greatest integer less than or equal to `value`.
///
/// Uses Euclidean division; the denominator is strictly positive so the
/// result always fits in `i64` and this only returns `None` in theory.
pub(crate) const fn floor_ratio(value: Ratio) -> Option<i64> {
    value.numer().checked_div_euclid(value.denom().get())
}

/// Returns the smallest integer greater than or equal to `value`.
///
/// Computed as the Euclidean floor plus one when there is a remainder, so
/// no negation of `i64::MIN` is ever required.
pub(crate) fn ceil_ratio(value: Ratio) -> Option<i64> {
    let quot = value.numer().checked_div_euclid(value.denom().get())?;
    let remnant = value.numer().checked_rem_euclid(value.denom().get())?;
    if remnant == 0 {
        Some(quot)
    } else {
        quot.checked_add(1)
    }
}

#[cfg(test)]
mod tests {
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

        let r_half1 = Ratio::new(1, d2).unwrap();
        let r_half2 = Ratio::new(2, d4).unwrap();
        let r_half3 = Ratio::new(3, d6).unwrap();
        let r_half4 = Ratio::new(100, d200).unwrap();
        let r_half_neg = Ratio::new(-2, d_neg4).unwrap();

        assert_eq!(r_half1, r_half2, "1/2 must equal 2/4");
        assert_eq!(r_half1, r_half3, "1/2 must equal 3/6");
        assert_eq!(r_half1, r_half4, "1/2 must equal 100/200");
        assert_eq!(r_half1, r_half_neg, "1/2 must equal -2/-4");

        assert_eq!(r_half1.numer(), 1, "numerator of 1/2 must be 1");
        assert_eq!(r_half1.denom().get(), 2, "denominator of 1/2 must be 2");

        // Negative values sign carried in numerator.
        let r_neg_half1 = Ratio::new(-1, d2).unwrap();
        let r_neg_half2 = Ratio::new(1, d_neg2).unwrap();
        let r_neg_half3 = Ratio::new(-2, d4).unwrap();
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
        let r_zero1 = Ratio::new(0, d1).unwrap();
        let r_zero2 = Ratio::new(0, d4).unwrap();
        let r_zero3 = Ratio::new(0, d_neg4).unwrap();
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

        let r_half = Ratio::new(1, d2).unwrap();
        let r_third = Ratio::new(1, d3).unwrap();
        let r_two_thirds = Ratio::new(2, d3).unwrap();
        let r_neg_one = Ratio::new(-1, d1).unwrap();

        assert!(r_third < r_half, "1/3 < 1/2");
        assert!(r_half < r_two_thirds, "1/2 < 2/3");
        assert!(r_neg_one < r_third, "-1 < 1/3");
        assert_eq!(r_half.cmp(&r_half), Ordering::Equal, "1/2 == 1/2");
    }

    #[test]
    fn test_ratio_to_f64() {
        let d1 = NonZeroI64::new(1).unwrap();
        let d2 = NonZeroI64::new(2).unwrap();
        let d4 = NonZeroI64::new(4).unwrap();

        let r1 = Ratio::new(1, d2).unwrap();
        let r2 = Ratio::new(3, d4).unwrap();
        let r3 = Ratio::new(-1, d4).unwrap();
        let r4 = Ratio::new(0, d1).unwrap();

        assert!((r1.to_f64() - 0.5).abs() < 1e-12, "1/2 must be 0.5");
        assert!((r2.to_f64() - 0.75).abs() < 1e-12, "3/4 must be 0.75");
        assert!((r3.to_f64() - (-0.25)).abs() < 1e-12, "-1/4 must be -0.25");
        assert!((r4.to_f64() - 0.0).abs() < 1e-12, "0/1 must be 0.0");
    }

    #[test]
    fn test_ratio_overflow_returns_none() {
        let d1 = NonZeroI64::new(1).unwrap();
        let max_ratio = Ratio::new(i64::MAX, d1).unwrap();
        let one_ratio = Ratio::new(1, d1).unwrap();
        let min_ratio = Ratio::new(i64::MIN, d1).unwrap();
        let zero_ratio = Ratio::new(0, d1).unwrap();

        // Addition overflow
        assert!(
            max_ratio.checked_add(one_ratio).is_none(),
            "i64::MAX + 1 must return None"
        );

        // Subtraction overflow
        assert!(
            min_ratio.checked_sub(one_ratio).is_none(),
            "i64::MIN - 1 must return None"
        );

        // Multiplication overflow
        let two_ratio = Ratio::new(2, d1).unwrap();
        assert!(
            max_ratio.checked_mul(two_ratio).is_none(),
            "i64::MAX * 2 must return None"
        );

        // Division by zero returns None
        assert!(
            one_ratio.checked_div(zero_ratio).is_none(),
            "division by zero must return None"
        );

        // Negation of i64::MIN returns None
        assert!(
            min_ratio.checked_neg().is_none(),
            "-i64::MIN must return None"
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

            let Some(r1) = Ratio::new(num1, nz_den1) else {
                continue;
            };
            let Some(r2) = Ratio::new(num2, nz_den2) else {
                continue;
            };

            // Test add & sub inverse: (r1 + r2) - r2 == r1
            if let Some(sum) = r1.checked_add(r2) {
                let diff = sum.checked_sub(r2);
                assert_eq!(diff, Some(r1), "(r1 + r2) - r2 must be r1");

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
                if let Some(prod) = r1.checked_mul(r2) {
                    let quot = prod.checked_div(r2);
                    assert_eq!(quot, Some(r1), "(r1 * r2) / r2 must be r1");

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
}
