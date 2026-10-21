//! Pixel color and buffer sizing.
//!
//! [`Rgb8`] is the single color representation used throughout the crate;
//! the free functions here size and blend the raw byte buffers that
//! [`Frame`](crate::Frame) owns.

use core::num::NonZeroUsize;

use crate::ratio::Overflow;
use crate::units::Dimensions;

/// 24-bit RGB pixel color with 8 bits per channel.
///
/// Built by naming each channel, `Rgb8 { red, green, blue }`, so two channels cannot
/// be swapped silently.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Rgb8 {
    /// Red channel value.
    pub red: Channel,
    /// Green channel value.
    pub green: Channel,
    /// Blue channel value.
    pub blue: Channel,
}

/// The intensity of one 8-bit color channel: `0` is none, `255` is full.
///
/// Every `u8` is a valid channel value. Kept apart from [`Alpha`] so an
/// intensity and an opacity cannot be swapped.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Channel(pub u8);

/// The opacity of one blend: `0` leaves the destination unchanged, `255`
/// replaces it. Kept apart from a channel value so the two cannot be swapped.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Alpha(pub u8);

/// Computes the exact byte length required for a 24-bit RGB frame with the given dimensions.
///
/// Returns `None` if `width * height * 3` overflows `usize`. The length of a
/// frame is never zero, because both dimensions are non-zero.
#[must_use]
pub fn required_buffer_len(dimensions: Dimensions) -> Option<NonZeroUsize> {
    // Convert u16 dimensions to usize for buffer calculation.
    let width = NonZeroUsize::from(dimensions.width.get()).get();
    let height = NonZeroUsize::from(dimensions.height.get()).get();
    // checked_mul prevents overflow on 32-bit platforms.
    let pixels = width.checked_mul(height)?;
    pixels.checked_mul(3).and_then(NonZeroUsize::new)
}

/// Blends `src` over `dst` with `alpha`, one 8-bit channel at a time, using
/// integer arithmetic.
///
/// Each channel uses the formula `(src * alpha + dst * (255 - alpha) + 127) / 255`.
/// Returns `Err(Overflow)` only if intermediate arithmetic overflows (which cannot occur
/// for valid 8-bit color channels).
///
/// # Errors
///
/// Returns `Err(Overflow)` if an intermediate value does not fit its integer type.
pub(crate) fn blend_rgb(src: Rgb8, alpha: Alpha, dst: Rgb8) -> Result<Rgb8, Overflow> {
    let channel = |from: Channel, onto: Channel| -> Result<Channel, Overflow> {
        let src_u32 = u32::from(from.0);
        let dst_u32 = u32::from(onto.0);
        let alpha_u32 = u32::from(alpha.0);
        // Total: alpha_u32 is at most 255 (widened from a u8), so subtracting
        // it from 255 never underflows.
        let inv_alpha = match 255_u32.checked_sub(alpha_u32) {
            Some(difference) => difference,
            None if alpha_u32 == 0 => 255,
            None => 0,
        };
        // Both src_u32 and alpha_u32 are at most 255, so their product (max 65,025) fits in u32.
        let term1 = src_u32.checked_mul(alpha_u32).ok_or(Overflow)?;
        // Both dst_u32 and inv_alpha are at most 255, so their product (max 65,025) fits in u32.
        let term2 = dst_u32.checked_mul(inv_alpha).ok_or(Overflow)?;
        // Sum of two products, each at most 65,025 (total 130,050), fits in u32.
        let sum = term1.checked_add(term2).ok_or(Overflow)?;
        // Adding 127 to sum (max 130,050) gives max 130,177, which fits in u32.
        let sum_rounded = sum.checked_add(127).ok_or(Overflow)?;
        // Division by 255 always succeeds since 255 is non-zero.
        let blended = sum_rounded.checked_div(255).ok_or(Overflow)?;
        // The result is at most 130,177 / 255 ≈ 255.5. By the alpha-blending formula,
        // the result is always in [0, 255].
        u8::try_from(blended).map(Channel).map_err(Overflow::from)
    };
    Ok(Rgb8 {
        red: channel(src.red, dst.red)?,
        green: channel(src.green, dst.green)?,
        blue: channel(src.blue, dst.blue)?,
    })
}

#[cfg(test)]
mod tests;
