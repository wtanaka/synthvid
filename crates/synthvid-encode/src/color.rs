//! RGB to YCbCr color space conversion.

use synthvid_scene::Rgb8;

/// Luma (brightness) component of YCbCr.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Luma(u8);

impl Luma {
    /// Creates a new luma value.
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    /// Gets the luma value.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Blue-difference chroma component of YCbCr.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ChromaBlue(u8);

impl ChromaBlue {
    /// Creates a new blue-difference chroma value.
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    /// Gets the chroma blue value.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Red-difference chroma component of YCbCr.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ChromaRed(u8);

impl ChromaRed {
    /// Creates a new red-difference chroma value.
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    /// Gets the chroma red value.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// YCbCr color components.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct YCbCr {
    /// Luma component (brightness).
    y: u8,
    /// Blue-difference chroma component.
    cb: u8,
    /// Red-difference chroma component.
    cr: u8,
}

impl YCbCr {
    /// Creates a new `YCbCr` color value.
    ///
    /// # Arguments
    /// * `y` - Luma component
    /// * `cb` - Blue-difference chroma component
    /// * `cr` - Red-difference chroma component
    #[must_use]
    pub const fn new(y: Luma, cb: ChromaBlue, cr: ChromaRed) -> Self {
        Self {
            y: y.get(),
            cb: cb.get(),
            cr: cr.get(),
        }
    }

    /// Gets the luma component.
    #[must_use]
    pub const fn y(self) -> u8 {
        self.y
    }

    /// Gets the blue-difference chroma component.
    #[must_use]
    pub const fn cb(self) -> u8 {
        self.cb
    }

    /// Gets the red-difference chroma component.
    #[must_use]
    pub const fn cr(self) -> u8 {
        self.cr
    }
}

/// Narrows a clamped `i32` to `u8` via a total conversion. `v.clamp(0, 255)`
/// is always representable in a single byte with the other three bytes of
/// its little-endian form all zero, so reading that low byte out directly
/// (rather than `u8::try_from` plus a fallback for a failure case that can
/// never actually happen after the clamp) needs no fallback at all.
fn clamp_to_u8(v: i32) -> u8 {
    let [lo, ..] = v.clamp(0, 255).to_le_bytes();
    lo
}

/// Computes `rounding + sum(coeff * value for (coeff, value) in terms)`,
/// checked at every step.
///
/// Takes the whole computation in one call, not separate two-`i32`
/// -parameter `add`/`mul` helpers: both operations are commutative, so
/// swapping their arguments would not even be a real transposition risk,
/// but each would still count as another "adjacent same-type parameters"
/// function against this crate's own transposable-parameter budget, for
/// no benefit -- just two more `(i32, i32)` argument lists the mechanical
/// scan cannot tell apart from a genuine transposition risk.
///
/// Every term's product is at most `255 * 150 = 38_250` in magnitude
/// (`value` is always a `u8`-sourced `i32`, and every coefficient used in
/// this file has magnitude at most 150), and the widest possible total
/// (all three channels at 255, plus `rounding`) is `65_408`, both far
/// inside `i32`'s range -- so the overflow arms below are never actually
/// reached. Written with `checked_mul`/`checked_add` and a match, rather
/// than `saturating_mul`/`saturating_add`, so that fact is demonstrated by
/// construction instead of relied on silently. The overflow arms still
/// saturate in the mathematically correct direction, rather than to a
/// fixed placeholder, purely so nothing downstream could misread an
/// unreachable overflow as a small or negative result it manifestly isn't.
fn weighted_sum(terms: [(i32, i32); 3], rounding: i32) -> i32 {
    let mut total = rounding;
    for (coeff, value) in terms {
        let product = match coeff.checked_mul(value) {
            Some(p) => p,
            None if (coeff < 0) == (value < 0) => i32::MAX,
            None => i32::MIN,
        };
        total = match total.checked_add(product) {
            Some(sum) => sum,
            None if product < 0 => i32::MIN,
            None => i32::MAX,
        };
    }
    total
}

/// Adds the JPEG chroma bias (128) to `v`. `v` is always a `weighted_sum`
/// result (magnitude well inside `i32`, see there) right-shifted by 8, so
/// adding a further 128 can only ever overflow toward positive infinity,
/// never negative -- unlike [`weighted_sum`]'s general case, there is only
/// one overflow direction to saturate toward here.
const fn add_chroma_bias(v: i32) -> i32 {
    match v.checked_add(128) {
        Some(sum) => sum,
        None => i32::MAX,
    }
}

/// Converts RGB to YCbCr using standard JPEG coefficients (ITU-R BT.601).
///
/// The output is in the range `[0, 255]` for all channels. Every term is
/// bounded in magnitude by `255 * 150 = 38_250` and the widest possible
/// summed total (all channels at 255) is `65_408`, both far inside `i32`
/// -- see `weighted_sum`, below.
#[must_use]
pub fn rgb_to_ycbcr(rgb: Rgb8) -> YCbCr {
    let r_val = i32::from(rgb.r);
    let g_val = i32::from(rgb.g);
    let b_val = i32::from(rgb.b);

    // Standard JPEG coefficients (scaled by 256 for integer arithmetic, rounded to nearest)
    // Y = 0.299*R + 0.587*G + 0.114*B (scaled: 77*R + 150*G + 29*B, sum=256)
    // Cb = -0.168736*R - 0.331264*G + 0.5*B + 128 (scaled: -43*R - 85*G + 128*B, then +128 offset)
    // Cr = 0.5*R - 0.418688*G - 0.081312*B + 128 (scaled: 128*R - 107*G - 21*B, then +128 offset)
    // Each uses (sum + 128) >> 8 for rounding, then chroma adds the 128 offset.

    // Compute luma. Bounded to [0, 255] by construction (see doc comment
    // above), so no clamp is needed before narrowing.
    let luma_raw = weighted_sum([(77, r_val), (150, g_val), (29, b_val)], 128) >> 8;
    let y = clamp_to_u8(luma_raw);

    // Compute blue chroma. This can reach exactly 256 at (r, g, b) =
    // (0, 0, 255) -- one past u8 range -- so, unlike luma, it still needs
    // an explicit clamp; `clamp_to_u8` saturates that single tie to 255
    // rather than silently wrapping.
    let blue_raw = weighted_sum([(-43, r_val), (-85, g_val), (128, b_val)], 128) >> 8;
    let cb = clamp_to_u8(add_chroma_bias(blue_raw));

    // Compute red chroma. Symmetric edge case at (r, g, b) = (255, 0, 0).
    let red_raw = weighted_sum([(128, r_val), (-107, g_val), (-21, b_val)], 128) >> 8;
    let cr = clamp_to_u8(add_chroma_bias(red_raw));

    YCbCr::new(Luma::new(y), ChromaBlue::new(cb), ChromaRed::new(cr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_to_ycbcr_converts_white() {
        let ycbcr = rgb_to_ycbcr(Rgb8::new(255, 255, 255));
        // White (255, 255, 255) should give exactly luma 255 with proper rounding
        // and neutral chroma (128, 128) since white has no color bias
        assert_eq!(ycbcr.y(), 255);
        assert_eq!(ycbcr.cb(), 128);
        assert_eq!(ycbcr.cr(), 128);
    }

    #[test]
    fn test_rgb_to_ycbcr_converts_black() {
        let ycbcr = rgb_to_ycbcr(Rgb8::new(0, 0, 0));
        assert_eq!(ycbcr.y(), 0);
        assert_eq!(ycbcr.cb(), 128);
        assert_eq!(ycbcr.cr(), 128);
    }

    #[test]
    fn test_rgb_to_ycbcr_small_red_value() {
        // Test proper rounding of chroma with small values
        // rgb_to_ycbcr(Rgb8::new(1, 0, 0)): blue_sum = -43*1 + -85*0 + 128*0 = -43
        // (-43 + 128) >> 8 = 85 >> 8 = 0, so blue_raw = 0, cb = 0 + 128 = 128
        let ycbcr = rgb_to_ycbcr(Rgb8::new(1, 0, 0));
        assert_eq!(ycbcr.cb(), 128);
        // Similarly for red: red_sum = 128*1 + -107*0 + -21*0 = 128
        // (128 + 128) >> 8 = 256 >> 8 = 1, so red_raw = 1, cr = 1 + 128 = 129
        assert_eq!(ycbcr.cr(), 129);
    }
}
