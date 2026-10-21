//! Quantization table management and scaling.
use crate::dct::reorder_to_zigzag;

use core::num::NonZeroU8;

use crate::config::Quality;
use crate::huffman::Component;

/// A single JPEG quantization divisor: guaranteed nonzero (1-255) by
/// construction. `dct::quantize` divides a DCT coefficient by this
/// value -- if it could ever be zero, that division would silently
/// produce infinity/NaN (a real bug this crate hit once: a computed
/// scaling value could truncate to 0 before this type existed). Making
/// zero unconstructable here means `dct::quantize` never has to prove
/// its divisor is nonzero -- the type already guarantees it. Backed by a
/// real `NonZeroU8`, not a plain `u8` a caller trusts is nonzero: widening
/// it into the `NonZeroU64` `dct::quantize` actually divides by is then a
/// total `From` conversion, with nothing to unwrap at that boundary.
#[derive(Debug, Clone, Copy)]
pub(crate) struct QuantDivisor(NonZeroU8);

impl QuantDivisor {
    /// Clamps `value` into the valid range `1..=255` and wraps it.
    /// There is no way to construct a `QuantDivisor` of 0 -- every
    /// constructor clamps to at least 1.
    const fn new(value: u32) -> Self {
        let clamped = if value < 1 {
            1
        } else if value > 255 {
            255
        } else {
            value
        };
        // clamped is now guaranteed 1..=255, so this truncation is lossless,
        // and NonZeroU8::new always succeeds; the None arm is unreachable.
        let [lo, ..] = clamped.to_le_bytes();
        Self(match NonZeroU8::new(lo) {
            Some(divisor) => divisor,
            None => NonZeroU8::MIN,
        })
    }

    /// The underlying divisor value (1-255).
    #[must_use]
    pub(crate) const fn get(self) -> NonZeroU8 {
        self.0
    }
}

impl Default for QuantDivisor {
    /// Default to the minimum valid divisor value (1).
    fn default() -> Self {
        Self(NonZeroU8::MIN)
    }
}

/// Standard JPEG luminance quantization table.
const STANDARD_LUMA_QUANTIZATION: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// Standard JPEG chrominance quantization table.
const STANDARD_CHROMA_QUANTIZATION: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

/// Lookup table for quality scaling factors.
/// Index by quality value (1-100); index 0 is unused.
const SCALE_BY_QUALITY: [u16; 101] = [
    0, 256, 256, 256, 256, 256, 256, 256, 256, 256, 256, 256, 256, 256, 256, 256, 256, 256, 256,
    256, 250, 238, 227, 217, 208, 200, 192, 185, 178, 172, 166, 161, 156, 151, 147, 142, 138, 135,
    131, 128, 125, 121, 119, 116, 113, 111, 108, 106, 104, 102, 100, 98, 96, 94, 92, 90, 88, 86,
    84, 82, 80, 78, 76, 74, 72, 70, 68, 66, 64, 62, 60, 58, 56, 54, 52, 50, 48, 46, 44, 42, 40, 38,
    36, 34, 32, 30, 28, 26, 24, 22, 20, 18, 16, 14, 12, 10, 8, 6, 4, 2, 2,
];

/// Quantization table bytes in JPEG zigzag scan order, as required for a
/// DQT marker.
///
/// Distinct from `QuantTable::divisors()` which returns raster-order divisors
/// -- these are genuinely different orderings of the same 64 values, and a
/// previous bug in this codebase came from writing raster-order bytes where
/// zigzag order was required; this type makes that specific mistake a compile
/// error instead of a silent spec violation.
#[derive(Debug, Clone, Copy)]
pub struct ZigzagQuantBytes([u8; 64]);

impl ZigzagQuantBytes {
    /// The raw bytes, in the order a DQT marker expects them.
    #[must_use]
    pub const fn as_marker_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

/// A JPEG quantization table: 64 per-coefficient divisors in raster order.
///
/// Used internally (in raster order) to quantize DCT coefficients. A DQT
/// marker needs the bytes in zigzag order instead -- see
/// [`Self::as_zigzag_bytes`].
#[derive(Debug, Clone, Copy)]
pub struct QuantTable([QuantDivisor; 64]);

impl QuantTable {
    /// The divisors in RASTER (row-major, zigzag-independent) order --
    /// this is what `dct::quantize` needs for per-coefficient division, NOT
    /// what a DQT marker needs. A DQT marker requires zigzag order; use
    /// [`Self::as_zigzag_bytes`] for that. Mixing these two orderings up is
    /// a real JPEG spec violation this codebase has hit once before, which
    /// is why this crate restricts this method to `pub(crate)` -- nothing
    /// outside this crate has a legitimate reason to touch raster-order
    /// divisors directly.
    #[must_use]
    pub(crate) const fn divisors(&self) -> [QuantDivisor; 64] {
        self.0
    }

    /// This table's bytes reordered into JPEG zigzag scan order, as
    /// required for a DQT marker (the internal raster-order
    /// representation is what `dct::quantize` needs; a DQT marker needs
    /// zigzag order -- these are genuinely different orderings of the
    /// same 64 values, and mixing them up is a real spec violation, not
    /// just a style concern).
    #[must_use]
    pub fn as_zigzag_bytes(&self) -> ZigzagQuantBytes {
        let raster_bytes: [u8; 64] = self.0.map(|divisor| divisor.get().get());
        ZigzagQuantBytes(reorder_to_zigzag(&raster_bytes))
    }
}

/// Scales a quantization table based on quality (1-100).
#[must_use]
pub fn scale_table(quality: Quality, component: Component) -> QuantTable {
    let standard = match component {
        Component::Luma => &STANDARD_LUMA_QUANTIZATION,
        Component::Chroma => &STANDARD_CHROMA_QUANTIZATION,
    };

    let scale = u32::from(
        match SCALE_BY_QUALITY.get(usize::from(quality.get())).copied() {
            Some(entry) => entry,
            // Unreachable: `Quality` guarantees `1..=100`, and
            // `SCALE_BY_QUALITY` has 101 entries (indices `0..=100`).
            None if quality.get() == 0 => 2,
            None => 2,
        },
    );

    // Each entry is `(table_val * scale + 50) / 100`, the rounded
    // quality-scaled quantization value, clamped into a `QuantDivisor`.
    // Total: `table_val` is at most `u8::MAX` (255) and `scale` is at most
    // 256 (the largest entry in `SCALE_BY_QUALITY`), so the product is at
    // most `255 * 256 = 65_280` and the biased sum at most `65_330`, both
    // far below `u32::MAX`.
    QuantTable(standard.map(|table_val| {
        let product = match u32::from(table_val).checked_mul(scale) {
            Some(exact) => exact,
            None if table_val == 0 => 0,
            None => u32::MAX,
        };
        let biased = match product.checked_add(50) {
            Some(sum) => sum,
            None if product == 0 => 50,
            None => u32::MAX,
        };
        QuantDivisor::new(biased.div_euclid(100))
    }))
}

#[cfg(test)]
mod tests;
