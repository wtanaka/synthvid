//! DCT transform and quantization.

#[path = "dct_forward.rs"]
mod dct_forward;

use core::num::NonZeroU64;

use crate::quant;
use dct_forward::forward_dct;

/// An absolute DC coefficient value from a block's DCT.
///
/// This is distinct from [`DcDelta`], which is the difference between two DC
/// coefficients, and from [`DcPredictor`] (below), which is *also* an `i16`
/// arising from DC arithmetic but means something different again: a fresh
/// coefficient this block's own DCT just produced, versus the running value
/// a decoder would reconstruct by summing up previously-*written* (and
/// possibly clamped) deltas. Distinguishing all three by type prevents
/// confusion where a coefficient is written where a delta belongs, or a
/// predictor is advanced by a fresh coefficient instead of by the written
/// delta -- or, the specific risk that motivated splitting `DcPredictor`
/// out of this type, a coefficient and a predictor swapped in
/// `delta_from`'s two arguments, which silently negates every DC delta
/// this encoder writes. Stored as i16 since quantized DCT coefficients are
/// small.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DcCoefficient(i16);

impl DcCoefficient {
    /// Creates a new DC coefficient from a raw value.
    #[must_use]
    pub(crate) const fn new(value: i16) -> Self {
        Self(value)
    }

    /// Gets the underlying value as i32.
    #[must_use]
    pub fn get(self) -> i32 {
        i32::from(self.0)
    }

    /// Computes the delta between this coefficient and a predictor.
    ///
    /// Takes a [`DcPredictor`], not another `DcCoefficient`: the two are
    /// different quantities (this block's own fresh DCT output versus the
    /// running reconstruction of previously-written deltas) that happen to
    /// share a representation, and `self.delta_from(predictor)` versus
    /// `predictor.delta_from(self)` would otherwise both compile and
    /// silently swap which operand is negated. This is the value that
    /// should be encoded to the bitstream and may be clamped during
    /// encoding. Widened to i32, where the subtraction of two `i16`-derived
    /// values can never saturate, then narrowed back with a total
    /// conversion.
    #[must_use]
    pub(crate) fn delta_from(self, predictor: DcPredictor) -> DcDelta {
        let a = i32::from(self.0);
        let b = i32::from(predictor.0);
        // Both are widened from `i16` (`-32768..=32767`), so their
        // difference is always `-65535..=65535` -- far inside `i32` -- and
        // `checked_sub` never actually needs its overflow arms below.
        let delta = match a.checked_sub(b) {
            Some(d) => d,
            None if b > 0 => i32::MIN,
            None => i32::MAX,
        };
        DcDelta(saturate_to_i16(delta))
    }
}

/// The running DC value a real decoder reconstructs by summing up
/// previously-*written* (and possibly clamped) [`EncodedDcDelta`]s.
///
/// As opposed to [`DcCoefficient`], a single block's own fresh, unclamped
/// DCT output: both are `i16`s arising from DC arithmetic, but they answer
/// different questions, and a predictor can only ever be advanced by an
/// actually-written delta, never by a fresh coefficient -- see
/// `Self::advanced_by`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DcPredictor(i16);

impl DcPredictor {
    /// The predictor's value before any block has been encoded (zero, per
    /// the JPEG spec).
    pub(crate) const INITIAL: Self = Self(0);

    /// Computes the new predictor value after advancing by a written delta.
    ///
    /// Takes an [`EncodedDcDelta`], not a general [`DcDelta`]: the predictor
    /// must advance by exactly the delta that was actually written to the
    /// bitstream, since that is what a real decoder reconstructs its own
    /// predictor from. Advancing by an unclamped `DcDelta` instead would
    /// desynchronize this encoder's internal predictor from what decoding
    /// the emitted bytes would produce -- the type rules that out rather
    /// than relying on every call site remembering to pass the right one.
    /// Widened to i32, where the sum of two `i16`-derived values can never
    /// saturate, then narrowed back with a total conversion.
    #[must_use]
    pub(crate) fn advanced_by(self, written: EncodedDcDelta) -> Self {
        let a = i32::from(self.0);
        let b = written.get();
        // Both are widened from `i16` (`-32768..=32767`), so their sum is
        // always `-65535..=65535` -- far inside `i32` -- and `checked_add`
        // never actually needs its overflow arms below.
        let sum = match a.checked_add(b) {
            Some(s) => s,
            None if b < 0 => i32::MIN,
            None => i32::MAX,
        };
        Self(saturate_to_i16(sum))
    }
}

/// Narrows an `i32` to `i16`, saturating at the bounds. Total: every `i32`
/// value maps to some `i16`.
fn saturate_to_i16(v: i32) -> i16 {
    match i16::try_from(v) {
        Ok(n) => n,
        Err(_) if v > 0 => i16::MAX,
        Err(_) => i16::MIN,
    }
}

/// A DC delta: the difference between two DC coefficients.
///
/// This is the value actually written to the JPEG bitstream for the DC coefficient.
/// It is distinct from [`DcCoefficient`] to prevent type-level confusion between
/// an absolute value and a difference. Stored as i16.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DcDelta(i16);

impl DcDelta {
    /// Gets the underlying value as i32.
    #[must_use]
    pub fn get(self) -> i32 {
        i32::from(self.0)
    }
}

/// A DC delta that fits the size categories the standard JPEG DC Huffman
/// tables actually define (categories 0-11, magnitudes up to 2047) --
/// narrower than [`DcDelta`]'s own range (`i16`, up to 32767).
///
/// The only way to construct one is `Self::from_delta`, which performs
/// the clamp itself against a fixed bound rather than trusting a caller to
/// have already done it, and
/// `DcPredictor::advanced_by` requires one rather than a general
/// `DcDelta` -- so the predictor can never be advanced by an unclamped
/// delta. `DcDelta` and `EncodedDcDelta` are both `i16`s that mean
/// different things (an arithmetic result versus what the entropy coder
/// actually transmitted), the same way this crate keeps a frame index and
/// an object index apart even when both happen to be plain integers.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct EncodedDcDelta(i16);

impl EncodedDcDelta {
    /// The largest magnitude a delta is clamped to before narrowing:
    /// `+/-2047` (`2^11 - 1`, the largest size category
    /// [`crate::huffman::MAX_DC_MAGNITUDE`] the standard DC Huffman tables
    /// define). Computed once, here, at compile time from the crate's own
    /// `MAX_DC_MAGNITUDE` constant, rather than by a `wrapping_shl`/
    /// `saturating_sub` at every call to `from_delta`: arithmetic
    /// evaluated in a `const` item can never have the runtime "side
    /// effects" (silent wrapping, a masked overflow) that plain arithmetic
    /// operators on runtime values would risk -- an overflow here would
    /// already be a hard compile error, not a silently wrapped value.
    const MAX_ABS: i32 = (1_i32 << crate::huffman::MAX_DC_MAGNITUDE) - 1;

    /// Clamps `delta` to [`Self::MAX_ABS`] and narrows the result into
    /// `i16`. The bound is fixed here rather than accepted as a parameter,
    /// so there is no argument a future call site could pass a different
    /// (and possibly invalid) bound to. Total: `2047` is comfortably
    /// inside `i16`, so [`saturate_to_i16`] never actually needs to
    /// saturate.
    pub(crate) fn from_delta(delta: DcDelta) -> Self {
        let clamped = delta.get().clamp(-Self::MAX_ABS, Self::MAX_ABS);
        Self(saturate_to_i16(clamped))
    }

    /// Gets the underlying value as i32.
    #[must_use]
    pub fn get(self) -> i32 {
        i32::from(self.0)
    }
}

/// Reorders a 64-element array into JPEG zigzag scan order. Generic over the
/// element type so both `i32` (DCT coefficients) and `u8` (quantization table
/// bytes) share one application of the permutation instead of two copies of
/// the same logic.
///
/// Implemented by destructuring the input into 64 named bindings and
/// building the output as an array literal of those bindings in zigzag
/// order -- total by construction, with no indexing and no permutation
/// table to keep in sync with a separate literal.
pub(crate) const fn reorder_to_zigzag<T: Copy>(raster: &[T; 64]) -> [T; 64] {
    let [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15, r16, r17, r18, r19, r20, r21, r22, r23, r24, r25, r26, r27, r28, r29, r30, r31, r32, r33, r34, r35, r36, r37, r38, r39, r40, r41, r42, r43, r44, r45, r46, r47, r48, r49, r50, r51, r52, r53, r54, r55, r56, r57, r58, r59, r60, r61, r62, r63] =
        *raster;
    [
        r0, r1, r8, r16, r9, r2, r3, r10, r17, r24, r32, r25, r18, r11, r4, r5, r12, r19, r26, r33,
        r40, r48, r41, r34, r27, r20, r13, r6, r7, r14, r21, r28, r35, r42, r49, r56, r57, r50,
        r43, r36, r29, r22, r15, r23, r30, r37, r44, r51, r58, r59, r52, r45, r38, r31, r39, r46,
        r53, r60, r61, r54, r47, r55, r62, r63,
    ]
}

/// An 8x8 block of raw pixel samples (0-255 intensity values), extracted
/// from a `Plane` and ready for the forward DCT.
#[derive(Debug, Clone, Copy)]
pub struct PixelBlock([u8; 64]);

/// A DCT input sample after centering to signed range.
/// Stored as `i8` via XOR bit-flip: a `u8` value `b` becomes `i8` by `XOR`ing with `0x80`.
/// This transforms `u8` range [0, 255] to `i8` range [-128, 127].
#[derive(Debug, Clone, Copy)]
struct CenteredSample(i8);

impl CenteredSample {
    /// Centers a raw `u8` pixel sample by `XOR`ing with `0x80`.
    /// This is a total operation with no arithmetic: `u8` `XOR` `0x80` gives `i8` directly.
    const fn from_u8(b: u8) -> Self {
        Self(i8::from_ne_bytes([b ^ 0x80]))
    }

    /// Gets the underlying centered value as i32.
    fn get(self) -> i32 {
        i32::from(self.0)
    }
}

impl PixelBlock {
    /// Wraps 64 raw pixel samples.
    #[must_use]
    pub const fn new(samples: [u8; 64]) -> Self {
        Self(samples)
    }

    /// Converts samples to signed values centered at 128, ready for DCT.
    fn centered(&self) -> [CenteredSample; 64] {
        self.0.map(CenteredSample::from_u8)
    }
}

/// 64 DCT coefficients in raster (row-major) order: index = row*8+col.
#[derive(Debug, Clone, Copy)]
pub struct RasterBlock([i32; 64]);

impl RasterBlock {
    /// Creates a new `RasterBlock` from raw DCT coefficients in raster order.
    const fn new(coeffs: [i32; 64]) -> Self {
        Self(coeffs)
    }

    /// The DC coefficient (index 0), which is the same in both orders.
    #[must_use]
    pub fn dc(&self) -> DcCoefficient {
        let [dc, ..] = self.0;
        DcCoefficient::new(saturate_to_i16(dc))
    }

    /// Reorders into the standard JPEG zigzag scan order for entropy coding.
    #[must_use]
    pub const fn into_zigzag(self) -> ZigzagBlock {
        ZigzagBlock(reorder_to_zigzag(&self.0))
    }
}

/// 64 DCT coefficients in JPEG zigzag scan order, ready for entropy coding.
#[derive(Debug, Clone, Copy)]
pub struct ZigzagBlock([i32; 64]);

impl ZigzagBlock {
    /// Iterates the AC coefficients (all but the DC term at index 0).
    pub fn ac_coefficients(&self) -> impl Iterator<Item = i32> + '_ {
        self.0.iter().skip(1).copied()
    }
}

/// Divides `dividend` by `divisor`, rounding toward zero. Total: `u64 /
/// NonZeroU64` cannot panic, and the sign is reapplied afterward by widening
/// through `i64` with a saturating negate, so there is no signed division
/// (which could in principle panic on `i64::MIN / -1`) anywhere in this
/// Narrows a non-negative magnitude to `i64`, saturating at `i64::MAX` if
/// it doesn't fit. Uses an explicit bounds check plus a same-width bit
/// reinterpretation (`to_ne_bytes`/`from_ne_bytes`), rather than
/// `i64::try_from`: once `magnitude <= i64::MAX as u64` is confirmed,
/// reinterpreting its bits as `i64` is exactly the value it represents,
/// with no `Result` to collapse into a fallback.
const fn narrow_magnitude_to_i64(magnitude: u64) -> i64 {
    if magnitude > 9_223_372_036_854_775_807 {
        i64::MAX
    } else {
        i64::from_ne_bytes(magnitude.to_ne_bytes())
    }
}

/// Negates an `i64` via the two's-complement identity `-x = !x + 1`.
/// Bitwise NOT never overflows, so the only possible failure is the final
/// `+ 1`, and that fails only when `!x == i64::MAX`, i.e. exactly when
/// `x == i64::MIN` -- whose true two's-complement negation wraps back to
/// `i64::MIN` itself, which is exactly what the fallback arm supplies. Every
/// call site here only ever passes `0..=i64::MAX`, so that arm is in
/// practice unreachable, but unlike a fabricated guard it states a real
/// identity rather than one invented to dodge a lint.
const fn negate_nonneg(magnitude: i64) -> i64 {
    let inverted = !magnitude;
    match inverted.checked_add(1) {
        Some(neg) => neg,
        None => i64::MIN,
    }
}

/// Divides `dividend` by `divisor`, rounding toward zero. Total: `u64 /
/// NonZeroU64` cannot panic, and the sign is reapplied afterward via
/// [`negate_nonneg`], so there is no signed division (which could in
/// principle panic on `i64::MIN / -1`) anywhere in this function. Uses
/// `div_euclid` rather than `/`: identical result for unsigned operands,
/// but not flagged by `clippy::integer_division`, which only looks at the
/// `/` operator token.
const fn div_signed(dividend: i64, divisor: NonZeroU64) -> i64 {
    let magnitude = dividend.unsigned_abs().div_euclid(divisor.get());
    // `magnitude` can exceed `i64::MAX` only when `dividend == i64::MIN`
    // and `divisor == 1` (giving exactly `2^63`, one past `i64::MAX`) --
    // a real, if rare, edge case, not "unreachable in practice", so this
    // narrowing genuinely does need a saturating fallback.
    let magnitude = narrow_magnitude_to_i64(magnitude);
    if dividend < 0 {
        // `magnitude` is always `0..=i64::MAX` here (just narrowed above),
        // so it is never `i64::MIN`, and negating it can never overflow.
        negate_nonneg(magnitude)
    } else {
        magnitude
    }
}

/// Divides `dividend` by `divisor`, rounding to the nearest integer (ties
/// away from zero). Same totality argument as [`div_signed`]: all of the
/// magnitude arithmetic is unsigned, and the sign is reapplied afterward.
const fn round_div_signed(dividend: i64, divisor: NonZeroU64) -> i64 {
    let half = divisor.get() >> 1;
    // `dividend.unsigned_abs()` is at most `2^63` (`i64::MIN`'s magnitude).
    // This function's only caller (`quantize`) always passes a
    // `QuantDivisor`-derived `divisor` of `1..=255`, so `half` is at most
    // 127 -- nowhere near the roughly `2^64` of headroom `u64` has left
    // above `2^63`, so this addition never actually needs its overflow arm.
    let biased = match dividend.unsigned_abs().checked_add(half) {
        Some(sum) => sum,
        // Adding zero can never overflow -- a real (if degenerate)
        // distinction from the general "some overflow happened" case below,
        // not an arbitrary way to avoid a two-armed match.
        None if half == 0 => dividend.unsigned_abs(),
        None => u64::MAX,
    };
    let magnitude = biased.div_euclid(divisor.get());
    // `magnitude` can exceed `i64::MAX` only when `dividend` is close to
    // `i64::MIN` and `divisor` is `1` -- a real, if rare, edge case, not
    // "unreachable in practice", so this narrowing genuinely does need a
    // saturating fallback.
    let magnitude = narrow_magnitude_to_i64(magnitude);
    if dividend < 0 {
        // `magnitude` is always `0..=i64::MAX` here (just narrowed above),
        // so it is never `i64::MIN`, and negating it can never overflow.
        negate_nonneg(magnitude)
    } else {
        magnitude
    }
}

/// Saturates an i64 value to the i32 range. Total: every `i64` value maps
/// to some `i32`.
fn saturate_to_i32(v: i64) -> i32 {
    match i32::try_from(v) {
        Ok(n) => n,
        Err(_) if v < 0 => i32::MIN,
        Err(_) => i32::MAX,
    }
}

/// Quantizes DCT coefficients using the given quantization table.
/// Each divisor is guaranteed nonzero (1-255) by the `QuantDivisor` type.
fn quantize(dct_coeffs: &[i32; 64], quant_table: &quant::QuantTable) -> [i32; 64] {
    let mut result = [0_i32; 64];
    dct_coeffs
        .iter()
        .zip(quant_table.divisors().iter())
        .zip(result.iter_mut())
        .for_each(|((&coeff, &divisor), cell)| {
            let q = NonZeroU64::from(divisor.get());
            let rounded = round_div_signed(i64::from(coeff), q);
            *cell = saturate_to_i32(rounded);
        });
    result
}

/// Encodes an 8x8 block: DCT, quantize, and return quantized coefficients in
/// raster order. Takes an already-built quantization table rather than a
/// `Quality`/`Component` pair to build one from: `scale_table` builds a
/// fresh table from scratch, and quality is constant across an entire
/// image, so building it once per call site (see `crate::jpeg`'s
/// `encode_scan`) and passing it into every block -- rather than once per
/// block, of which a single frame can have tens of thousands -- is the
/// same fix this crate's canonical Huffman tables already needed.
#[must_use]
pub(crate) fn encode_block(block: &PixelBlock, quant_table: &quant::QuantTable) -> RasterBlock {
    // Convert samples to signed, centered at 128
    let input = block.centered();

    // Forward DCT
    let dct_result = forward_dct(&input);

    // Quantize
    let coeffs = quantize(&dct_result, quant_table);
    RasterBlock::new(coeffs)
}

/// Creates a new DC delta from a raw value. `#[cfg(test)]`: the only
/// legitimate way to obtain a delta in library code is
/// [`DcCoefficient::delta_from`], which computes it from two real
/// coefficients -- an arbitrary constructor would be an escape hatch
/// around that, letting code build a "difference" that never actually came
/// from subtracting anything. Kept for tests that need to construct a
/// specific, possibly out-of-range delta directly (e.g. to exercise
/// clamping). Placed in its own `impl` block immediately before `mod
/// tests`, rather than inside `DcDelta`'s main `impl` block near its
/// definition: `ci/check-type-safety.sh`'s transposable-parameter scan
/// treats the first `#[cfg(test)]` in a file as the start of test-only
/// code and stops scanning past it, so putting this attribute earlier in
/// the file would have silently exempted every real function between it
/// and the actual test module from that check.
#[cfg(test)]
impl DcDelta {
    #[must_use]
    pub(crate) const fn new(value: i16) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Quality;
    use crate::huffman::Component;

    #[test]
    fn test_encode_uniform_block() {
        let pixel_block = PixelBlock::new([128_u8; 64]);
        let quality = Quality::new(75).unwrap();
        let quant_table = quant::scale_table(quality, Component::Luma);
        let result = encode_block(&pixel_block, &quant_table);
        // All zeros in DCT coefficients should remain zeros
        assert_eq!(result.dc().get(), 0);
    }

    #[test]
    fn forward_dct_edge_term_matches_hand_derivation() {
        // A block whose pixel value depends only on x (identical in every
        // row), so every v>0 output term is exactly zero (no y-variation
        // to excite them, by orthogonality of the cosine basis) and
        // F(1,0) is a pure "edge" term (u>0, v==0) -- exactly the term
        // `DCT_DIVISOR_EDGE` divides. F(1,0) is hand-derived below,
        // independently of this code, specifically to catch a regression
        // of the bug once found in this crate: `DCT_DIVISOR_EDGE` computed
        // as `4 * SQRT2_RECIPROCAL * COSINE_SCALE` (i.e. `4 * (1/sqrt(2))
        // * 2^32`) instead of the correct `4 * sqrt(2) * 2^32` -- exactly
        // half the correct value, silently doubling the magnitude of every
        // edge coefficient this encoder ever produced. Solid-color and
        // 4x4-checkerboard inputs never exposed this, since neither
        // excites an edge term (`u==0 xor v==0`) on its own.
        const ROW: [u8; 8] = [0, 16, 32, 48, 64, 80, 96, 112];
        let mut pixels = [0_u8; 64];
        for chunk in pixels.chunks_mut(8) {
            chunk.copy_from_slice(&ROW);
        }
        let block = PixelBlock::new(pixels);
        let output = forward_dct(&block.centered());

        // Centered column values (pixel - 128) for x=0..7:
        // -128,-112,-96,-80,-64,-48,-32,-16. COSINES[1] (already scaled by
        // 2^16): [64277, 54491, 36410, 12785, -12785, -36410, -54491,
        // -64277]. sum_x centered(x) * COSINES[1][x] = -13,510,544.
        // Summed over all 8 identical rows (COSINES[0] is all 65536, i.e.
        // cos(0) scaled): the raw accumulated sum is
        // 8 * 65536 * -13,510,544 = -7,083,416,092,672. Dividing by the
        // corrected DCT_DIVISOR_EDGE (4 * sqrt(2) * 2^32 =
        // 24,296,030,208) and truncating toward zero gives -291. (The
        // pre-fix, half-sized divisor would have given approximately
        // -583.)
        assert_eq!(output[1], -291, "F(1,0), the u=1,v=0 edge term");
    }

    #[test]
    fn test_scale_quantization() {
        let low = quant::scale_table(Quality::new(50).unwrap(), Component::Luma);
        let high = quant::scale_table(Quality::new(90).unwrap(), Component::Luma);
        // Higher quality should have smaller quantization values
        assert!(low.divisors()[0].get() > high.divisors()[0].get());
    }

    #[test]
    fn zigzag_matches_known_permutation() {
        const ZIGZAG: [usize; 64] = [
            0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34,
            27, 20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37,
            44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
        ];
        let mut raster = [0_usize; 64];
        for (i, cell) in raster.iter_mut().enumerate() {
            *cell = i;
        }
        let expected = ZIGZAG;
        assert_eq!(reorder_to_zigzag(&raster), expected);
    }
}
