//! The aperture of a lens: how wide it opens, what shape it has, and how it is focused.
//!
//! An [`Aperture`] is either a pinhole, which admits light through a single point, or an [`Iris`].
//! An iris holds an [`FStop`], an [`ApertureShape`], a [`Focus`], the [`FrameHeight`] it is
//! described against, and the [`ApertureGrid`] of lens samples that gives its blur. An f-number is
//! dimensionless: it is the focal length divided by the diameter of the entrance pupil.

use core::error::Error;
use core::fmt;
use core::num::NonZeroU8;

use synthvid_scene::{Numerator, Overflow, Ratio};

use crate::optics::{Focus, FrameHeight};

/// A standard f-number of a lens aperture.
///
/// The variants are the stops of the conventional series.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum FStop {
    /// f/1.4.
    F1_4,
    /// f/2.
    F2,
    /// f/2.8.
    F2_8,
    /// f/4.
    F4,
    /// f/5.6.
    F5_6,
    /// f/8.
    F8,
    /// f/11.
    F11,
    /// f/16.
    F16,
    /// f/22.
    F22,
}

impl FStop {
    /// Returns the nominal f-number of the stop as an exact ratio.
    ///
    /// The nominal values are 7/5, 2, 14/5, 4, 28/5, 8, 11, 16, and 22.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if the exact ratio cannot be formed.
    pub fn nominal(self) -> Result<Ratio, Overflow> {
        match self {
            Self::F1_4 => {
                Ratio::from_integer(Numerator(7)).checked_div(Ratio::from_integer(Numerator(5)))
            }
            Self::F2 => Ok(Ratio::from_integer(Numerator(2))),
            Self::F2_8 => {
                Ratio::from_integer(Numerator(14)).checked_div(Ratio::from_integer(Numerator(5)))
            }
            Self::F4 => Ok(Ratio::from_integer(Numerator(4))),
            Self::F5_6 => {
                Ratio::from_integer(Numerator(28)).checked_div(Ratio::from_integer(Numerator(5)))
            }
            Self::F8 => Ok(Ratio::from_integer(Numerator(8))),
            Self::F11 => Ok(Ratio::from_integer(Numerator(11))),
            Self::F16 => Ok(Ratio::from_integer(Numerator(16))),
            Self::F22 => Ok(Ratio::from_integer(Numerator(22))),
        }
    }
}

/// Error returned when attempting to construct a [`BladeCount`] outside the range 3 to 16.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BladeCountError {
    /// A bladed aperture has at least three blades, so a count below three is rejected.
    BelowThree,
    /// A bladed aperture has at most sixteen blades, so a count above sixteen is rejected.
    AboveSixteen,
}

impl fmt::Display for BladeCountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BelowThree => write!(f, "blade count must be at least 3"),
            Self::AboveSixteen => write!(f, "blade count must be at most 16"),
        }
    }
}

impl Error for BladeCountError {}

/// The number of blades of a bladed aperture, from 3 to 16 inclusive.
///
/// The count is checked by [`BladeCount::new`], so a value outside the range cannot be built.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BladeCount(NonZeroU8);

impl BladeCount {
    /// The fewest blades an aperture may have.
    const MIN: u8 = 3;
    /// The most blades an aperture may have.
    const MAX: u8 = 16;

    /// Creates a blade count, rejecting counts below 3 and above 16.
    ///
    /// # Errors
    ///
    /// Returns `Err(BladeCountError::BelowThree)` when `count` is 1 or 2, and
    /// `Err(BladeCountError::AboveSixteen)` when `count` is greater than 16.
    pub const fn new(count: NonZeroU8) -> Result<Self, BladeCountError> {
        let value = count.get();
        if value < Self::MIN {
            Err(BladeCountError::BelowThree)
        } else if value > Self::MAX {
            Err(BladeCountError::AboveSixteen)
        } else {
            Ok(Self(count))
        }
    }

    /// Returns the number of blades.
    #[must_use]
    pub const fn get(self) -> NonZeroU8 {
        self.0
    }
}

/// The outline of a lens aperture.
///
/// A [`Disc`](ApertureShape::Disc) is a round iris. It is a distinct variant rather than a
/// polygon with many blades, because a round iris has no edges to render as a polygon.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ApertureShape {
    /// A round iris.
    Disc,
    /// A bladed iris with the given number of straight blades.
    Polygon(BladeCount),
}

/// The number of lens samples taken along each axis of an iris.
///
/// There is no single-sample grid, because one sample would silently remove the blur that the
/// aperture describes.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ApertureGrid {
    /// Two samples along each axis.
    Two,
    /// Four samples along each axis.
    Four,
    /// Eight samples along each axis.
    Eight,
}

/// The aperture a lens admits light through.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Aperture {
    /// A pinhole, which admits light through a single point and so has no iris to describe.
    Pinhole,
    /// An iris with the given stop, outline, focus, frame height, and sampling grid.
    Iris(Iris),
}

/// An iris described by its f-number, outline, focus, frame height, and sampling grid.
///
/// Each field is a separate typed value, so the five terms cannot be passed in each other's
/// place. Construction cannot fail: every combination of the terms is an iris that can be
/// described.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Iris {
    /// The f-number of the aperture.
    stop: FStop,
    /// The outline of the aperture.
    shape: ApertureShape,
    /// Where the lens is focused.
    focus: Focus,
    /// The height of the frame on the image plane, in world units.
    frame_height: FrameHeight,
    /// The number of lens samples taken along each axis.
    samples: ApertureGrid,
}

impl Iris {
    /// Creates an iris from its f-number, outline, focus, frame height, and sampling grid.
    #[must_use]
    pub const fn new(
        stop: FStop,
        shape: ApertureShape,
        focus: Focus,
        frame_height: FrameHeight,
        samples: ApertureGrid,
    ) -> Self {
        Self {
            stop,
            shape,
            focus,
            frame_height,
            samples,
        }
    }

    /// Returns the f-number of the aperture.
    #[must_use]
    pub const fn stop(self) -> FStop {
        self.stop
    }

    /// Returns the outline of the aperture.
    #[must_use]
    pub const fn shape(self) -> ApertureShape {
        self.shape
    }

    /// Returns where the lens is focused.
    #[must_use]
    pub const fn focus(self) -> Focus {
        self.focus
    }

    /// Returns the height of the frame on the image plane, in world units.
    #[must_use]
    pub const fn frame_height(self) -> FrameHeight {
        self.frame_height
    }

    /// Returns the number of lens samples taken along each axis.
    #[must_use]
    pub const fn samples(self) -> ApertureGrid {
        self.samples
    }
}

#[cfg(test)]
mod tests;
