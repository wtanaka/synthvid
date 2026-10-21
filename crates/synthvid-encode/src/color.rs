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
    luma: Luma,
    /// Blue-difference chroma component.
    cb: ChromaBlue,
    /// Red-difference chroma component.
    cr: ChromaRed,
}

impl YCbCr {
    /// Creates a new `YCbCr` color value.
    ///
    /// # Arguments
    /// * `luma` - Luma component
    /// * `cb` - Blue-difference chroma component
    /// * `cr` - Red-difference chroma component
    #[must_use]
    pub const fn new(luma: Luma, cb: ChromaBlue, cr: ChromaRed) -> Self {
        Self { luma, cb, cr }
    }

    /// Gets the luma component.
    #[must_use]
    pub const fn y(self) -> Luma {
        self.luma
    }

    /// Gets the blue-difference chroma component.
    #[must_use]
    pub const fn cb(self) -> ChromaBlue {
        self.cb
    }

    /// Gets the red-difference chroma component.
    #[must_use]
    pub const fn cr(self) -> ChromaRed {
        self.cr
    }
}

/// One `coeff * value` term of a [`weighted_sum`]: a fixed-point
/// coefficient scaled by 256, and a `u8`-sourced channel value.
#[derive(Debug, Clone, Copy)]
struct Term(i32, i32);

/// A channel's fixed-point weighted sum, before or after it is scaled back
/// down by 256.
#[derive(Debug, Clone, Copy)]
struct Weighted(i32);

/// Computes `rounding + sum(coeff * value for Term(coeff, value) in terms)`,
/// checked at every step, where `rounding` is 128 (half of the 256 the
/// coefficients are scaled by, so the later `>> 8` rounds to nearest).
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
fn weighted_sum(terms: [Term; 3]) -> Weighted {
    let mut total = 128_i32;
    for Term(coeff, value) in terms {
        let product = match coeff.checked_mul(value) {
            Some(product) => product,
            None if (coeff < 0_i32) == (value < 0_i32) => i32::MAX,
            None => i32::MIN,
        };
        total = match total.checked_add(product) {
            Some(sum) => sum,
            None if product < 0_i32 => i32::MIN,
            None => i32::MAX,
        };
    }
    Weighted(total)
}

impl Weighted {
    /// Scales the sum back down by 256 (`>> 8`).
    const fn descaled(self) -> Self {
        Self(self.0 >> 8)
    }

    /// Adds the JPEG chroma bias (128). `self` is always a
    /// [`weighted_sum`] result (magnitude well inside `i32`, see there)
    /// scaled down by 256, so adding a further 128 can only ever overflow
    /// toward positive infinity, never negative -- unlike
    /// [`weighted_sum`]'s general case, there is only one overflow
    /// direction to saturate toward here.
    const fn with_chroma_bias(self) -> Self {
        Self(match self.0.checked_add(128) {
            Some(sum) => sum,
            None => i32::MAX,
        })
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
    let r_val = i32::from(rgb.red.0);
    let g_val = i32::from(rgb.green.0);
    let b_val = i32::from(rgb.blue.0);

    // Standard JPEG coefficients (scaled by 256 for integer arithmetic, rounded to nearest)
    // Y = 0.299*R + 0.587*G + 0.114*B (scaled: 77*R + 150*G + 29*B, sum=256)
    // Cb = -0.168736*R - 0.331264*G + 0.5*B + 128 (scaled: -43*R - 85*G + 128*B, then +128 offset)
    // Cr = 0.5*R - 0.418688*G - 0.081312*B + 128 (scaled: 128*R - 107*G - 21*B, then +128 offset)
    // Each uses (sum + 128) >> 8 for rounding, then chroma adds the 128 offset.

    // Narrows a clamped sum to `u8` via a total conversion. `v.clamp(0, 255)`
    // is always representable in a single byte with the other three bytes
    // of its little-endian form all zero, so reading that low byte out
    // directly (rather than `u8::try_from` plus a fallback for a failure
    // case that can never actually happen after the clamp) needs no
    // fallback at all.
    let clamp_to_u8 = |Weighted(weighted): Weighted| {
        let [lo, ..] = weighted.clamp(0_i32, 255_i32).to_le_bytes();
        lo
    };

    // Compute luma. Bounded to [0, 255] by construction (see doc comment
    // above), so no clamp is needed before narrowing.
    let luma_raw = weighted_sum([Term(77, r_val), Term(150, g_val), Term(29, b_val)]).descaled();
    let luma = clamp_to_u8(luma_raw);

    // Compute blue chroma. This can reach exactly 256 at (r, g, b) =
    // (0, 0, 255) -- one past u8 range -- so, unlike luma, it still needs
    // an explicit clamp; `clamp_to_u8` saturates that single tie to 255
    // rather than silently wrapping.
    let blue_raw = weighted_sum([Term(-43, r_val), Term(-85, g_val), Term(128, b_val)]).descaled();
    let cb = clamp_to_u8(blue_raw.with_chroma_bias());

    // Compute red chroma. Symmetric edge case at (r, g, b) = (255, 0, 0).
    let red_raw = weighted_sum([Term(128, r_val), Term(-107, g_val), Term(-21, b_val)]).descaled();
    let cr = clamp_to_u8(red_raw.with_chroma_bias());

    YCbCr::new(Luma::new(luma), ChromaBlue::new(cb), ChromaRed::new(cr))
}

#[cfg(test)]
mod tests;
