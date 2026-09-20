//! Newtypes for the fixed quantities that describe a video.
//!
//! Frame indices and counts, pixel dimensions, frame rate, and the seed that
//! makes a render reproducible. Each one is a distinct type so they cannot be
//! passed in the wrong order or mixed up with a bare integer.

use core::fmt;
use core::num::{NonZeroI64, NonZeroU16, NonZeroU32};

use crate::ratio::Ratio;

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

/// Zero-based index of an object in [`Scene`](crate::scene::Scene)::`objects`.
///
/// Exists so an error can name which object failed without carrying a bare
/// integer.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct ObjectIndex(pub u32);

impl ObjectIndex {
    /// Creates a new object index.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Returns the underlying object index value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl From<u32> for ObjectIndex {
    fn from(index: u32) -> Self {
        Self(index)
    }
}

impl From<ObjectIndex> for u32 {
    fn from(index: ObjectIndex) -> Self {
        index.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU32;

    use crate::color::Rgb8;
    use crate::ratio::Ratio;

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
}
