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

/// Computes the exact byte length required for a 24-bit RGB frame with the given dimensions.
///
/// Returns `None` if `width * height * 3` overflows `usize`.
#[must_use]
pub fn required_buffer_len(dimensions: Dimensions) -> Option<usize> {
    // Convert u16 dimensions to usize for buffer calculation.
    let w = usize::from(dimensions.width.get().get());
    let h = usize::from(dimensions.height.get().get());
    // checked_mul prevents overflow on 32-bit platforms.
    let pixels = w.checked_mul(h)?;
    pixels.checked_mul(3)
}

/// Blends a single 8-bit color channel using integer arithmetic.
///
/// Uses the formula `(src * alpha + dst * (255 - alpha) + 127) / 255`.
fn blend_channel(src: u8, dst: u8, alpha: u8) -> u8 {
    let src_u32 = u32::from(src);
    let dst_u32 = u32::from(dst);
    let alpha_u32 = u32::from(alpha);
    // saturating_sub prevents underflow since alpha <= 255.
    let inv_alpha = 255_u32.saturating_sub(alpha_u32);
    // checked_mul prevents overflow when multiplying channel and alpha.
    let term1 = src_u32.checked_mul(alpha_u32).unwrap_or_default();
    let term2 = dst_u32.checked_mul(inv_alpha).unwrap_or_default();
    // checked_add combines the two weighted terms safely.
    let sum = term1.checked_add(term2).unwrap_or_default();
    // checked_add adds 127 for symmetric rounding to the nearest integer.
    let sum_rounded = sum.checked_add(127).unwrap_or(sum);
    // checked_div divides by 255; 255 is non-zero so this never fails.
    let blended = sum_rounded.checked_div(255).unwrap_or_default();
    u8::try_from(blended).unwrap_or_default()
}

/// A video frame owning a 24-bit RGB pixel buffer of exactly `width * height * 3` bytes.
///
/// Constructed only through constructors that strictly guarantee the length invariant.
/// Accessors are bounds-checked and return [`Option`]. No indexing operators can panic.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Frame {
    /// The 2D dimensions of the frame.
    dimensions: Dimensions,
    /// The raw 24-bit RGB pixel buffer (3 bytes per pixel: red, green, blue).
    data: Vec<u8>,
}

impl Frame {
    /// Creates a new [`Frame`] from [`Dimensions`] and an owned pixel buffer.
    ///
    /// Returns `None` if `data.len()` does not exactly match `width * height * 3` bytes,
    /// or if the required buffer length calculation overflows `usize`.
    #[must_use]
    pub fn new(dimensions: Dimensions, data: Vec<u8>) -> Option<Self> {
        let expected_len = required_buffer_len(dimensions)?;
        if data.len() != expected_len {
            return None;
        }
        Some(Self { dimensions, data })
    }

    /// Creates a new [`Frame`] from [`Dimensions`] and an owned vector of bytes.
    ///
    /// Alias for [`Self::new`].
    #[must_use]
    pub fn from_vec(dimensions: Dimensions, data: Vec<u8>) -> Option<Self> {
        Self::new(dimensions, data)
    }

    /// Creates a new [`Frame`] from [`Dimensions`] and an owned buffer of bytes.
    ///
    /// Alias for [`Self::new`].
    #[must_use]
    pub fn from_buffer(dimensions: Dimensions, data: Vec<u8>) -> Option<Self> {
        Self::new(dimensions, data)
    }

    /// Creates a zero-initialized (black) [`Frame`] with the given dimensions.
    ///
    /// Returns `None` if the required buffer length calculation overflows `usize`.
    #[must_use]
    pub fn zeroed(dimensions: Dimensions) -> Option<Self> {
        let len = required_buffer_len(dimensions)?;
        let data = vec![0_u8; len];
        Some(Self { dimensions, data })
    }

    /// Creates a new [`Frame`] filled uniformly with the given [`Rgb8`] color.
    ///
    /// Returns `None` if the required buffer length calculation overflows `usize`.
    #[must_use]
    pub fn from_color(dimensions: Dimensions, color: Rgb8) -> Option<Self> {
        let mut frame = Self::zeroed(dimensions)?;
        frame.fill(color);
        Some(frame)
    }

    /// Returns the 2D dimensions of the frame.
    #[must_use]
    pub const fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    /// Returns the width of the frame.
    #[must_use]
    pub const fn width(&self) -> Width {
        self.dimensions.width
    }

    /// Returns the height of the frame.
    #[must_use]
    pub const fn height(&self) -> Height {
        self.dimensions.height
    }

    /// Returns a slice of the underlying raw RGB byte buffer.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns a slice of the underlying raw RGB byte buffer.
    ///
    /// Alias for [`Self::data`].
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Returns a slice of the underlying raw RGB byte buffer.
    ///
    /// Alias for [`Self::data`].
    #[must_use]
    pub fn buffer(&self) -> &[u8] {
        &self.data
    }

    /// Consumes the frame and returns the owned raw RGB byte buffer.
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.data
    }

    /// Consumes the frame and returns the owned raw RGB byte buffer.
    ///
    /// Alias for [`Self::into_vec`].
    #[must_use]
    pub fn into_buffer(self) -> Vec<u8> {
        self.data
    }

    /// Returns the total number of bytes in the pixel buffer.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the pixel buffer is empty.
    ///
    /// Note that frames always have non-zero dimensions, so this always returns `false`.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the byte value at the given raw buffer index, or `None` if out of bounds.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<u8> {
        self.data.get(index).copied()
    }

    /// Returns the [`Rgb8`] pixel color at the given `(x, y)` coordinate,
    /// or `None` if the coordinate is out of bounds.
    #[must_use]
    pub fn pixel(&self, x: u16, y: u16) -> Option<Rgb8> {
        if x >= self.dimensions.width.get().get() || y >= self.dimensions.height.get().get() {
            return None;
        }

        let x_usize = usize::from(x);
        let y_usize = usize::from(y);
        let width_usize = usize::from(self.dimensions.width.get().get());

        // checked_mul and checked_add calculate row and pixel offsets safely.
        let row_offset = y_usize.checked_mul(width_usize)?;
        let pixel_index = row_offset.checked_add(x_usize)?;
        let byte_index = pixel_index.checked_mul(3)?;

        let r_idx = byte_index;
        let g_idx = byte_index.checked_add(1)?;
        let b_idx = byte_index.checked_add(2)?;

        let red = *self.data.get(r_idx)?;
        let green = *self.data.get(g_idx)?;
        let blue = *self.data.get(b_idx)?;

        Some(Rgb8::new(red, green, blue))
    }

    /// Returns the [`Rgb8`] pixel color at the given `(x, y)` coordinate,
    /// or `None` if the coordinate is out of bounds.
    ///
    /// Alias for [`Self::pixel`].
    #[must_use]
    pub fn get_pixel(&self, x: u16, y: u16) -> Option<Rgb8> {
        self.pixel(x, y)
    }

    /// Returns the 3-byte RGB slice for the pixel at `(x, y)`,
    /// or `None` if out of bounds.
    #[must_use]
    pub fn pixel_bytes(&self, x: u16, y: u16) -> Option<&[u8]> {
        if x >= self.dimensions.width.get().get() || y >= self.dimensions.height.get().get() {
            return None;
        }

        let x_usize = usize::from(x);
        let y_usize = usize::from(y);
        let width_usize = usize::from(self.dimensions.width.get().get());

        let row_offset = y_usize.checked_mul(width_usize)?;
        let pixel_index = row_offset.checked_add(x_usize)?;
        let byte_index = pixel_index.checked_mul(3)?;
        let end_index = byte_index.checked_add(3)?;

        self.data.get(byte_index..end_index)
    }

    /// Sets the pixel at `(x, y)` to `color`.
    ///
    /// Returns `None` if `(x, y)` is out of bounds, or `Some(())` on success.
    pub fn set_pixel(&mut self, x: u16, y: u16, color: Rgb8) -> Option<()> {
        if x >= self.dimensions.width.get().get() || y >= self.dimensions.height.get().get() {
            return None;
        }

        let x_usize = usize::from(x);
        let y_usize = usize::from(y);
        let width_usize = usize::from(self.dimensions.width.get().get());

        let row_offset = y_usize.checked_mul(width_usize)?;
        let pixel_index = row_offset.checked_add(x_usize)?;
        let byte_index = pixel_index.checked_mul(3)?;

        let r_idx = byte_index;
        let g_idx = byte_index.checked_add(1)?;
        let b_idx = byte_index.checked_add(2)?;

        *self.data.get_mut(r_idx)? = color.r;
        *self.data.get_mut(g_idx)? = color.g;
        *self.data.get_mut(b_idx)? = color.b;

        Some(())
    }

    /// Fills the entire frame with a uniform [`Rgb8`] color.
    pub fn fill(&mut self, color: Rgb8) {
        let (chunks, _) = self.data.as_chunks_mut::<3>();
        for pixel in chunks {
            *pixel = [color.r, color.g, color.b];
        }
    }

    /// Blends a pixel at `(x, y)` with an [`Rgb8`] color and alpha value.
    ///
    /// An `alpha` of 255 replaces the destination pixel entirely with `color`.
    /// An `alpha` of 0 leaves the destination pixel unchanged.
    /// Intermediate values linearly blend using integer arithmetic with symmetric rounding.
    ///
    /// Returns `None` if `(x, y)` is out of bounds, or `Some(())` if successfully blended.
    pub fn blend_pixel(&mut self, x: u16, y: u16, color: Rgb8, alpha: u8) -> Option<()> {
        if x >= self.dimensions.width.get().get() || y >= self.dimensions.height.get().get() {
            return None;
        }
        if alpha == 0 {
            return Some(());
        }

        let x_usize = usize::from(x);
        let y_usize = usize::from(y);
        let width_usize = usize::from(self.dimensions.width.get().get());

        let row_offset = y_usize.checked_mul(width_usize)?;
        let pixel_index = row_offset.checked_add(x_usize)?;
        let byte_index = pixel_index.checked_mul(3)?;

        let r_idx = byte_index;
        let g_idx = byte_index.checked_add(1)?;
        let b_idx = byte_index.checked_add(2)?;

        if alpha == 255 {
            *self.data.get_mut(r_idx)? = color.r;
            *self.data.get_mut(g_idx)? = color.g;
            *self.data.get_mut(b_idx)? = color.b;
            return Some(());
        }

        let dst_red = *self.data.get(r_idx)?;
        let dst_green = *self.data.get(g_idx)?;
        let dst_blue = *self.data.get(b_idx)?;

        *self.data.get_mut(r_idx)? = blend_channel(color.r, dst_red, alpha);
        *self.data.get_mut(g_idx)? = blend_channel(color.g, dst_green, alpha);
        *self.data.get_mut(b_idx)? = blend_channel(color.b, dst_blue, alpha);

        Some(())
    }
}

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
    Ratio::new(signed_numer, scale_denom).unwrap_or(Ratio {
        numer: 0,
        denom: fallback_denom,
    })
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
    Ratio::new(signed_numer, scale_denom).unwrap_or(Ratio {
        numer: 0,
        denom: fallback_denom,
    })
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

    /// Helper function to construct a [`Ratio`] for tests.
    fn make_ratio(numer: i64, denom: i64) -> Option<Ratio> {
        let nz = NonZeroI64::new(denom)?;
        Ratio::new(numer, nz)
    }

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

    #[test]
    fn test_frame_buffer_length_invariant() {
        let Some(w) = Width::new(4) else { return };
        let Some(h) = Height::new(3) else { return };
        let dims = Dimensions {
            width: w,
            height: h,
        };
        // Expected buffer size: 4 * 3 * 3 = 36 bytes.
        let Some(expected_len) = required_buffer_len(dims) else {
            return;
        };
        assert_eq!(expected_len, 36, "4x3 RGB frame requires exactly 36 bytes");

        // Too short buffer must be rejected.
        let too_short = vec![0_u8; 35];
        assert!(
            Frame::new(dims, too_short).is_none(),
            "buffer shorter than required must return None"
        );

        // Too long buffer must be rejected.
        let too_long = vec![0_u8; 37];
        assert!(
            Frame::new(dims, too_long).is_none(),
            "buffer longer than required must return None"
        );

        // Empty buffer must be rejected.
        assert!(
            Frame::new(dims, Vec::new()).is_none(),
            "empty buffer must return None"
        );

        // Exact length buffer must succeed.
        let exact = vec![0_u8; 36];
        let frame_opt = Frame::new(dims, exact);
        assert!(
            frame_opt.is_some(),
            "exact length buffer must return Some(Frame)"
        );
        let Some(frame) = frame_opt else { return };
        assert_eq!(frame.len(), 36, "frame len must equal 36");
        assert!(!frame.is_empty(), "frame must not be empty");
        assert_eq!(frame.dimensions(), dims, "dimensions must match");
        assert_eq!(frame.width(), w, "width must match");
        assert_eq!(frame.height(), h, "height must match");
    }

    #[test]
    fn test_frame_zeroed_and_from_color() {
        let Some(w) = Width::new(2) else { return };
        let Some(h) = Height::new(2) else { return };
        let dims = Dimensions {
            width: w,
            height: h,
        };

        let Some(zeroed) = Frame::zeroed(dims) else {
            return;
        };
        assert_eq!(zeroed.len(), 12, "2x2 frame must have 12 bytes");
        for b in zeroed.data() {
            assert_eq!(*b, 0, "all bytes in zeroed frame must be 0");
        }

        let red = Rgb8::new(255, 0, 0);
        let Some(color_frame) = Frame::from_color(dims, red) else {
            return;
        };
        for y in 0..2_u16 {
            for x in 0..2_u16 {
                assert_eq!(
                    color_frame.pixel(x, y),
                    Some(red),
                    "pixel must match filled color"
                );
            }
        }
    }

    #[test]
    fn test_frame_bounds_checked_accessors() {
        let Some(w) = Width::new(3) else { return };
        let Some(h) = Height::new(2) else { return };
        let dims = Dimensions {
            width: w,
            height: h,
        };
        let Some(mut frame) = Frame::zeroed(dims) else {
            return;
        };

        // Within bounds
        let col = Rgb8::new(10, 20, 30);
        assert!(
            frame.set_pixel(1, 1, col).is_some(),
            "setting pixel within bounds must succeed"
        );
        assert_eq!(
            frame.pixel(1, 1),
            Some(col),
            "pixel at (1, 1) must return set color"
        );
        assert_eq!(
            frame.get_pixel(1, 1),
            Some(col),
            "get_pixel at (1, 1) must return set color"
        );
        assert_eq!(
            frame.pixel_bytes(1, 1),
            Some(&[10, 20, 30][..]),
            "pixel_bytes at (1, 1) must return [10, 20, 30]"
        );

        // Out of bounds: x == width
        assert!(
            frame.pixel(3, 0).is_none(),
            "pixel at x == width must return None"
        );
        assert!(
            frame.pixel_bytes(3, 0).is_none(),
            "pixel_bytes at x == width must return None"
        );
        assert!(
            frame.set_pixel(3, 0, col).is_none(),
            "set_pixel at x == width must return None"
        );

        // Out of bounds: y == height
        assert!(
            frame.pixel(0, 2).is_none(),
            "pixel at y == height must return None"
        );
        assert!(
            frame.set_pixel(0, 2, col).is_none(),
            "set_pixel at y == height must return None"
        );

        // Far out of bounds
        assert!(
            frame.pixel(u16::MAX, u16::MAX).is_none(),
            "pixel at u16::MAX must return None"
        );
        assert!(
            frame.pixel_bytes(u16::MAX, u16::MAX).is_none(),
            "pixel_bytes at u16::MAX must return None"
        );
        assert!(
            frame.set_pixel(u16::MAX, u16::MAX, col).is_none(),
            "set_pixel at u16::MAX must return None"
        );

        // Raw byte access
        assert_eq!(frame.get(0), Some(0), "byte 0 must be 0");
        assert!(
            frame.get(18).is_none(),
            "byte index 18 on 18-byte buffer must return None"
        );
    }

    #[test]
    fn test_frame_fill() {
        let Some(w) = Width::new(2) else { return };
        let Some(h) = Height::new(2) else { return };
        let dims = Dimensions {
            width: w,
            height: h,
        };
        let Some(mut frame) = Frame::zeroed(dims) else {
            return;
        };

        let green = Rgb8::new(0, 255, 0);
        frame.fill(green);

        for y in 0..2_u16 {
            for x in 0..2_u16 {
                assert_eq!(
                    frame.pixel(x, y),
                    Some(green),
                    "every pixel after fill must match green"
                );
            }
        }
    }

    #[test]
    fn test_frame_blend_pixel() {
        let Some(w) = Width::new(2) else { return };
        let Some(h) = Height::new(2) else { return };
        let dims = Dimensions {
            width: w,
            height: h,
        };
        let Some(mut frame) = Frame::zeroed(dims) else {
            return;
        };

        // Set pixel (0, 0) to white (255, 255, 255)
        let white = Rgb8::new(255, 255, 255);
        let _ = frame.set_pixel(0, 0, white);

        // Blend with black (0, 0, 0) at alpha 0 -> must remain white
        let black = Rgb8::new(0, 0, 0);
        assert!(
            frame.blend_pixel(0, 0, black, 0).is_some(),
            "blend at alpha 0 must succeed"
        );
        assert_eq!(
            frame.pixel(0, 0),
            Some(white),
            "pixel with alpha 0 blend must remain unchanged"
        );

        // Blend with red (255, 0, 0) at alpha 255 -> must become pure red
        let red = Rgb8::new(255, 0, 0);
        assert!(
            frame.blend_pixel(0, 0, red, 255).is_some(),
            "blend at alpha 255 must succeed"
        );
        assert_eq!(
            frame.pixel(0, 0),
            Some(red),
            "pixel with alpha 255 blend must become source color"
        );

        // Set pixel (1, 1) to (100, 100, 100) and blend with (200, 200, 200) at alpha 128
        let gray100 = Rgb8::new(100, 100, 100);
        let _ = frame.set_pixel(1, 1, gray100);
        let gray200 = Rgb8::new(200, 200, 200);
        assert!(
            frame.blend_pixel(1, 1, gray200, 128).is_some(),
            "blend at alpha 128 must succeed"
        );
        // Blend formula: (200 * 128 + 100 * 127 + 127) / 255 = (25600 + 12700 + 127) / 255 = 38427 / 255 = 150
        let Some(blended) = frame.pixel(1, 1) else {
            return;
        };
        assert_eq!(
            blended.r, 150,
            "blended channel at 50% opacity must equal 150"
        );
        assert_eq!(
            blended.g, 150,
            "blended channel at 50% opacity must equal 150"
        );
        assert_eq!(
            blended.b, 150,
            "blended channel at 50% opacity must equal 150"
        );

        // Out of bounds blend returns None
        assert!(
            frame.blend_pixel(2, 0, red, 255).is_none(),
            "out of bounds blend must return None"
        );
        assert!(
            frame.blend_pixel(u16::MAX, u16::MAX, red, 255).is_none(),
            "out of bounds blend at u16::MAX must return None"
        );
    }
}
