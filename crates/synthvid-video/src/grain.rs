//! The grain of photographic film, rendered as a correlated noise field.
//!
//! The grain is a Gaussian approximation of the statistics of a thresholded grain field. Three
//! properties of such a field set the terms of the model:
//!
//! - the variance of a thresholded field goes as `coverage * (1 - coverage)`, so the standard
//!   deviation of a channel value `v` in code values is `density * sqrt(v * (255 - v)) / 16`,
//!   which is zero at black and white and about 8 code values at mid-gray for density 1.0;
//! - the grain size sets the spatial correlation, which is the width of a box filter in sensels,
//!   the [`GrainWindow`];
//! - each dye layer grains independently, so every color channel has its own field, and film
//!   grain differs from frame to frame, so every frame draws a fresh field.
//!
//! A channel's field is a box average of white normal draws. Every draw comes from a stream seed
//! computed in closed form, so a frame never depends on the frames rendered before it. Every
//! intermediate value is checked: a value that does not fit its integer type is
//! [`GrainError::Overflow`], and the rounding at the end clamps to the code range, which is the
//! saturation of the film rather than a substituted value.

use core::error::Error;
use core::fmt;
use core::num::{NonZeroU128, NonZeroU16};

use synthvid_scene::frame::{Frame, PixelColumn, PixelCoord, PixelOutOfBounds, PixelRow};
use synthvid_scene::{
    isqrt, Channel, Dimensions, FrameIndex, Height, IntSqrtInput, Overflow, Rgb8,
};

use crate::medium::{FilmGrain, GrainBlur, GrainDensity, GrainRadius, GrainSpread, Q8_8};
use crate::sensor::{round_to_channel, ColorPlane, SignedQ8, CODE_VALUE};

mod field;
#[cfg(test)]
mod tests;

/// The widest correlation length a grain may have, in sensels.
const MAX_WINDOW: u128 = 256;

/// The rounding offset of the window, which is half of [`SCALE_SQUARED`] in the denominator.
const WINDOW_ROUNDING: u128 = 0x8000;

/// `256 * 256`, the square of the Q8.8 scale, which the window formula divides by.
const SCALE_SQUARED: u128 = 0x1_0000;

/// The Q8.8 raw value of 1.0, which the blur term adds to.
const UNIT_RAW: u128 = 256;

/// `256 * 256` as a `u64`: the squared Q8.8 scale, which the spread term adds to and which scales
/// the Q16 variance of a channel value.
const SCALE_SQUARED_U64: u64 = 0x1_0000;

/// `256 * 16`, which divides `density_raw * isqrt(v * (255 - v) * 65536)` down to a Q8.8 deviation
/// of `density * sqrt(v * (255 - v)) / 16` code values.
const SIGMA_DIVISOR: u64 = 4_096;

/// Why film grain could not be applied to a frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GrainError {
    /// An intermediate value of the grain arithmetic does not fit its integer type.
    Overflow,
    /// The grain would need a correlation longer than 256 sensels, which the frame cannot hold.
    GrainTooCoarse,
    /// A sensel lies outside the frame.
    PixelOutOfBounds,
}

impl fmt::Display for GrainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => write!(formatter, "film grain arithmetic overflowed"),
            Self::GrainTooCoarse => write!(
                formatter,
                "film grain correlates more than 256 sensels, which is too coarse"
            ),
            Self::PixelOutOfBounds => write!(formatter, "sensel lies outside the frame"),
        }
    }
}

impl Error for GrainError {}

impl From<Overflow> for GrainError {
    fn from(Overflow: Overflow) -> Self {
        Self::Overflow
    }
}

/// The width of the box filter that correlates one channel's grain, in sensels.
///
/// The width is at least one and at most 256. The field is private, so a window is built only by
/// [`grain_window`] and no other code can hold one out of range.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GrainWindow(NonZeroU16);

impl GrainWindow {
    /// Returns the window width in sensels.
    #[must_use]
    pub(crate) const fn get(self) -> NonZeroU16 {
        self.0
    }
}

/// The correlation length of the grain on a frame of the given height, in sensels.
///
/// The window is `2 * radius * height * sqrt(1 + spread^2) * (1 + blur)`, rounded to the nearest
/// whole sensel with ties upward, and never less than one. The radius is the mean grain radius as
/// a fraction of the frame height. The spread and the blur are Q8.8 values, so the factor
/// `sqrt(1 + spread^2)` is `isqrt(65536 + spread_raw^2) / 256`, and `1 + blur` is
/// `(256 + blur_raw) / 256`.
///
/// # Errors
///
/// Returns `Err(GrainError::GrainTooCoarse)` if the window would exceed 256 sensels, and
/// `Err(GrainError::Overflow)` if an intermediate value does not fit its integer type.
pub fn grain_window(
    radius: GrainRadius,
    spread: GrainSpread,
    blur: GrainBlur,
    height: Height,
) -> Result<GrainWindow, GrainError> {
    let ratio = radius.get();
    let radius_numer = u128::try_from(ratio.numer().0).map_err(Overflow::from)?;
    let radius_denom = u128::try_from(ratio.denom().get()).map_err(Overflow::from)?;

    let GrainSpread(Q8_8(spread_raw)) = spread;
    let spread_squared = u64::from(spread_raw)
        .checked_mul(u64::from(spread_raw))
        .ok_or(GrainError::Overflow)?;
    let spread_sum = spread_squared
        .checked_add(SCALE_SQUARED_U64)
        .ok_or(GrainError::Overflow)?;
    let spread_root = u128::from(isqrt(IntSqrtInput(spread_sum)).0);

    let GrainBlur(Q8_8(blur_raw)) = blur;
    let blur_factor = u128::from(blur_raw)
        .checked_add(UNIT_RAW)
        .ok_or(GrainError::Overflow)?;

    let rows = NonZeroU128::from(height.get()).get();
    let rounding = radius_denom
        .checked_mul(WINDOW_ROUNDING)
        .ok_or(GrainError::Overflow)?;
    let numerator = radius_numer
        .checked_mul(2)
        .and_then(|product| product.checked_mul(rows))
        .and_then(|product| product.checked_mul(spread_root))
        .and_then(|product| product.checked_mul(blur_factor))
        .and_then(|product| product.checked_add(rounding))
        .ok_or(GrainError::Overflow)?;
    let denominator = radius_denom
        .checked_mul(SCALE_SQUARED)
        .ok_or(GrainError::Overflow)?;
    // The denominator is a positive multiple of 65536, so the division cannot see zero.
    let quotient = numerator
        .checked_div(denominator)
        .ok_or(GrainError::Overflow)?;

    let window = quotient.max(1);
    if window > MAX_WINDOW {
        return Err(GrainError::GrainTooCoarse);
    }
    // The window is between 1 and 256 here, so the narrowing cannot fail and the zero check
    // cannot see zero.
    let raw = u16::try_from(window).map_err(Overflow::from)?;
    let width = NonZeroU16::new(raw).ok_or(GrainError::Overflow)?;
    Ok(GrainWindow(width))
}

/// The standard deviation of the grain on one channel value, as a Q8.8 value in code values.
///
/// It is `density * sqrt(v * (255 - v)) / 16` code values, so a density of 1.0 gives about 8 code
/// values at mid-gray and nothing at black or white.
///
/// # Errors
///
/// Returns `Err(GrainError::Overflow)` if the deviation does not fit a Q8.8 value. A deviation is
/// never clamped.
pub fn grain_sigma(density: GrainDensity, value: Channel) -> Result<Q8_8, GrainError> {
    let Channel(code) = value;
    let complement = u8::MAX.checked_sub(code).ok_or(GrainError::Overflow)?;
    let variance = u64::from(code)
        .checked_mul(u64::from(complement))
        .and_then(|product| product.checked_mul(SCALE_SQUARED_U64))
        .ok_or(GrainError::Overflow)?;
    let root = isqrt(IntSqrtInput(variance)).0;
    let Q8_8(density_raw) = density.get();
    let scaled = u64::from(density_raw)
        .checked_mul(root)
        .ok_or(GrainError::Overflow)?;
    let sigma = scaled
        .checked_div(SIGMA_DIVISOR)
        .ok_or(GrainError::Overflow)?;
    let raw = u16::try_from(sigma).map_err(Overflow::from)?;
    Ok(Q8_8(raw))
}

/// What fixes one channel's noise field: the seed, the frame, the channel, the frame size and the
/// window.
#[derive(Copy, Clone, Debug)]
struct FieldShape {
    /// Seed of the grain pattern.
    seed: synthvid_scene::Seed,
    /// Index of the frame being rendered.
    index: FrameIndex,
    /// The channel whose field this is.
    plane: ColorPlane,
    /// Size of the frame.
    dimensions: Dimensions,
    /// Correlation length of the grain.
    window: GrainWindow,
}

/// Applies film grain to a frame, in place, as the frame at `index`.
///
/// Each channel of each sensel gets a deviation from its clean value, drawn from that channel's
/// own field, so the three channels are grained independently. The deviation is
/// [`grain_sigma`] of the clean value times the filtered draw, and the result is rounded and
/// clamped to the code range.
///
/// # Errors
///
/// Returns `Err(GrainError::GrainTooCoarse)` if the correlation length exceeds 256 sensels, and
/// `Err(GrainError::Overflow)` if an intermediate value does not fit its integer type. On error
/// the frame may hold a partly updated row and must not be used.
pub fn apply_film_grain(
    frame: &mut Frame,
    grain: &FilmGrain,
    index: FrameIndex,
) -> Result<(), GrainError> {
    let dimensions = frame.dimensions();
    let window = grain_window(
        grain.radius(),
        grain.spread(),
        grain.blur(),
        dimensions.height,
    )?;
    for plane in [ColorPlane::Red, ColorPlane::Green, ColorPlane::Blue] {
        let shape = FieldShape {
            seed: grain.seed().get(),
            index,
            plane,
            dimensions,
            window,
        };
        let noise = field::filtered_noise(shape)?;
        apply_plane(frame, grain.density(), plane, &noise)?;
    }
    Ok(())
}

/// Replaces one channel of every sensel of a frame with its grained value.
fn apply_plane(
    frame: &mut Frame,
    density: GrainDensity,
    plane: ColorPlane,
    noise: &[Vec<SignedQ8>],
) -> Result<(), GrainError> {
    let dimensions = frame.dimensions();
    let width = dimensions.width.get().get();
    let height = dimensions.height.get().get();
    for (row, noise_row) in (0..height).zip(noise) {
        for (column, &filtered) in (0..width).zip(noise_row) {
            let coord = PixelCoord::new(PixelColumn(column), PixelRow(row));
            let color = frame.get_pixel(coord).ok_or(GrainError::PixelOutOfBounds)?;
            let level = grained_level(channel_of(color, plane), density, filtered)?;
            let value = round_to_channel(level)?;
            frame
                .set_pixel(coord, with_channel(color, plane, value))
                .map_err(|PixelOutOfBounds| GrainError::PixelOutOfBounds)?;
        }
    }
    Ok(())
}

/// The clean channel value plus its grain, as a signed Q8.8 level before rounding.
///
/// The deviation is `sigma * filtered / 256`, floored, where `sigma` is taken from the clean value.
fn grained_level(
    clean: Channel,
    density: GrainDensity,
    filtered: SignedQ8,
) -> Result<SignedQ8, GrainError> {
    let Channel(code) = clean;
    let Q8_8(sigma_raw) = grain_sigma(density, clean)?;
    let SignedQ8(filtered_raw) = filtered;
    let deviation = i64::from(sigma_raw)
        .checked_mul(filtered_raw)
        .and_then(|product| product.checked_div_euclid(CODE_VALUE))
        .ok_or(GrainError::Overflow)?;
    let base = i64::from(code)
        .checked_mul(CODE_VALUE)
        .ok_or(GrainError::Overflow)?;
    let level = base.checked_add(deviation).ok_or(GrainError::Overflow)?;
    Ok(SignedQ8(level))
}

/// The value of one channel of a color.
const fn channel_of(color: Rgb8, plane: ColorPlane) -> Channel {
    match plane {
        ColorPlane::Red => color.red,
        ColorPlane::Green => color.green,
        ColorPlane::Blue => color.blue,
    }
}

/// A color with one channel replaced.
const fn with_channel(color: Rgb8, plane: ColorPlane, value: Channel) -> Rgb8 {
    match plane {
        ColorPlane::Red => Rgb8 {
            red: value,
            ..color
        },
        ColorPlane::Green => Rgb8 {
            green: value,
            ..color
        },
        ColorPlane::Blue => Rgb8 {
            blue: value,
            ..color
        },
    }
}
