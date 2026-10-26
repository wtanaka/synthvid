//! How a camera's optics focus light onto the image plane before it is recorded.
//!
//! A flat camera records a two-dimensional scene, which has no depth, so the only optical effect
//! that applies to it is a vignette. [`Optics::Off`] is an ideal lens with no effect, and
//! [`Optics::Vignette`] dims the frame toward its corners by the given [`VignetteStrength`].
//!
//! A [`Lens`] describes a real lens by its focal length, its [`Aperture`], and its [`Vignette`].
//! The deep pipeline renders a lens: a deep shot's rig holds one, and the render uses it to
//! project each layer. A flat camera has no lens of its own. Lengths on the image plane are
//! measured in frame heights, so one frame height is the height of the image, and the
//! [`FrameHeight`] of an [`Iris`] gives that height in world units.
//!
//! Every quantity that must be positive is a type that cannot hold zero: a focal length is a
//! non-zero count of sixteenths of a frame height, and a frame height is a non-zero count of world
//! units. Every fraction of brightness is a [`Q8_8`] that is checked against its upper bound.

use core::error::Error;
use core::fmt;
use core::num::{NonZeroI64, NonZeroU16, NonZeroU32, NonZeroU8};

use synthvid_scene::{Numerator, Overflow, Ratio};

use crate::medium::Q8_8;

mod aperture;

pub use aperture::{
    Aperture, ApertureGrid, ApertureShape, BladeCount, BladeCountError, FStop, Iris,
};

/// The height of the frame on the image plane, in world units.
///
/// The height is non-zero by construction, so a frame cannot have no extent.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FrameHeight(pub NonZeroU16);

/// A focal length in sixteenths of a frame height: `FocalLength(n)` is `n / 16` frame heights.
///
/// Every non-zero count is legal, so the length cannot be zero, and the sixteenths reach from
/// `1/16` to `255/16` of the frame height.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FocalLength(pub NonZeroU8);

impl FocalLength {
    /// The number of sixteenths in one frame height.
    const SIXTEENTHS_PER_FRAME_HEIGHT: i64 = 16;

    /// Returns the exact focal length as a fraction of the frame height.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if the exact fraction cannot be represented.
    pub fn ratio(self) -> Result<Ratio, Overflow> {
        let sixteenths = NonZeroI64::from(self.0).get();
        Ratio::from_integer(Numerator(sixteenths)).checked_div(Ratio::from_integer(Numerator(
            Self::SIXTEENTHS_PER_FRAME_HEIGHT,
        )))
    }
}

/// Error returned when attempting to construct a [`FocusDistance`] beyond 2^20 scene units.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FocusDistanceError {
    /// A focus distance above 1048576 scene units, which is 2^20, is refused.
    TooFar,
}

impl fmt::Display for FocusDistanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFar => write!(f, "focus distance must be at most 1048576 scene units"),
        }
    }
}

impl Error for FocusDistanceError {}

/// The distance from the camera to the plane of perfect focus, in scene units.
///
/// The distance is at least one scene unit, because it is non-zero, and at most 2^20 scene units,
/// because [`FocusDistance::new`] refuses anything farther.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FocusDistance(NonZeroU32);

impl FocusDistance {
    /// The farthest focus distance, 2^20 scene units.
    const MAX: u32 = 0x0010_0000;

    /// Creates a focus distance from a non-zero number of scene units.
    ///
    /// # Errors
    ///
    /// Returns `Err(FocusDistanceError::TooFar)` when `distance` is above 2^20 scene units.
    pub const fn new(distance: NonZeroU32) -> Result<Self, FocusDistanceError> {
        if distance.get() > Self::MAX {
            Err(FocusDistanceError::TooFar)
        } else {
            Ok(Self(distance))
        }
    }

    /// Returns the focus distance in scene units.
    #[must_use]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }
}

/// Where a lens is focused.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Focus {
    /// Focused at infinity, so distant objects are sharp.
    Infinity,
    /// Focused on the plane at the given distance from the camera.
    At(FocusDistance),
}

/// Error returned when attempting to construct a [`VignetteStrength`] above 100 percent.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VignetteStrengthError {
    /// Vignette strength must not exceed 1.0, which is a loss of the whole brightness.
    AboveOne,
}

impl fmt::Display for VignetteStrengthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AboveOne => write!(f, "vignette strength must not exceed 1.0"),
        }
    }
}

impl Error for VignetteStrengthError {}

/// The fraction of brightness lost at the frame corner, as a [`Q8_8`] value.
///
/// The raw value is non-zero, so a strength of zero is written as [`Vignette::None`] and not as
/// a radial vignette of no effect. [`VignetteStrength::new`] rejects raw values above 256, which
/// is 1.0 in [`Q8_8`], so the loss can never exceed the whole brightness.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VignetteStrength(NonZeroU16);

impl VignetteStrength {
    /// The raw [`Q8_8`] value of 1.0.
    const ONE: u16 = 256;

    /// Creates a vignette strength from its raw [`Q8_8`] value, in which 256 is 1.0.
    ///
    /// # Errors
    ///
    /// Returns `Err(VignetteStrengthError::AboveOne)` when `raw` is greater than 256.
    pub const fn new(raw: NonZeroU16) -> Result<Self, VignetteStrengthError> {
        if raw.get() > Self::ONE {
            Err(VignetteStrengthError::AboveOne)
        } else {
            Ok(Self(raw))
        }
    }

    /// Returns the vignette strength as a [`Q8_8`] value.
    #[must_use]
    pub const fn get(self) -> Q8_8 {
        Q8_8(self.0.get())
    }
}

/// How brightness falls off toward the frame corners.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Vignette {
    /// No falloff: the brightness is even across the frame.
    None,
    /// Brightness falls off radially toward the corners by the given strength.
    Radial(VignetteStrength),
}

/// A lens described by its focal length, its aperture, and its vignetting.
///
/// Each field is a separate typed value, so the three terms cannot be passed in each other's
/// place. Construction cannot fail: every combination of the terms is a lens that can be
/// described.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Lens {
    /// The focal length, in sixteenths of a frame height.
    focal_length: FocalLength,
    /// The aperture the light passes through.
    aperture: Aperture,
    /// The falloff of brightness toward the corners.
    vignette: Vignette,
}

impl Lens {
    /// Creates a lens from its focal length, aperture, and vignetting.
    #[must_use]
    pub const fn new(focal_length: FocalLength, aperture: Aperture, vignette: Vignette) -> Self {
        Self {
            focal_length,
            aperture,
            vignette,
        }
    }

    /// Returns the focal length, in sixteenths of a frame height.
    #[must_use]
    pub const fn focal_length(self) -> FocalLength {
        self.focal_length
    }

    /// Returns the aperture the light passes through.
    #[must_use]
    pub const fn aperture(self) -> Aperture {
        self.aperture
    }

    /// Returns the falloff of brightness toward the corners.
    #[must_use]
    pub const fn vignette(self) -> Vignette {
        self.vignette
    }
}

/// The optics of a camera: an ideal lens, or a vignette that a flat camera can express.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Optics {
    /// An ideal lens: perfectly sharp, with no vignetting.
    Off,
    /// A vignette of the given strength: brightness falls off radially toward the frame corners.
    Vignette(VignetteStrength),
}

#[cfg(test)]
mod tests;
