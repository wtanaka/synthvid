//! Scene description, motion, geometry, and rendering.
//!
//! This crate performs no input or output. It takes values and returns values.
#![forbid(unsafe_code)]

use core::cmp::Ordering;
use core::fmt;
use core::num::{NonZeroI64, NonZeroU16, NonZeroU32};

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

/// Zero-based index of a frame in a video sequence.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct FrameIndex(pub u32);

impl FrameIndex {
    /// Creates a new frame index.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Returns the underlying frame index value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl From<u32> for FrameIndex {
    fn from(index: u32) -> Self {
        Self(index)
    }
}

impl From<FrameIndex> for u32 {
    fn from(index: FrameIndex) -> Self {
        index.0
    }
}

/// Count of frames in a video sequence, guaranteed to be non-zero.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FrameCount(pub NonZeroU32);

impl FrameCount {
    /// Creates a frame count if `count` is non-zero.
    #[must_use]
    pub const fn new(count: u32) -> Option<Self> {
        match NonZeroU32::new(count) {
            Some(nz) => Some(Self(nz)),
            None => None,
        }
    }

    /// Creates a frame count directly from a [`NonZeroU32`].
    #[must_use]
    pub const fn from_nonzero(count: NonZeroU32) -> Self {
        Self(count)
    }

    /// Returns the underlying [`NonZeroU32`].
    #[must_use]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }
}

impl From<NonZeroU32> for FrameCount {
    fn from(count: NonZeroU32) -> Self {
        Self(count)
    }
}

impl From<FrameCount> for NonZeroU32 {
    fn from(count: FrameCount) -> Self {
        count.0
    }
}

/// Horizontal dimension in pixels, guaranteed to be non-zero.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Width(pub NonZeroU16);

impl Width {
    /// Creates a new width if `width` is non-zero.
    #[must_use]
    pub const fn new(width: u16) -> Option<Self> {
        match NonZeroU16::new(width) {
            Some(nz) => Some(Self(nz)),
            None => None,
        }
    }

    /// Creates a width directly from a [`NonZeroU16`].
    #[must_use]
    pub const fn from_nonzero(width: NonZeroU16) -> Self {
        Self(width)
    }

    /// Returns the underlying [`NonZeroU16`].
    #[must_use]
    pub const fn get(self) -> NonZeroU16 {
        self.0
    }
}

impl From<NonZeroU16> for Width {
    fn from(width: NonZeroU16) -> Self {
        Self(width)
    }
}

impl From<Width> for NonZeroU16 {
    fn from(width: Width) -> Self {
        width.0
    }
}

/// Vertical dimension in pixels, guaranteed to be non-zero.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Height(pub NonZeroU16);

impl Height {
    /// Creates a new height if `height` is non-zero.
    #[must_use]
    pub const fn new(height: u16) -> Option<Self> {
        match NonZeroU16::new(height) {
            Some(nz) => Some(Self(nz)),
            None => None,
        }
    }

    /// Creates a height directly from a [`NonZeroU16`].
    #[must_use]
    pub const fn from_nonzero(height: NonZeroU16) -> Self {
        Self(height)
    }

    /// Returns the underlying [`NonZeroU16`].
    #[must_use]
    pub const fn get(self) -> NonZeroU16 {
        self.0
    }
}

impl From<NonZeroU16> for Height {
    fn from(height: NonZeroU16) -> Self {
        Self(height)
    }
}

impl From<Height> for NonZeroU16 {
    fn from(height: Height) -> Self {
        height.0
    }
}

/// 2D frame dimensions consisting of width and height.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Dimensions {
    /// Horizontal dimension in pixels.
    pub width: Width,
    /// Vertical dimension in pixels.
    pub height: Height,
}

impl Dimensions {
    /// Creates new dimensions from width and height.
    #[must_use]
    pub const fn new(width: Width, height: Height) -> Self {
        Self { width, height }
    }
}

/// Exact frame rate in frames per second, represented as a rational number.
///
/// Rejects zero and negative rates. Frame rates must be strictly positive.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FrameRate(Ratio);

impl FrameRate {
    /// Creates a frame rate from a [`Ratio`], rejecting zero and negative values.
    #[must_use]
    pub const fn new(rate: Ratio) -> Option<Self> {
        if rate.numer() > 0 {
            Some(Self(rate))
        } else {
            None
        }
    }

    /// Creates a frame rate from a [`Ratio`], rejecting zero and negative values.
    #[must_use]
    pub const fn from_ratio(rate: Ratio) -> Option<Self> {
        Self::new(rate)
    }

    /// Creates an exact frame rate from an integer frames per second value.
    ///
    /// Rejects zero.
    #[must_use]
    pub fn from_fps(fps: u32) -> Option<Self> {
        let nz_fps = NonZeroU32::new(fps)?;
        let numer = i64::from(nz_fps.get());
        let denom = NonZeroI64::new(1)?;
        let ratio = Ratio::new(numer, denom)?;
        Self::new(ratio)
    }

    /// Creates an exact frame rate from numerator and non-zero denominator (e.g. 30000 / 1001).
    ///
    /// Rejects zero numerator.
    #[must_use]
    pub fn from_fraction(numer: u32, denom: NonZeroU32) -> Option<Self> {
        let n = i64::from(numer);
        let d = i64::from(denom.get());
        let nz_d = NonZeroI64::new(d)?;
        let ratio = Ratio::new(n, nz_d)?;
        Self::new(ratio)
    }

    /// Returns the underlying [`Ratio`].
    #[must_use]
    pub const fn get(self) -> Ratio {
        self.0
    }

    /// Returns the underlying [`Ratio`].
    #[must_use]
    pub const fn ratio(self) -> Ratio {
        self.0
    }
}

/// Error returned when attempting to construct a non-positive [`FrameRate`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FrameRateError {
    /// Frame rate must be strictly positive.
    NonPositive,
}

impl fmt::Display for FrameRateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositive => write!(f, "frame rate must be strictly positive"),
        }
    }
}

impl core::error::Error for FrameRateError {}

impl TryFrom<Ratio> for FrameRate {
    type Error = FrameRateError;

    fn try_from(rate: Ratio) -> Result<Self, Self::Error> {
        Self::new(rate).ok_or(FrameRateError::NonPositive)
    }
}

/// Seed for deterministic pseudo-random number generation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct Seed(pub u64);

impl Seed {
    /// Creates a new [`Seed`].
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Returns the underlying `u64` seed value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for Seed {
    fn from(seed: u64) -> Self {
        Self(seed)
    }
}

impl From<Seed> for u64 {
    fn from(seed: Seed) -> Self {
        seed.0
    }
}

/// A small, fully specified 64-bit counter-based pseudo-random number generator.
///
/// This generator implements the `SplitMix64` algorithm (Steele et al., 2014).
/// The algorithm is specified below so its outputs can never drift between versions
/// or across target architectures:
///
/// 1. State: a single 64-bit unsigned integer initialized to `seed.get()`.
/// 2. Transition:
///    `state = state.wrapping_add(0x9e37_79b9_7f4a_7c15)`
/// 3. Output mixing:
///    - `z = state`
///    - `z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9)`
///    - `z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb)`
///    - result = `z ^ (z >> 31)`
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Rng {
    /// The current 64-bit internal generator state.
    state: u64,
}

impl Rng {
    /// Creates a deterministic pseudo-random number generator from a [`Seed`].
    #[must_use]
    pub const fn from_seed(seed: Seed) -> Self {
        Self { state: seed.get() }
    }

    /// Generates the next pseudo-random 64-bit unsigned integer.
    pub const fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Generates a pseudo-random integer uniformly distributed in `[0, bound)`.
    ///
    /// Uses rejection sampling with a power-of-two bitmask to eliminate modulo bias.
    /// Returns 0 if `bound <= 1`.
    pub fn next_bounded(&mut self, bound: u64) -> u64 {
        if bound <= 1 {
            return 0;
        }
        let bits = 64_u32
            .checked_sub((bound.wrapping_sub(1)).leading_zeros())
            .unwrap_or(64);
        let mask = if bits >= 64 {
            u64::MAX
        } else {
            (1_u64.checked_shl(bits).unwrap_or(0)).wrapping_sub(1)
        };
        loop {
            let candidate = self.next_u64() & mask;
            if candidate < bound {
                return candidate;
            }
        }
    }

    /// Generates a pseudo-random [`Ratio`] uniformly distributed in the unit interval `[0, 1)`.
    ///
    /// The rational is formed by sampling a 62-bit numerator and placing it over a
    /// denominator of 2^62, then reducing to lowest terms via [`Ratio::new`].
    #[must_use]
    pub fn next_ratio_unit(&mut self) -> Ratio {
        let raw = self.next_u64();
        // Shift right by 2 to obtain 62 bits, fitting strictly within positive i64 bounds.
        let numer = i64::try_from(raw >> 2).unwrap_or_default();
        // 2^62 = 0x4000_0000_0000_0000 fits in positive i64.
        let denom = NonZeroI64::new(0x4000_0000_0000_0000).unwrap_or(NonZeroI64::MIN);
        let fallback_denom = NonZeroI64::new(1).unwrap_or(NonZeroI64::MIN);
        Ratio::new(numer, denom).unwrap_or(Ratio {
            numer: 0,
            denom: fallback_denom,
        })
    }
}

impl From<Seed> for Rng {
    fn from(seed: Seed) -> Self {
        Self::from_seed(seed)
    }
}

/// 24-bit RGB pixel color with 8 bits per channel.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct Rgb8 {
    /// Red channel value.
    pub r: u8,
    /// Green channel value.
    pub g: u8,
    /// Blue channel value.
    pub b: u8,
}

impl Rgb8 {
    /// Creates a new [`Rgb8`] color from red, green, and blue components.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            let rem = rand.checked_rem(u_span.wrapping_add(1)).unwrap_or_default();
            let rem_i64 = i64::try_from(rem).unwrap_or_default();
            min.wrapping_add(rem_i64)
        }
    }

    #[test]
    fn test_ratio_normalisation_equal_values() {
        let nz1 = NonZeroI64::new(1);
        let nz2 = NonZeroI64::new(2);
        let nz_neg2 = NonZeroI64::new(-2);
        let nz4 = NonZeroI64::new(4);
        let nz_neg4 = NonZeroI64::new(-4);
        let nz6 = NonZeroI64::new(6);
        let nz200 = NonZeroI64::new(200);

        assert!(nz1.is_some(), "non-zero 1 must be some");
        assert!(nz2.is_some(), "non-zero 2 must be some");
        assert!(nz_neg2.is_some(), "non-zero -2 must be some");
        assert!(nz4.is_some(), "non-zero 4 must be some");
        assert!(nz_neg4.is_some(), "non-zero -4 must be some");
        assert!(nz6.is_some(), "non-zero 6 must be some");
        assert!(nz200.is_some(), "non-zero 200 must be some");

        let Some(d1) = nz1 else { return };
        let Some(d2) = nz2 else { return };
        let Some(d_neg2) = nz_neg2 else { return };
        let Some(d4) = nz4 else { return };
        let Some(d_neg4) = nz_neg4 else { return };
        let Some(d6) = nz6 else { return };
        let Some(d200) = nz200 else { return };

        let r_half1 = Ratio::new(1, d2);
        let r_half2 = Ratio::new(2, d4);
        let r_half3 = Ratio::new(3, d6);
        let r_half4 = Ratio::new(100, d200);
        let r_half_neg = Ratio::new(-2, d_neg4);

        assert_eq!(r_half1, r_half2, "1/2 must equal 2/4");
        assert_eq!(r_half1, r_half3, "1/2 must equal 3/6");
        assert_eq!(r_half1, r_half4, "1/2 must equal 100/200");
        assert_eq!(r_half1, r_half_neg, "1/2 must equal -2/-4");

        let Some(r) = r_half1 else { return };
        assert_eq!(r.numer(), 1, "numerator of 1/2 must be 1");
        assert_eq!(r.denom().get(), 2, "denominator of 1/2 must be 2");

        // Negative values sign carried in numerator.
        let r_neg_half1 = Ratio::new(-1, d2);
        let r_neg_half2 = Ratio::new(1, d_neg2);
        let r_neg_half3 = Ratio::new(-2, d4);
        assert_eq!(r_neg_half1, r_neg_half2, "-1/2 must equal 1/-2");
        assert_eq!(r_neg_half1, r_neg_half3, "-1/2 must equal -2/4");

        let Some(rn) = r_neg_half1 else { return };
        assert_eq!(rn.numer(), -1, "numerator must carry negative sign");
        assert_eq!(rn.denom().get(), 2, "denominator must remain positive");

        // Zero representation is uniquely 0/1.
        let r_zero1 = Ratio::new(0, d1);
        let r_zero2 = Ratio::new(0, d4);
        let r_zero3 = Ratio::new(0, d_neg4);
        assert_eq!(r_zero1, r_zero2, "0/1 must equal 0/4");
        assert_eq!(r_zero1, r_zero3, "0/1 must equal 0/-4");

        let Some(rz) = r_zero1 else { return };
        assert_eq!(rz.numer(), 0, "zero numerator must be 0");
        assert_eq!(rz.denom().get(), 1, "zero denominator must be 1");
    }

    #[test]
    fn test_ratio_comparison() {
        let Some(d1) = NonZeroI64::new(1) else { return };
        let Some(d2) = NonZeroI64::new(2) else { return };
        let Some(d3) = NonZeroI64::new(3) else { return };

        let Some(r_half) = Ratio::new(1, d2) else {
            return;
        };
        let Some(r_third) = Ratio::new(1, d3) else {
            return;
        };
        let Some(r_two_thirds) = Ratio::new(2, d3) else {
            return;
        };
        let Some(r_neg_one) = Ratio::new(-1, d1) else {
            return;
        };

        assert!(r_third < r_half, "1/3 < 1/2");
        assert!(r_half < r_two_thirds, "1/2 < 2/3");
        assert!(r_neg_one < r_third, "-1 < 1/3");
        assert_eq!(r_half.cmp(&r_half), Ordering::Equal, "1/2 == 1/2");
    }

    #[test]
    fn test_ratio_to_f64() {
        let Some(d1) = NonZeroI64::new(1) else { return };
        let Some(d2) = NonZeroI64::new(2) else { return };
        let Some(d4) = NonZeroI64::new(4) else { return };

        let Some(r1) = Ratio::new(1, d2) else { return };
        let Some(r2) = Ratio::new(3, d4) else { return };
        let Some(r3) = Ratio::new(-1, d4) else { return };
        let Some(r4) = Ratio::new(0, d1) else { return };

        assert!((r1.to_f64() - 0.5).abs() < 1e-12, "1/2 must be 0.5");
        assert!((r2.to_f64() - 0.75).abs() < 1e-12, "3/4 must be 0.75");
        assert!((r3.to_f64() - (-0.25)).abs() < 1e-12, "-1/4 must be -0.25");
        assert!((r4.to_f64() - 0.0).abs() < 1e-12, "0/1 must be 0.0");
    }

    #[test]
    fn test_ratio_overflow_returns_none() {
        let Some(d1) = NonZeroI64::new(1) else { return };
        let Some(max_ratio) = Ratio::new(i64::MAX, d1) else {
            return;
        };
        let Some(one_ratio) = Ratio::new(1, d1) else {
            return;
        };
        let Some(min_ratio) = Ratio::new(i64::MIN, d1) else {
            return;
        };
        let Some(zero_ratio) = Ratio::new(0, d1) else {
            return;
        };

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
        let Some(two_ratio) = Ratio::new(2, d1) else {
            return;
        };
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

    #[test]
    fn test_newtypes_and_framerate() {
        // FrameIndex
        let fi0 = FrameIndex::new(0);
        assert_eq!(fi0.get(), 0, "frame index 0");
        let fi42 = FrameIndex::from(42);
        assert_eq!(u32::from(fi42), 42, "frame index from / into");

        // FrameCount
        assert!(FrameCount::new(0).is_none(), "frame count 0 rejected");
        let fc = FrameCount::new(100);
        assert!(fc.is_some(), "frame count 100 accepted");
        let Some(fc_val) = fc else { return };
        assert_eq!(fc_val.get().get(), 100, "frame count get");

        // Width & Height & Dimensions
        assert!(Width::new(0).is_none(), "width 0 rejected");
        assert!(Height::new(0).is_none(), "height 0 rejected");
        let Some(w) = Width::new(1920) else { return };
        let Some(h) = Height::new(1080) else { return };
        let dims = Dimensions::new(w, h);
        assert_eq!(dims.width.get().get(), 1920, "dims width");
        assert_eq!(dims.height.get().get(), 1080, "dims height");

        // Seed
        let seed = Seed::new(12345);
        assert_eq!(seed.get(), 12345, "seed value");

        // Rgb8
        let col = Rgb8::new(255, 128, 64);
        assert_eq!(col.r, 255, "red channel");
        assert_eq!(col.g, 128, "green channel");
        assert_eq!(col.b, 64, "blue channel");

        // FrameRate
        let Some(d1) = NonZeroI64::new(1) else { return };
        let Some(zero_ratio) = Ratio::new(0, d1) else {
            return;
        };
        let Some(neg_ratio) = Ratio::new(-24, d1) else {
            return;
        };

        assert!(FrameRate::new(zero_ratio).is_none(), "rate 0 rejected");
        assert!(
            FrameRate::new(neg_ratio).is_none(),
            "negative rate rejected"
        );
        assert!(FrameRate::from_fps(0).is_none(), "fps 0 rejected");

        // Integer rate 24 fps
        let fps24 = FrameRate::from_fps(24);
        assert!(fps24.is_some(), "fps 24 accepted");
        let Some(r24) = fps24 else { return };
        assert_eq!(r24.get().numer(), 24, "fps 24 numerator");
        assert_eq!(r24.get().denom().get(), 1, "fps 24 denominator");

        // Fractional rate 30000/1001 (NTSC)
        let Some(nz1001) = NonZeroU32::new(1001) else {
            return;
        };
        let ntsc = FrameRate::from_fraction(30000, nz1001);
        assert!(ntsc.is_some(), "30000/1001 accepted");
        let Some(r_ntsc) = ntsc else { return };
        assert_eq!(r_ntsc.ratio().numer(), 30000, "NTSC numerator");
        assert_eq!(r_ntsc.ratio().denom().get(), 1001, "NTSC denominator");

        // Check TryFrom
        let Some(d2) = NonZeroI64::new(2) else { return };
        let Some(pos_ratio) = Ratio::new(60, d2) else {
            return;
        };
        let try_res = FrameRate::try_from(pos_ratio);
        assert!(try_res.is_ok(), "TryFrom positive ratio succeeds");
        let err_res = FrameRate::try_from(neg_ratio);
        assert_eq!(
            err_res,
            Err(FrameRateError::NonPositive),
            "TryFrom negative ratio fails"
        );
    }

    #[test]
    fn test_rng_frozen_vectors() {
        let expected_seed_0: [u64; 16] = [
            0xe220_a839_7b1d_cdaf,
            0x6e78_9e6a_a1b9_65f4,
            0x06c4_5d18_8009_454f,
            0xf88b_b8a8_724c_81ec,
            0x1b39_896a_51a8_749b,
            0x53cb_9f0c_747e_a2ea,
            0x2c82_9abe_1f45_32e1,
            0xc584_133a_c916_ab3c,
            0x3ee5_7890_41c9_8ac3,
            0xf3b8_488c_368c_b0a6,
            0x657e_ecdd_3cb1_3d09,
            0xc2d3_26e0_055b_def6,
            0x8621_a03f_e0bb_db7b,
            0x8e1f_7555_983a_a92f,
            0xb54e_0f16_00cc_4d19,
            0x84bb_3f97_971d_80ab,
        ];

        let expected_seed_1: [u64; 16] = [
            0x910a_2dec_8902_5cc1,
            0xbeeb_8da1_658e_ec67,
            0xf893_a2ee_fb32_555e,
            0x71c1_8690_ee42_c90b,
            0x71bb_54d8_d101_b5b9,
            0xc34d_0bff_9015_0280,
            0xe099_ec6c_d736_3ca5,
            0x85e7_bb0f_1227_8575,
            0x4917_18de_357e_3da8,
            0xcb43_5c8e_7461_6796,
            0x6775_dc77_0156_4f61,
            0x9afc_d44d_14cf_8bfe,
            0x7476_cf8a_4baa_5dc0,
            0x87b3_41d6_90d7_a28a,
            0x6f9b_6dae_6f4c_57a8,
            0x2ac2_ce17_a579_4a3b,
        ];

        let expected_seed_deadbeef: [u64; 16] = [
            0x4adf_b90f_68c9_eb9b,
            0xde58_6a31_41a1_0922,
            0x021f_bc2f_8e1c_fc1d,
            0x7466_ce73_7be1_6790,
            0x3bfa_8764_f685_bd1c,
            0xab20_3e50_3cb5_5b3f,
            0x5a2f_dc2b_f68c_edb3,
            0xb30a_4ccf_430b_1b5a,
            0x0a90_4150_39bd_5985,
            0x26ae_5084_7745_eb7e,
            0xe239_ed30_6d9b_1929,
            0xfb7d_9a8d_444d_41bc,
            0x1bb5_2e52_3960_d559,
            0xcf86_31b4_0292_b5d5,
            0xf618_6c41_b838_b122,
            0x4324_97ff_b78c_1173,
        ];

        let mut rng0 = Rng::from_seed(Seed::new(0));
        for &expected in &expected_seed_0 {
            assert_eq!(rng0.next_u64(), expected, "seed 0 frozen vector mismatch");
        }

        let mut rng1 = Rng::from_seed(Seed::new(1));
        for &expected in &expected_seed_1 {
            assert_eq!(rng1.next_u64(), expected, "seed 1 frozen vector mismatch");
        }

        let mut rng_db = Rng::from_seed(Seed::new(0xdead_beef));
        for &expected in &expected_seed_deadbeef {
            assert_eq!(
                rng_db.next_u64(),
                expected,
                "seed 0xDEADBEEF frozen vector mismatch"
            );
        }
    }

    #[test]
    fn test_rng_bounded() {
        let mut rng = Rng::from_seed(Seed::new(42));
        assert_eq!(rng.next_bounded(0), 0, "bound 0 must return 0");
        assert_eq!(rng.next_bounded(1), 0, "bound 1 must return 0");

        for _ in 0..1000 {
            let val = rng.next_bounded(7);
            assert!(val < 7, "bounded value must be strictly less than bound 7");
        }

        for _ in 0..1000 {
            let val = rng.next_bounded(100);
            assert!(
                val < 100,
                "bounded value must be strictly less than bound 100"
            );
        }

        for _ in 0..1000 {
            let val = rng.next_bounded(1024);
            assert!(
                val < 1024,
                "bounded value must be strictly less than bound 1024"
            );
        }

        // Test determinism
        let mut rng_a = Rng::from_seed(Seed::new(999));
        let mut rng_b = Rng::from_seed(Seed::new(999));
        for _ in 0..100 {
            assert_eq!(
                rng_a.next_bounded(50),
                rng_b.next_bounded(50),
                "identical seeds must yield identical bounded sequence"
            );
        }
    }

    #[test]
    fn test_rng_ratio_unit() {
        let mut rng = Rng::from_seed(Seed::new(123));
        let Some(one) = Ratio::from_integer(1) else {
            return;
        };
        let Some(zero) = Ratio::from_integer(0) else {
            return;
        };

        for _ in 0..1000 {
            let r = rng.next_ratio_unit();
            assert!(r >= zero, "unit ratio must be non-negative");
            assert!(r < one, "unit ratio must be strictly less than 1");
            assert!(r.numer() >= 0, "unit ratio numerator must be non-negative");
            assert!(
                r.denom().get() > 0,
                "unit ratio denominator must be strictly positive"
            );
        }

        // Test determinism
        let mut rng_a = Rng::from_seed(Seed::new(777));
        let mut rng_b = Rng::from_seed(Seed::new(777));
        for _ in 0..100 {
            assert_eq!(
                rng_a.next_ratio_unit(),
                rng_b.next_ratio_unit(),
                "identical seeds must yield identical ratio unit sequence"
            );
        }
    }
}
