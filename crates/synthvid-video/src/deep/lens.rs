//! The shift of a layer's image caused by looking through one point of the lens aperture.
//!
//! A lens with an iris of f-number `Nn / Nd` and frame height `h` sees a layer at depth `z`
//! shift in the image when the view passes through a different point of the aperture. The
//! aperture is sampled on a grid of `A` by `A` points, and each sample is named by an odd
//! numerator `g` with `|g| <= A - 1`, so the unit-aperture coordinate is `g / A`. For a lens
//! focused at distance `s`, the shift along one axis is given by the following formula.
//!
//! `-H * fn^2 * h * Nd * g * (s - z) / (2 * Nn * A * z * s)`.
//!
//! For a lens focused at infinity it is `-H * fn^2 * h * Nd * g / (2 * Nn * A * z)`. Each shift
//! is rounded once, half up, to 1/256-sensel units.
//!
//! The formula is a far-field approximation. A thin lens of focal length `f` focused at distance
//! `s` forms its image at distance `v = f * s / (s - f)`, and the formula uses `f` in place of `v`.
//! The two agree to first order only when `s` is much larger than `f`. Here the world focal length
//! is `f = fn * H / 16`, where `fn` is the focal length in sixteenths of a frame height and `H` is
//! the frame height of the iris, both in the same world units as the scene and the focus distance.
//! A shot refuses a focus closer than eight focal lengths, that is `s < 8 * f` or `2 * s < fn * H`
//! in exact integers, before any shift is computed. At `s = 8 * f` the image distance is `8 * f / 7`,
//! so the approximation is off by one-seventh of `f` at the closest focus allowed, and the error
//! shrinks for every farther focus.

use core::error::Error;
use core::fmt;
use core::num::NonZeroI128;

use synthvid_scene::{Height, LatticeShift, SubSensel};

use crate::optics::{ApertureGrid, FocalLength, Focus, FrameHeight, Iris};

use super::error::ProjectionError;
use super::project::{guarded, Depth};
use super::round::{round_half_up, NonZeroWideInt, RoundingInput, WideInt};

/// The odd numerator of one lens sample, `2k + 1 - A` for sample index `k` of `A` samples.
///
/// The value is the sample's position across the aperture, scaled by `A`. Every `i64` is a
/// legal raw value at this level; [`LensSample::new`] refuses the values that name no sample.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct LensNumerator(pub i64);

/// The numerators of one lens sample on each axis of the aperture.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LensNumerators {
    /// The numerator of the sample across the aperture.
    pub across: LensNumerator,
    /// The numerator of the sample down the aperture.
    pub down: LensNumerator,
}

/// Why a lens sample could not be formed from its numerators and grid.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LensSampleError {
    /// A numerator lies outside the aperture, so its magnitude exceeds `A - 1`.
    OutsideAperture,
    /// A numerator is even, but a sample of an aperture grid always has an odd numerator.
    EvenNumerator,
}

impl fmt::Display for LensSampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideAperture => write!(f, "lens sample lies outside the aperture"),
            Self::EvenNumerator => write!(f, "lens sample numerator must be odd"),
        }
    }
}

impl Error for LensSampleError {}

/// One point of the lens aperture, named by its numerator on each axis.
///
/// Both numerators are odd and lie within `A - 1` of zero for the grid that the sample was
/// formed with. The fields are private, so a sample cannot be built with a numerator that names
/// no point of its grid.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LensSample {
    /// The numerator of the sample across the aperture.
    across: LensNumerator,
    /// The numerator of the sample down the aperture.
    down: LensNumerator,
    /// The grid the sample is taken from, which sets the aperture count `A`.
    grid: ApertureGrid,
}

impl LensSample {
    /// Forms a sample from its across and down numerators on the given grid.
    ///
    /// # Errors
    ///
    /// Returns `Err(LensSampleError::OutsideAperture)` if a numerator has a magnitude above
    /// `A - 1`, and `Err(LensSampleError::EvenNumerator)` if a numerator is even.
    pub fn new(numerators: LensNumerators, grid: ApertureGrid) -> Result<Self, LensSampleError> {
        let LensNumerators { across, down } = numerators;
        admit(across, grid)?;
        admit(down, grid)?;
        Ok(Self { across, down, grid })
    }

    /// Forms the sample of a candidate that a grid enumerates on its own axes, as the aperture
    /// points do when they walk their kept candidates. The numerators of such a candidate are odd
    /// and within `A - 1` of zero by the choice of the axis values, so the checks of [`Self::new`]
    /// hold without being run again.
    pub(super) const fn of_candidate(numerators: LensNumerators, grid: ApertureGrid) -> Self {
        Self {
            across: numerators.across,
            down: numerators.down,
            grid,
        }
    }

    /// Returns the numerator of the sample across the aperture.
    #[must_use]
    pub const fn across(self) -> LensNumerator {
        self.across
    }

    /// Returns the numerator of the sample down the aperture.
    #[must_use]
    pub const fn down(self) -> LensNumerator {
        self.down
    }

    /// Returns the grid the sample is taken from.
    #[must_use]
    pub const fn grid(self) -> ApertureGrid {
        self.grid
    }
}

/// The largest magnitude of a numerator on a grid, `A - 1`.
const fn largest_numerator(grid: ApertureGrid) -> LensNumerator {
    match grid {
        ApertureGrid::Two => LensNumerator(1),
        ApertureGrid::Four => LensNumerator(3),
        ApertureGrid::Eight => LensNumerator(7),
    }
}

/// The number of samples `A` along each axis of a grid.
const fn sample_count(grid: ApertureGrid) -> WideInt {
    match grid {
        ApertureGrid::Two => WideInt(2),
        ApertureGrid::Four => WideInt(4),
        ApertureGrid::Eight => WideInt(8),
    }
}

/// Checks that one numerator names a point inside the aperture of a grid.
///
/// # Errors
///
/// Returns `Err(LensSampleError::OutsideAperture)` if the magnitude exceeds `A - 1`, and
/// `Err(LensSampleError::EvenNumerator)` if the numerator is even.
fn admit(numerator: LensNumerator, grid: ApertureGrid) -> Result<(), LensSampleError> {
    let LensNumerator(raw) = numerator;
    let LensNumerator(limit) = largest_numerator(grid);
    // The unsigned magnitude cannot overflow, so the most negative numerator is refused as
    // outside the aperture like any other numerator beyond the grid.
    if raw.unsigned_abs() > limit.unsigned_abs() {
        return Err(LensSampleError::OutsideAperture);
    }
    if raw.checked_rem_euclid(2) != Some(1) {
        return Err(LensSampleError::EvenNumerator);
    }
    Ok(())
}

/// The inputs of a lens shift: the lens, the camera, the layer, and the aperture sample.
///
/// The fields are private, and `LensShiftInput::from_iris_sample` is the only constructor. The
/// aperture count of the shift is the grid of the sample, which is the grid its numerators name.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub(super) struct LensShiftInput {
    /// The iris, which gives the f-number, frame height, and focus.
    iris: Iris,
    /// The focal length of the camera, in sixteenths of a frame height.
    focal: FocalLength,
    /// The height of the output frame, in sensels.
    height: Height,
    /// The depth of the layer whose image shifts.
    depth: Depth,
    /// The point of the aperture that the view passes through.
    sample: LensSample,
}

impl LensShiftInput {
    /// Forms the inputs of a lens shift from the iris, the camera, the layer, and an aperture
    /// sample.
    ///
    /// The sample names its point with numerators on its own grid, and the shift divides by that
    /// grid's count. The iris contributes its f-number, frame height, and focus.
    pub(super) const fn from_iris_sample(
        iris: Iris,
        focal: FocalLength,
        height: Height,
        depth: Depth,
        sample: LensSample,
    ) -> Self {
        Self {
            iris,
            focal,
            height,
            depth,
            sample,
        }
    }
}

/// The terms of a lens shift that are shared by both axes of one layer.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct ShiftTerms {
    /// The product `H * fn^2 * h * Nd`, which every axis multiplies by its numerator.
    scale: WideInt,
    /// The numerator `Nn` of the f-number.
    stop_numerator: WideInt,
    /// The aperture count `A`.
    count: WideInt,
    /// The depth `z` of the layer.
    depth: WideInt,
    /// Where the lens is focused.
    focus: Focus,
}

/// The shift of a layer along one axis, for one numerator of the aperture.
///
/// # Errors
///
/// Returns `Err(ProjectionError::Overflow)` if an intermediate value leaves the `i128` range,
/// and `Err(ProjectionError::BeyondGuardBand)` if the shift has a magnitude above 2^28.
fn axis_shift(terms: &ShiftTerms, numerator: LensNumerator) -> Result<SubSensel, ProjectionError> {
    let negated = terms.scale.times(WideInt::from(numerator))?.negate()?;
    let rounded = match terms.focus {
        Focus::Infinity => {
            let denominator = WideInt(2)
                .times(terms.stop_numerator)?
                .times(terms.count)?
                .times(terms.depth)?;
            round_half_up(RoundingInput {
                numerator: negated,
                denominator: NonZeroWideInt::new(denominator)?,
            })?
        }
        Focus::At(distance) => {
            let focus = WideInt(NonZeroI128::from(distance.get()).get());
            let gap = focus.minus(terms.depth)?;
            let denominator = WideInt(2)
                .times(terms.stop_numerator)?
                .times(terms.count)?
                .times(terms.depth)?
                .times(focus)?;
            round_half_up(RoundingInput {
                numerator: negated.times(gap)?,
                denominator: NonZeroWideInt::new(denominator)?,
            })?
        }
    };
    guarded(rounded)
}

/// Returns the shift of a layer's image caused by looking through one point of the lens.
///
/// The result is the across and down shift in 1/256-sensel units, each rounded once, half up.
/// The aperture count `A` is the grid of the sample, which is the grid its numerators name, so the
/// shift is the same whichever grid the iris itself samples.
///
/// # Errors
///
/// Returns `Err(ProjectionError::Overflow)` if an intermediate value leaves the `i128` range.
/// Returns `Err(ProjectionError::BeyondGuardBand)` if either shift has a magnitude above 2^28.
pub(super) fn lens_shift(input: LensShiftInput) -> Result<LatticeShift, ProjectionError> {
    let LensShiftInput {
        iris,
        focal,
        height,
        depth,
        sample,
    } = input;
    let stop = iris.stop().nominal()?;
    let stop_numerator = WideInt(i128::from(stop.numer().0));
    let stop_denominator = WideInt(NonZeroI128::from(stop.denom()).get());
    let focal_wide = WideInt::from(focal);
    let scale = WideInt::from(height)
        .times(WideInt::from(iris.frame_height()))?
        .times(focal_wide.times(focal_wide)?)?
        .times(stop_denominator)?;
    let terms = ShiftTerms {
        scale,
        stop_numerator,
        count: sample_count(sample.grid()),
        depth: WideInt::from(depth),
        focus: iris.focus(),
    };
    Ok(LatticeShift {
        across: axis_shift(&terms, sample.across())?,
        down: axis_shift(&terms, sample.down())?,
    })
}

impl From<LensNumerator> for WideInt {
    fn from(numerator: LensNumerator) -> Self {
        Self(i128::from(numerator.0))
    }
}

impl From<FrameHeight> for WideInt {
    fn from(frame_height: FrameHeight) -> Self {
        Self(NonZeroI128::from(frame_height.0).get())
    }
}

#[cfg(test)]
mod tests;
