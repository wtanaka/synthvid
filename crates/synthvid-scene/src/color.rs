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
/// Returns `None` only if intermediate arithmetic overflows (which cannot occur
/// for valid 8-bit color channels).
#[must_use]
pub(crate) fn blend_channel(src: u8, dst: u8, alpha: u8) -> Option<u8> {
    let src_u32 = u32::from(src);
    let dst_u32 = u32::from(dst);
    let alpha_u32 = u32::from(alpha);
    // saturating_sub prevents underflow since alpha <= 255.
    let inv_alpha = 255_u32.saturating_sub(alpha_u32);
    // Both src_u32 and alpha_u32 are at most 255, so their product (max 65,025) fits in u32.
    let term1 = src_u32.checked_mul(alpha_u32)?;
    // Both dst_u32 and inv_alpha are at most 255, so their product (max 65,025) fits in u32.
    let term2 = dst_u32.checked_mul(inv_alpha)?;
    // Sum of two products, each at most 65,025 (total 130,050), fits in u32.
    let sum = term1.checked_add(term2)?;
    // Adding 127 to sum (max 130,050) gives max 130,177, which fits in u32.
    let sum_rounded = sum.checked_add(127)?;
    // Division by 255 always succeeds since 255 is non-zero.
    let blended = sum_rounded.checked_div(255)?;
    // The result is at most 130,177 / 255 ≈ 255.5. By the alpha-blending formula,
    // the result is always in [0, 255].
    u8::try_from(blended).ok()
}

#[cfg(test)]
mod tests {
    use super::blend_channel;

    /// `blend_channel` returns `Option` only because `arithmetic_side_effects`
    /// forces `checked_*`; the arithmetic itself cannot overflow. Since
    /// `alpha + inv_alpha == 255`, the weighted sum is at most `255 * 255`, so
    /// every intermediate fits in `u16`, let alone the `u32` it uses.
    ///
    /// The whole input space is `256^3`, so that claim is checked exhaustively
    /// rather than argued. The reference is computed in `u64` with a different
    /// shape, so it does not inherit a mistake from the code under test. If a
    /// future edit breaks the bound, this fails rather than letting a `None`
    /// reach a caller.
    #[test]
    fn blend_channel_is_total_and_exact_over_every_input() {
        for alpha in 0..=u8::MAX {
            let inv = u8::MAX.checked_sub(alpha).expect("alpha never exceeds 255");
            for src in 0..=u8::MAX {
                for dst in 0..=u8::MAX {
                    let got = blend_channel(src, dst, alpha)
                        .expect("blend_channel is total over u8 x u8 x u8");
                    let from_src = u64::from(src)
                        .checked_mul(u64::from(alpha))
                        .expect("u8 times u8 fits in u64");
                    let from_dst = u64::from(dst)
                        .checked_mul(u64::from(inv))
                        .expect("u8 times u8 fits in u64");
                    let total = from_src
                        .checked_add(from_dst)
                        .and_then(|t| t.checked_add(127))
                        .expect("the weighted sum plus 127 fits in u64");
                    let expected = u8::try_from(total.checked_div(255).expect("255 is not zero"))
                        .expect("the rounded weighted mean of two u8 values is a u8");
                    assert_eq!(got, expected, "blend(src={src}, dst={dst}, alpha={alpha})");
                }
            }
        }
    }
}
