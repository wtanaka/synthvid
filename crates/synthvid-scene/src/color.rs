//! Pixel colour and buffer sizing.
//!
//! [`Rgb8`] is the single colour representation used throughout the crate;
//! the free functions here size and blend the raw byte buffers that
//! [`Frame`](crate::Frame) owns.

use crate::units::Dimensions;

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
pub(crate) fn blend_channel(src: u8, dst: u8, alpha: u8) -> u8 {
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
