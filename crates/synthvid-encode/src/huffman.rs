//! Huffman table definitions and encoding.

use crate::bitstream::{BitCount, BitPattern, BitstreamWriter};
use crate::dct::{DcDelta, EncodedDcDelta};
use core::num::{NonZeroI16, NonZeroI32};

mod tables;
mod types;
pub use tables::{
    get_ac_chroma_bits, get_ac_chroma_values, get_ac_luma_bits, get_ac_luma_values,
    get_dc_chroma_bits, get_dc_chroma_values, get_dc_luma_bits, get_dc_luma_values,
};

/// The destination index of a Huffman or quantization table: `0` for luma,
/// `1` for chroma. Derived from [`Component`] alone.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct TableDestination(pub(crate) u8);
pub use types::{Codeword, HuffmanBits, HuffmanValues, SizeCategory};
use types::{SignPattern, SignedValue};

/// A count of preceding zero coefficients before a nonzero AC coefficient (0-15).
///
/// The JPEG symbol byte packs this into the high nibble (`run << 4`), so a value outside
/// this range would silently wrap and corrupt the entropy-coded stream.
///
/// `pub(crate)`: this and [`AcSymbol`]/[`encode_ac`] are only ever meant to be
/// driven by [`crate::ac_encoding::encode_ac_coefficients`]'s run-length
/// bookkeeping, which is the only code that knows whether a `Zrl` is
/// actually valid to emit (a `Zrl` is only correct when a nonzero
/// coefficient follows it -- something only that bookkeeping tracks). If
/// these were public, any external caller could build and emit `AcSymbol`s
/// directly, out of order, producing a self-consistent but corrupted
/// bitstream with no compiler error -- the exact "one public escape hatch"
/// risk this crate otherwise avoids.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct ZeroRun(u8);

impl ZeroRun {
    /// Gets the underlying count.
    #[must_use]
    pub(crate) const fn get(self) -> u8 {
        self.0
    }

    /// Builds a `ZeroRun` from the low 4 bits of `value` (`value & 15`),
    /// which is always `0..=15` by construction of the mask. Total, with
    /// no `Option` to unwrap -- unlike [`Self::new`]'s general validation,
    /// which must reject an out-of-range input rather than only ever
    /// seeing one that already fits.
    #[must_use]
    pub(crate) const fn from_low_nibble(value: u8) -> Self {
        Self(value & 15)
    }
}

/// An AC coefficient symbol for entropy coding.
///
/// Either a control symbol (end of block, zero run length) or a real nonzero coefficient with
/// its preceding run of zeros. Making these three cases distinct variants -- instead of
/// overloading `(run: u8, value: i32)` by sentinel values -- means the one combination that
/// used to silently write nothing (`run` outside `{0, 15}` with `value == 0`) can no longer be
/// constructed at all.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum AcSymbol {
    /// End of block: no more nonzero coefficients in this block.
    Eob,
    /// Zero run length: 16 zero coefficients in a row (run resets to 0 after this symbol, more
    /// zeros may follow).
    Zrl,
    /// A nonzero coefficient, preceded by `run` zero coefficients.
    Coefficient {
        /// Number of preceding zero coefficients (0-15).
        run: ZeroRun,
        /// The nonzero coefficient value.
        value: AcCoefficient,
    },
}

/// A nonzero AC coefficient, already clamped to the magnitude range the
/// standard AC Huffman tables can represent (size categories 1-10,
/// magnitudes up to 1023).
///
/// `AcSymbol::Coefficient` requires this instead of a bare `i32` so a zero
/// value cannot be constructed at all: a zero value combined with a zero
/// run would otherwise produce the exact same symbol byte as EOB (`0x00`),
/// silently miscoding the stream rather than being folded into the run
/// length as a real zero coefficient should be.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct AcCoefficient(NonZeroI16);

impl AcCoefficient {
    /// Clamps `value` to the AC magnitude range and wraps it, or returns
    /// `None` if `value` is zero -- a zero coefficient has no `AcSymbol`
    /// representation of its own; it is folded into the preceding
    /// `ZeroRun` instead. Backed by a real `NonZeroI16`, not a plain
    /// `i32` a caller trusts is nonzero: zero is unrepresentable at the
    /// bit level, not merely refused by this constructor.
    #[must_use]
    pub fn new(value: i32) -> Option<Self> {
        if value == 0_i32 {
            return None;
        }
        // `clamp_to_magnitude` cannot cross zero: it only shrinks a
        // nonzero value's magnitude toward the bound, never past it, so
        // `clamped` is nonzero whenever `value` is. `i16::try_from`'s
        // fallback is unreachable in practice, since |clamped| <= 1023
        // already fits i16 -- but total either way.
        let SignedValue(clamped) = clamp_to_magnitude(SignedValue(value), MAX_AC_MAGNITUDE);
        let narrowed = match i16::try_from(clamped) {
            Ok(fits) => fits,
            Err(_) if clamped > 0_i32 => i16::MAX,
            Err(_) => i16::MIN,
        };
        NonZeroI16::new(narrowed).map(Self)
    }

    /// Gets the underlying value.
    #[must_use]
    pub fn get(self) -> i32 {
        NonZeroI32::from(self.0).get()
    }
}

/// Component type for Huffman tables.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Component {
    /// Luma component (Y).
    Luma,
    /// Chroma component (Cb or Cr).
    Chroma,
}

impl Component {
    /// The quantization/DC-Huffman/AC-Huffman table destination index this
    /// component's content selector maps to: `0` for luma, `1` for chroma.
    /// A single derivation, since all three destinations always agree.
    #[must_use]
    pub(crate) const fn table_dest(self) -> TableDestination {
        match self {
            Self::Luma => TableDestination(0),
            Self::Chroma => TableDestination(1),
        }
    }
}

/// Huffman code for a value.
#[derive(Debug, Clone, Copy)]
pub struct HuffmanCode {
    /// The code value.
    code: Codeword,
    /// The number of bits in the code, guaranteed `1..=16` by `new`'s
    /// validation -- stored as a `BitCount` so reading it back can never
    /// fail, instead of re-validating a bare `u8` every time it's needed.
    bits: BitCount,
}

impl HuffmanCode {
    /// Creates a new `HuffmanCode`.
    ///
    /// # Arguments
    /// * `code` - The code value
    /// * `bits` - The number of bits. JPEG limits Huffman codes to 1-16 bits;
    ///   values exceeding this are rejected even though `BitCount` permits 1-32.
    ///
    /// # Returns
    /// `Some(HuffmanCode)` if parameters are valid, `None` otherwise.
    #[must_use]
    pub const fn new(code: Codeword, bits: BitCount) -> Option<Self> {
        // JPEG (T.81) limits Huffman codes to 16 bits maximum
        if bits.get() > 16 {
            return None;
        }
        // The largest code representable in `bits.get()` bits (`2^bits -
        // 1`). `bits.get()` is `1..=16` here (checked above, and never
        // zero per `BitCount`'s own `NonZero` invariant), a small closed
        // set, so this is a total lookup rather than a `checked_shl`/
        // `checked_sub` whose "out of range" arm would need a fallback
        // that can't actually be reached from this call site anyway.
        let max_val: u16 = match bits.get() {
            1 => 0x0001,
            2 => 0x0003,
            3 => 0x0007,
            4 => 0x000F,
            5 => 0x001F,
            6 => 0x003F,
            7 => 0x007F,
            8 => 0x00FF,
            9 => 0x01FF,
            10 => 0x03FF,
            11 => 0x07FF,
            12 => 0x0FFF,
            13 => 0x1FFF,
            14 => 0x3FFF,
            15 => 0x7FFF,
            _ => 0xFFFF,
        };
        if code.0 > max_val {
            return None;
        }
        Some(Self { code, bits })
    }

    /// Gets the code value.
    #[must_use]
    pub const fn code(self) -> Codeword {
        self.code
    }

    /// This code's length as a `BitCount`.
    #[must_use]
    pub const fn bit_count(self) -> BitCount {
        self.bits
    }
}

/// The largest DC size category the standard Huffman tables define (0-11).
///
/// A DC delta whose magnitude needs more than 11 bits has no symbol in
/// either standard DC table. This is not purely theoretical: this crate's
/// simplified DCT can produce a raw DC term up to roughly `average * 8` for
/// an 8-bit-per-sample block (average centered at 0, so up to `±1024`), and
/// the DC *delta* encoded here is the difference between two such blocks --
/// up to roughly `±2048`, which needs 12 bits. Silently dropping that
/// symbol (the previous behavior) desyncs the entropy-coded stream with no
/// error; clamping the value before encoding keeps the bitstream valid.
///
/// This constant is verified against the actual Huffman table values in tests
/// to ensure it never silently drifts if the tables change. `pub(crate)` so
/// `dct::EncodedDcDelta`'s constructor can clamp to this exact bound
/// directly, rather than accepting an arbitrary bound as a parameter that a
/// future call site could pass something else to.
pub(crate) const MAX_DC_MAGNITUDE: SizeCategory = SizeCategory(11);

/// The largest AC size category the standard Huffman tables define (1-10).
/// See [`MAX_DC_MAGNITUDE`] for why a coefficient can plausibly exceed this.
///
/// This constant is verified against the actual Huffman table values in tests
/// to ensure it never silently drifts if the tables change.
const MAX_AC_MAGNITUDE: SizeCategory = SizeCategory(10);

/// The `(-max_abs, max_abs)` clamp bounds for a magnitude category, where
/// `max_abs = 2^max_magnitude - 1`.
///
/// A match over the small set of JPEG magnitude categories this crate ever
/// actually clamps to (`0..=11`: [`MAX_AC_MAGNITUDE`]'s 10 and
/// [`MAX_DC_MAGNITUDE`]'s 11, with a little headroom), rather than a
/// runtime shift/subtract/negate: there is no arithmetic operator here
/// that could overflow, only a lookup, so there is no `None` case to
/// prove unreachable in the first place.
const fn magnitude_bounds(max_magnitude: SizeCategory) -> (SignedValue, SignedValue) {
    let (low, high) = match max_magnitude.0 {
        0 => (0_i32, 0_i32),
        1 => (-1_i32, 1_i32),
        2 => (-3_i32, 3_i32),
        3 => (-7_i32, 7_i32),
        4 => (-15_i32, 15_i32),
        5 => (-31_i32, 31_i32),
        6 => (-63_i32, 63_i32),
        7 => (-127_i32, 127_i32),
        8 => (-255_i32, 255_i32),
        9 => (-511_i32, 511_i32),
        10 => (-1_023_i32, 1_023_i32),
        11 => (-2_047_i32, 2_047_i32),
        _ => (i32::MIN, i32::MAX),
    };
    (SignedValue(low), SignedValue(high))
}

/// Clamps `value` so `encode_signed_value` never reports a magnitude
/// greater than `max_magnitude`, i.e. so it always fits a category the
/// standard Huffman tables actually define.
fn clamp_to_magnitude(value: SignedValue, max_magnitude: SizeCategory) -> SignedValue {
    let (SignedValue(neg_max_abs), SignedValue(max_abs)) = magnitude_bounds(max_magnitude);
    SignedValue(value.0.clamp(neg_max_abs, max_abs))
}

/// Encodes a DC delta, returning the actually-written (possibly-clamped) delta.
#[must_use]
pub(crate) fn encode_dc(
    value: DcDelta,
    writer: &mut BitstreamWriter,
    component: Component,
) -> EncodedDcDelta {
    let clamped = EncodedDcDelta::from_delta(value);
    let bits = encode_signed_value(SignedValue(clamped.get()));

    let table = match component {
        Component::Chroma => tables::get_dc_chroma_table(),
        Component::Luma => tables::get_dc_luma_table(),
    };

    // `magnitude` is now guaranteed <= MAX_DC_MAGNITUDE (11) by the clamp
    // above, so this lookup can never miss a symbol the table defines.
    if let Some(Some(huffman_code)) = table.get(usize::from(bits.magnitude().0)) {
        writer.write_bits(huffman_code.code().bits(), huffman_code.bit_count());

        if let Some((bit_count, pattern)) = bits.sign_bits {
            writer.write_bits(BitPattern(pattern.0), bit_count);
        }
    }
    clamped
}

/// Encodes an AC coefficient symbol with run-length encoding.
pub(crate) fn encode_ac(symbol: AcSymbol, writer: &mut BitstreamWriter, component: Component) {
    let table = match component {
        Component::Chroma => tables::get_ac_chroma_table(),
        Component::Luma => tables::get_ac_luma_table(),
    };

    let (ac_symbol_byte, sign_bits_info) = match symbol {
        AcSymbol::Eob => (0x00_u8, None),
        AcSymbol::Zrl => (0xF0_u8, None),
        AcSymbol::Coefficient { run, value } => {
            let bits = encode_signed_value(SignedValue(value.get()));
            // `bits.magnitude()` is at most 10 in practice, since `value`
            // came from `AcCoefficient::new`'s own clamp -- but the packing
            // below only has 4 bits (0..=15) to put it in regardless of
            // that upstream guarantee, so `.min(15)` defends the packing's
            // own invariant directly, rather than depending on staying in
            // sync with whatever bound `AcCoefficient` happens to use.
            // Without it, a magnitude of 16 or more would spill into the
            // `run` nibble, corrupting the symbol into a different one
            // instead of just being wrong.
            let SizeCategory(size) = bits.magnitude().min(SizeCategory(15));
            ((run.get() << 4_i32) | size, bits.sign_bits)
        }
    };

    if let Some(Some(huffman_code)) = table.get(usize::from(ac_symbol_byte)) {
        writer.write_bits(huffman_code.code().bits(), huffman_code.bit_count());

        if let Some((bit_count, pattern)) = sign_bits_info {
            writer.write_bits(BitPattern(pattern.0), bit_count);
        }
    }
}

/// The result of encoding a signed value's magnitude and sign bits for
/// JPEG entropy coding.
///
/// Stores only `sign_bits`, not a separate `magnitude` field: `magnitude`
/// is always exactly `0` when `sign_bits` is `None` and always exactly the
/// `BitCount` inside `Some` otherwise, so storing both would be the same
/// fact twice, with two chances to drift out of sync after an edit.
/// [`Self::magnitude`] derives it from the one field that actually varies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SignedValueBits {
    /// The sign-bit pattern to write after the Huffman code, along
    /// with how many bits of it to write -- `None` when the magnitude
    /// category is 0, meaning there are no sign bits to write at all.
    /// Bundling this as `Option<(BitCount, SignPattern)>` means callers no longer
    /// need to separately re-check the magnitude and re-derive a
    /// `BitCount` from it -- this type already knows both facts.
    sign_bits: Option<(BitCount, SignPattern)>,
}

impl SignedValueBits {
    /// The magnitude category (0-32): how many bits are needed to
    /// represent this value's absolute value, used to select a Huffman
    /// symbol. Derived from `sign_bits` rather than stored separately.
    /// Held as a `u8` (via `BitCount::get_u8`'s total narrowing, not a
    /// fallible conversion a caller has to handle) since a Huffman symbol
    /// byte is always a `u8` anyway.
    const fn magnitude(self) -> SizeCategory {
        match self.sign_bits {
            Some((bit_count, _)) => SizeCategory(bit_count.get_u8()),
            None => SizeCategory(0),
        }
    }
}

/// Encodes a signed value as its magnitude category and sign bit pattern.
#[must_use]
const fn encode_signed_value(signed_value: SignedValue) -> SignedValueBits {
    let SignedValue(value) = signed_value;
    let abs_val = value.unsigned_abs();

    // Count bits needed
    let magnitude = if abs_val == 0 {
        0
    } else {
        // `abs_val != 0` here, so `leading_zeros()` is `0..=31` (`u32`'s
        // `leading_zeros` only ever reaches 32 for a zero value, which
        // the outer branch already excludes) -- so this subtraction never
        // actually needs its `None` arm.
        match 32_u32.checked_sub(abs_val.leading_zeros()) {
            Some(bits) => bits,
            None => 0,
        }
    };

    let sign_bits = if value >= 0_i32 {
        abs_val
    } else {
        // T.81 F.1.2.1.3 EXTEND procedure: the appended bits for a negative
        // value V are the low S bits of (V - 1), which for V < 0 equals
        // |V| XOR mask (where mask = (1 << S) - 1). A previous version of
        // this code incorrectly computed (|V| - 1) XOR mask instead --
        // subtracting 1 from the wrong operand -- which silently corrupted
        // the sign/magnitude of every negative coefficient ever encoded
        // (a full sign flip whenever |V| was a power of two, otherwise an
        // off-by-one error toward zero).
        //
        // `magnitude` is `1..=32` here (`value < 0` means `abs_val != 0`,
        // so `magnitude` took the branch above). `1 << 32` would be a
        // genuine shift-amount overflow for a `u32`, so 32 is handled
        // directly rather than by shifting; for `magnitude < 32`,
        // `1 << magnitude` is a power of two and therefore never zero, so
        // the following `checked_sub(1)` never actually needs its `None`
        // arm either.
        let mask = if magnitude == 32 {
            u32::MAX
        } else {
            match (1_u32 << magnitude).checked_sub(1) {
                Some(ones) => ones,
                None => 0,
            }
        };
        abs_val ^ mask
    };

    let sign_bits_option = if magnitude > 0 {
        // magnitude > 0 and magnitude <= 32 here, so BitCount::new always succeeds:
        // magnitude is computed as 32 - leading_zeros() when abs_val != 0, which
        // gives a value in the range [1, 32].
        match BitCount::new(magnitude) {
            Some(bit_count) => Some((bit_count, SignPattern(sign_bits))),
            None => None, // This branch is unreachable due to the conditions above
        }
    } else {
        None
    };

    SignedValueBits {
        sign_bits: sign_bits_option,
    }
}

/// `ZERO`, `new`, and `advance` are only used by
/// `ac_encoding::tests::encode_ac_coefficients_pre_fix_buggy`, which
/// deliberately re-implements the pre-fix (buggy) run-length loop to prove
/// the real fix changes behavior. Kept in their own `#[cfg(test)]` impl
/// block immediately before `mod tests`, rather than inside `ZeroRun`'s
/// main impl block near its definition: `ci/check-type-safety.sh` treats
/// the first `#[cfg(test)]` in a file as the start of test-only code and
/// stops scanning production code past it, so putting this attribute
/// earlier in the file would have silently exempted every real function
/// between it and the actual test module from that check.
#[cfg(test)]
impl ZeroRun {
    /// A zero run count of zero.
    pub(crate) const ZERO: Self = Self(0);

    /// Creates a `ZeroRun`, or `None` if `run` exceeds 15.
    #[must_use]
    pub(crate) const fn new(run: u8) -> Option<Self> {
        if run <= 15 {
            Some(Self(run))
        } else {
            None
        }
    }

    /// The next run count after one more zero coefficient, or `None` if
    /// that would exceed 15 (the caller must emit a ZRL symbol and
    /// start a new run instead).
    #[must_use]
    pub(crate) const fn advance(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(next) => Self::new(next),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests;
