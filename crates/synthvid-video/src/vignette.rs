//! The radial vignette: how brightness falls off from the frame center toward the corners.
//!
//! A [`Vignette::Radial`] dims each sensel by its distance from the frame center, treating each
//! channel value as linear light. The center is the origin of a doubled coordinate system, so no
//! fraction is needed: a sensel in column `x` of a frame `W` columns wide has the doubled offset
//! `2x + 1 - W`, and a sensel in row `y` of a frame `H` rows tall has `2y + 1 - H`. The squared
//! radius fraction of a sensel is its squared doubled offset over `W^2 + H^2`. That fraction is
//! below 1.0 for every sensel, because the far corner sensel lies inside the squared diagonal.
//!
//! The loss of brightness at a sensel is `strength * 256 * r2` in parts per 65536, floored, where
//! the strength is the raw [`Q8_8`](crate::medium::Q8_8) value of the [`VignetteStrength`]. Each
//! channel value `v` becomes `floor((v * factor + 32768) / 65536)`, where `factor` is the remaining
//! brightness in parts per 65536, so ties round toward positive infinity. A factor of 65536 leaves
//! a channel unchanged.
//!
//! Every intermediate value is checked. A value that does not fit its integer type is
//! [`VignetteError::Overflow`], never a wrapped or substituted value.

use core::error::Error;
use core::fmt;
use core::num::{NonZeroI64, NonZeroU64};

use synthvid_scene::frame::{Frame, PixelColumn, PixelCoord, PixelOutOfBounds, PixelRow};
use synthvid_scene::{Channel, Dimensions, Overflow, Rgb8};

use crate::optics::{Vignette, VignetteStrength};

/// One whole unit of brightness, in parts per 65536.
const UNITY: i128 = 0x1_0000;

/// Half of a whole unit, the rounding offset that makes ties round toward positive infinity.
const HALF_UNITY: i128 = 0x8000;

/// The parts per 65536 that one raw [`Q8_8`](crate::medium::Q8_8) unit of strength stands for.
const PARTS_PER_RAW_UNIT: i128 = 256;

/// Why a radial vignette could not be applied to a frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VignetteError {
    /// An intermediate value of the vignette arithmetic does not fit its integer type.
    Overflow,
    /// A sensel lies outside the frame.
    PixelOutOfBounds,
}

impl fmt::Display for VignetteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => write!(formatter, "vignette arithmetic overflowed"),
            Self::PixelOutOfBounds => write!(formatter, "sensel lies outside the frame"),
        }
    }
}

impl Error for VignetteError {}

/// The doubled horizontal offset of a column from the frame center: `2x + 1 - W`.
///
/// The offset is negative for the left half of the frame and positive for the right half, and
/// its magnitude is less than the frame width.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct DoubledColumnOffset(pub i64);

/// The doubled vertical offset of a row from the frame center: `2y + 1 - H`.
///
/// The offset is negative for the top half of the frame and positive for the bottom half, and
/// its magnitude is less than the frame height.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct DoubledRowOffset(pub i64);

/// The doubled offsets of one sensel from the frame center, one for each axis.
///
/// The columns and rows are distinct types, so the two offsets cannot be passed in each other's
/// place. The fields are private, so the offsets can only be taken from a sensel's position in
/// a frame with [`FrameCenterOffsets::of_sensel`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FrameCenterOffsets {
    /// The doubled offset of the sensel's column.
    columns: DoubledColumnOffset,
    /// The doubled offset of the sensel's row.
    rows: DoubledRowOffset,
}

impl FrameCenterOffsets {
    /// Takes the doubled offsets of the sensel at the given column and row of a frame.
    ///
    /// # Errors
    ///
    /// Returns `Err(VignetteError::Overflow)` if an offset does not fit an `i64`.
    pub fn of_sensel(
        column: PixelColumn,
        row: PixelRow,
        dimensions: Dimensions,
    ) -> Result<Self, VignetteError> {
        let PixelColumn(column_index) = column;
        let PixelRow(row_index) = row;
        let width = NonZeroI64::from(dimensions.width.get()).get();
        let height = NonZeroI64::from(dimensions.height.get()).get();
        let columns = i64::from(column_index)
            .checked_mul(2)
            .and_then(|doubled| doubled.checked_add(1))
            .and_then(|doubled| doubled.checked_sub(width))
            .ok_or(VignetteError::Overflow)?;
        let rows = i64::from(row_index)
            .checked_mul(2)
            .and_then(|doubled| doubled.checked_add(1))
            .and_then(|doubled| doubled.checked_sub(height))
            .ok_or(VignetteError::Overflow)?;
        Ok(Self {
            columns: DoubledColumnOffset(columns),
            rows: DoubledRowOffset(rows),
        })
    }
}

/// The squared diagonal of a frame, `W^2 + H^2`, which normalizes the squared offsets.
///
/// The value is always positive, because both dimensions are nonzero. It is taken from the
/// frame's dimensions, so it cannot disagree with them.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct DoubledExtent(u64);

impl DoubledExtent {
    /// Takes the squared diagonal of a frame with the given dimensions.
    ///
    /// # Errors
    ///
    /// Returns `Err(VignetteError::Overflow)` if the sum does not fit a `u64`.
    pub fn from_dimensions(dimensions: Dimensions) -> Result<Self, VignetteError> {
        let width = NonZeroU64::from(dimensions.width.get()).get();
        let height = NonZeroU64::from(dimensions.height.get()).get();
        let width_squared = width.checked_mul(width).ok_or(VignetteError::Overflow)?;
        let height_squared = height.checked_mul(height).ok_or(VignetteError::Overflow)?;
        let sum = width_squared
            .checked_add(height_squared)
            .ok_or(VignetteError::Overflow)?;
        Ok(Self(sum))
    }
}

/// The brightness that remains at one sensel, in parts per 65536.
///
/// A factor of 65536 leaves the light unchanged. The field is private, and the only constructor
/// refuses any value above 65536, so a factor can never brighten a channel. The bound holds
/// because the loss of brightness is never negative.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VignetteFactor(u32);

impl VignetteFactor {
    /// Takes the remaining brightness from its exact value in parts per 65536.
    ///
    /// # Errors
    ///
    /// Returns `Err(VignetteError::Overflow)` if the value is negative or above 65536, because
    /// it does not fit the range of a factor.
    fn from_remaining(brightness: RemainingBrightness) -> Result<Self, VignetteError> {
        let RemainingBrightness(remaining) = brightness;
        if remaining > UNITY {
            return Err(VignetteError::Overflow);
        }
        let raw = u32::try_from(remaining)
            .map_err(Overflow::from)
            .map_err(|Overflow| VignetteError::Overflow)?;
        Ok(Self(raw))
    }
}

/// The exact remaining brightness of one sensel in parts per 65536, before it is checked
/// against the range of a [`VignetteFactor`]. Every `i128` is a legal raw value here.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct RemainingBrightness(i128);

/// The brightness factor of one sensel under a radial vignette.
///
/// The loss is `strength * 256 * (dx2^2 + dy2^2) / (W^2 + H^2)`, floored, in parts per 65536, and
/// the factor is `65536 - loss`.
///
/// # Errors
///
/// Returns `Err(VignetteError::Overflow)` if an intermediate value does not fit its integer type.
pub fn vignette_factor(
    strength: VignetteStrength,
    offsets: FrameCenterOffsets,
    extent: DoubledExtent,
) -> Result<VignetteFactor, VignetteError> {
    let FrameCenterOffsets { columns, rows } = offsets;
    let DoubledColumnOffset(column_raw) = columns;
    let DoubledRowOffset(row_raw) = rows;
    let DoubledExtent(extent_raw) = extent;
    let strength_raw = i128::from(strength.get().0);

    let column_wide = i128::from(column_raw);
    let row_wide = i128::from(row_raw);
    let column_square = column_wide
        .checked_mul(column_wide)
        .ok_or(VignetteError::Overflow)?;
    let row_square = row_wide
        .checked_mul(row_wide)
        .ok_or(VignetteError::Overflow)?;
    let squared_offset = column_square
        .checked_add(row_square)
        .ok_or(VignetteError::Overflow)?;

    let loss = strength_raw
        .checked_mul(PARTS_PER_RAW_UNIT)
        .and_then(|scaled| scaled.checked_mul(squared_offset))
        .and_then(|scaled| scaled.checked_div(i128::from(extent_raw)))
        .ok_or(VignetteError::Overflow)?;
    let factor = UNITY.checked_sub(loss).ok_or(VignetteError::Overflow)?;
    VignetteFactor::from_remaining(RemainingBrightness(factor))
}

/// Scales one channel value by a factor, rounding to the nearest code value.
///
/// The result is `floor((value * factor + 32768) / 65536)`. The factor is at most 65536, so the
/// result is at most 255 and always fits a channel.
fn scale_channel(value: Channel, factor: VignetteFactor) -> Result<Channel, VignetteError> {
    let Channel(raw) = value;
    let VignetteFactor(factor_raw) = factor;
    let scaled = i128::from(raw)
        .checked_mul(i128::from(factor_raw))
        .and_then(|product| product.checked_add(HALF_UNITY))
        .and_then(|sum| sum.checked_div_euclid(UNITY))
        .ok_or(VignetteError::Overflow)?;
    let byte = u8::try_from(scaled)
        .map_err(Overflow::from)
        .map_err(|Overflow| VignetteError::Overflow)?;
    Ok(Channel(byte))
}

/// Dims every sensel of a frame by its distance from the center, in place.
fn apply_radial(frame: &mut Frame, strength: VignetteStrength) -> Result<(), VignetteError> {
    let dimensions = frame.dimensions();
    let extent = DoubledExtent::from_dimensions(dimensions)?;
    let width = dimensions.width.get().get();
    let height = dimensions.height.get().get();
    for row in 0..height {
        for column in 0..width {
            let coord = PixelCoord::new(PixelColumn(column), PixelRow(row));
            let color = frame
                .get_pixel(coord)
                .ok_or(VignetteError::PixelOutOfBounds)?;
            let offsets =
                FrameCenterOffsets::of_sensel(PixelColumn(column), PixelRow(row), dimensions)?;
            let factor = vignette_factor(strength, offsets, extent)?;
            let rendered = Rgb8 {
                red: scale_channel(color.red, factor)?,
                green: scale_channel(color.green, factor)?,
                blue: scale_channel(color.blue, factor)?,
            };
            frame
                .set_pixel(coord, rendered)
                .map_err(|PixelOutOfBounds| VignetteError::PixelOutOfBounds)?;
        }
    }
    Ok(())
}

/// Applies a lens vignette to a frame, in place, treating each channel value as linear light.
///
/// [`Vignette::None`] leaves the frame untouched. [`Vignette::Radial`] dims each sensel by its
/// distance from the frame center, as described in the module documentation.
///
/// # Errors
///
/// Returns `Err(VignetteError::Overflow)` if an intermediate value does not fit its integer
/// type, and `Err(VignetteError::PixelOutOfBounds)` if a sensel cannot be addressed in the frame.
/// On error the frame may hold partly updated sensels and must not be used.
pub fn apply_vignette(frame: &mut Frame, vignette: Vignette) -> Result<(), VignetteError> {
    match vignette {
        Vignette::None => Ok(()),
        Vignette::Radial(strength) => apply_radial(frame, strength),
    }
}

#[cfg(test)]
mod tests;
