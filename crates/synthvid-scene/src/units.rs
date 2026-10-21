//! Newtypes for the fixed quantities that describe a video.
//!
//! Frame indices and counts, pixel dimensions, frame rate, and the seed that
//! makes a render reproducible. Each one is a distinct type so they cannot be
//! passed in the wrong order or mixed up with a bare integer.

use core::error::Error;
use core::fmt;
use core::num::{NonZeroI64, NonZeroU16, NonZeroU32};

use crate::ratio::{Numerator, Overflow, Ratio};

/// Zero-based index of a frame in a video sequence.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
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

/// Count of frames in a video sequence, guaranteed to be non-zero.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FrameCount(pub NonZeroU32);

/// Error returned when attempting to construct a zero [`FrameCount`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FrameCountError {
    /// Frame count must be strictly positive.
    Zero,
}

impl fmt::Display for FrameCountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => write!(f, "frame count must be strictly positive"),
        }
    }
}

impl Error for FrameCountError {}

impl FrameCount {
    /// Creates a frame count if `count` is non-zero.
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameCountError::Zero)` if `count` is zero.
    pub const fn new(count: u32) -> Result<Self, FrameCountError> {
        match NonZeroU32::new(count) {
            Some(nz) => Ok(Self(nz)),
            None => Err(FrameCountError::Zero),
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

/// Error returned when attempting to construct a zero [`Width`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WidthError {
    /// Width must be strictly positive.
    Zero,
}

impl fmt::Display for WidthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => write!(f, "width must be strictly positive"),
        }
    }
}

impl Error for WidthError {}

impl Width {
    /// Creates a new width if `width` is non-zero.
    ///
    /// # Errors
    ///
    /// Returns `Err(WidthError::Zero)` if `width` is zero.
    pub const fn new(width: u16) -> Result<Self, WidthError> {
        match NonZeroU16::new(width) {
            Some(nz) => Ok(Self(nz)),
            None => Err(WidthError::Zero),
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

/// Error returned when attempting to construct a zero [`Height`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum HeightError {
    /// Height must be strictly positive.
    Zero,
}

impl fmt::Display for HeightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => write!(f, "height must be strictly positive"),
        }
    }
}

impl Error for HeightError {}

impl Height {
    /// Creates a new height if `height` is non-zero.
    ///
    /// # Errors
    ///
    /// Returns `Err(HeightError::Zero)` if `height` is zero.
    pub const fn new(height: u16) -> Result<Self, HeightError> {
        match NonZeroU16::new(height) {
            Some(nz) => Ok(Self(nz)),
            None => Err(HeightError::Zero),
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
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameRateError::NonPositive)` if the rate is zero or negative.
    pub const fn new(rate: Ratio) -> Result<Self, FrameRateError> {
        if rate.numer().0 > 0 {
            Ok(Self(rate))
        } else {
            Err(FrameRateError::NonPositive)
        }
    }

    /// Creates a frame rate from a [`Ratio`], rejecting zero and negative values.
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameRateError::NonPositive)` if the rate is zero or negative.
    pub const fn from_ratio(rate: Ratio) -> Result<Self, FrameRateError> {
        Self::new(rate)
    }

    /// Creates an exact frame rate from an integer frames per second value.
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameRateError::NonPositive)` if `fps` is zero.
    pub fn from_fps(fps: u32) -> Result<Self, FrameRateError> {
        let nz_fps = NonZeroU32::new(fps).ok_or(FrameRateError::NonPositive)?;
        let ratio = Ratio::from_integer(Numerator(NonZeroI64::from(nz_fps).get()));
        Self::new(ratio)
    }

    /// Creates an exact frame rate from numerator and non-zero denominator (e.g. 30000 / 1001).
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameRateError::NonPositive)` if the numerator is zero or negative.
    pub fn from_fraction(numer: u32, denom: NonZeroU32) -> Result<Self, FrameRateError> {
        let numer_wide = i64::from(numer);
        let ratio = Ratio::new(Numerator(numer_wide), NonZeroI64::from(denom))
            .map_err(|Overflow| FrameRateError::NonPositive)?;
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

impl Error for FrameRateError {}

impl TryFrom<Ratio> for FrameRate {
    type Error = FrameRateError;

    fn try_from(rate: Ratio) -> Result<Self, Self::Error> {
        Self::new(rate)
    }
}

/// Seed for deterministic pseudo-random number generation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
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

/// Zero-based index of an object in [`Scene`](crate::scene::Scene)::`objects`.
///
/// Exists so an error can name which object failed without carrying a bare
/// integer.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
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

#[cfg(test)]
mod tests;
