//! Projection of a vertex and of a radius onto the sensel lattice.
//!
//! The camera's output is `W` by `H` sensels, and its focal length is `fn / 16` frame heights, so
//! one frame height is `H` sensels on the image plane. A world point at offset `(dx, dy)` from the
//! camera axis and depth `z` lands at `16 * H * fn * (X, Y) / z` in 1/256-sensel units from the
//! frame center, where `(X, Y)` is the offset rotated by the roll. The frame center is at lattice
//! coordinates `(128 * W, 128 * H)`, because a sensel center sits at `256 * c + 128`.
//!
//! The rotated offsets carry the 2^15 scale of [`TrigPair`], so the divisor of a vertex is
//! `2^15 * z`. Each projected coordinate is rounded once, half up, and a coordinate whose
//! magnitude exceeds 2^28 lattice units is [`ProjectionError::BeyondGuardBand`].
//!
//! The guard band is a hard limit of rendering. A layer whose projected extent, or whose lens
//! shift, exceeds 2^28 lattice units, which is 2^20 sensels, makes the frame fail. The projection
//! never clips or culls such a layer, because a silently dropped layer would be a wrong image.
//! A layer must therefore be kept farther from the camera, or made smaller, to render.

use core::num::{NonZeroI128, NonZeroU32};

use synthvid_scene::{Dimensions, Extent, Height, LatticePoint, Overflow, SubSensel, Width};

use crate::optics::FocalLength;

use super::error::ProjectionError;
use super::roll::TrigPair;
use super::round::{round_half_up, NonZeroWideInt, RoundingInput, WideInt};

/// The largest magnitude of a projected lattice coordinate, 2^28 lattice units, or 2^20 sensels.
const GUARD_BAND: WideInt = WideInt(0x1000_0000);

/// The lattice units in one sensel, 256, scaled by 128 for the center of a sensel.
const SENSEL_CENTER: WideInt = WideInt(128);

/// The factor of 16 that turns a frame height into sixteenths of a frame height.
const SIXTEENTHS: WideInt = WideInt(16);

/// The trigonometric scale of 2^15 that the rotated offsets carry.
const TRIG_SCALE: WideInt = WideInt(0x8000);

/// A distance along one world axis from the camera axis, in world units.
///
/// Every `i64` is a legal offset: a magnitude too large for the lattice is refused by the
/// projection, not by this type.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct WorldOffset(pub i64);

/// The offset of a vertex from the camera, across and down, in world units.
///
/// The axes are distinct fields, so the across and down offsets cannot be passed in each other's
/// place.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PlaneOffset {
    /// The offset to the right of the camera axis.
    pub across: WorldOffset,
    /// The offset below the camera axis.
    pub down: WorldOffset,
}

/// The distance of a point forward from the camera, in world units, at least one.
///
/// The count is non-zero, so a point cannot lie in the plane of the camera. The caller culls
/// points at or behind that plane before projecting them.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Depth(pub NonZeroU32);

impl From<WorldOffset> for WideInt {
    fn from(offset: WorldOffset) -> Self {
        Self(i128::from(offset.0))
    }
}

impl From<Depth> for WideInt {
    fn from(depth: Depth) -> Self {
        Self(NonZeroI128::from(depth.0).get())
    }
}

impl From<Depth> for NonZeroWideInt {
    fn from(depth: Depth) -> Self {
        Self(NonZeroI128::from(depth.0))
    }
}

impl From<FocalLength> for WideInt {
    fn from(focal: FocalLength) -> Self {
        Self(NonZeroI128::from(focal.0).get())
    }
}

impl From<Width> for WideInt {
    fn from(width: Width) -> Self {
        Self(NonZeroI128::from(width.0).get())
    }
}

impl From<Height> for WideInt {
    fn from(height: Height) -> Self {
        Self(NonZeroI128::from(height.0).get())
    }
}

impl From<Extent> for WideInt {
    fn from(extent: Extent) -> Self {
        Self(NonZeroI128::from(extent.get()).get())
    }
}

/// Checks a projected lattice coordinate against the guard band and narrows it to lattice units.
///
/// # Errors
///
/// Returns `Err(ProjectionError::BeyondGuardBand)` if the magnitude of `coordinate` exceeds 2^28.
/// Returns `Err(ProjectionError::Overflow)` if a value within the guard band does not fit an
/// `i64`, which the guard band prevents.
pub(super) fn guarded(coordinate: WideInt) -> Result<SubSensel, ProjectionError> {
    // The unsigned magnitude cannot overflow, so the guard compares every `i128` honestly,
    // including the most negative one.
    if coordinate.0.unsigned_abs() > GUARD_BAND.0.unsigned_abs() {
        return Err(ProjectionError::BeyondGuardBand);
    }
    let raw = i64::try_from(coordinate.0).map_err(Overflow::from)?;
    Ok(SubSensel(raw))
}

/// Projects a vertex at the given offset and depth onto the lattice.
///
/// The result is `(128 * W + P_across, 128 * H + P_down)`, where `P` is the rotated offset
/// scaled by `16 * H * fn / (2^15 * z)` and rounded once, half up.
///
/// # Errors
///
/// Returns `Err(ProjectionError::Overflow)` if an intermediate value leaves the `i128` range.
/// Returns `Err(ProjectionError::BeyondGuardBand)` if either lattice coordinate has a magnitude
/// above 2^28.
pub fn project_vertex(
    offset: PlaneOffset,
    depth: Depth,
    trig: TrigPair,
    focal: FocalLength,
    dimensions: Dimensions,
) -> Result<LatticePoint, ProjectionError> {
    let PlaneOffset { across, down } = offset;
    let cosine = WideInt::from(trig.cosine());
    let sine = WideInt::from(trig.sine());
    let across_world = WideInt::from(across);
    let down_world = WideInt::from(down);

    let rotated_across = cosine.times(across_world)?.minus(sine.times(down_world)?)?;
    let rotated_down = sine.times(across_world)?.plus(cosine.times(down_world)?)?;

    let scale = SIXTEENTHS
        .times(WideInt::from(dimensions.height))?
        .times(WideInt::from(focal))?;
    let denominator = NonZeroWideInt::new(TRIG_SCALE.times(WideInt::from(depth))?)?;

    let across_offset = round_half_up(RoundingInput {
        numerator: scale.times(rotated_across)?,
        denominator,
    })?;
    let down_offset = round_half_up(RoundingInput {
        numerator: scale.times(rotated_down)?,
        denominator,
    })?;

    let center_across = SENSEL_CENTER.times(WideInt::from(dimensions.width))?;
    let center_down = SENSEL_CENTER.times(WideInt::from(dimensions.height))?;
    Ok(LatticePoint {
        across: guarded(center_across.plus(across_offset)?)?,
        down: guarded(center_down.plus(down_offset)?)?,
    })
}

/// Projects a world radius at the given depth onto the lattice, as a length.
///
/// The result is `16 * H * fn * r / z`, rounded once, half up, in 1/256-sensel units.
///
/// # Errors
///
/// Returns `Err(ProjectionError::Overflow)` if an intermediate value leaves the `i128` range.
/// Returns `Err(ProjectionError::BeyondGuardBand)` if the length has a magnitude above 2^28.
pub fn project_radius(
    radius: Extent,
    depth: Depth,
    focal: FocalLength,
    dimensions: Dimensions,
) -> Result<SubSensel, ProjectionError> {
    let numerator = SIXTEENTHS
        .times(WideInt::from(dimensions.height))?
        .times(WideInt::from(focal))?
        .times(WideInt::from(radius))?;
    let rounded = round_half_up(RoundingInput {
        numerator,
        denominator: NonZeroWideInt::from(depth),
    })?;
    guarded(rounded)
}

#[cfg(test)]
mod tests;
