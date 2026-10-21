//! Bitstream writing for JPEG data.

use core::num::NonZeroU8;

#[path = "bitstream_types.rs"]
mod bitstream_types;

pub use bitstream_types::{BitPattern, ByteBuffer};
use bitstream_types::{ByteFill, PartialByte};

/// A single bit value, named so that a call site cannot confuse one bit
/// state with the other.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Bit {
    /// The bit `0`.
    Zero,
    /// The bit `1`.
    One,
}

/// Number of bits to write (1-32).
///
/// Stored as `NonZeroU8`, not `u32`: the valid range (1..=32) always fits
/// `u8`, so [`Self::get_u8`] is a plain field read rather than a
/// `u32::try_from` that must handle a theoretical failure case that can
/// never actually occur once `new` has validated the value -- and the
/// lower bound (never zero) is carried by the type itself rather than
/// left to `new`'s check alone, the same way `QuantDivisor` (in
/// `quant.rs`) uses `NonZeroU8` for its own "never zero" divisor.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BitCount(NonZeroU8);

impl BitCount {
    /// Creates a new `BitCount`.
    ///
    /// # Arguments
    /// * `value` - Number of bits (1-32)
    ///
    /// # Returns
    /// `Some(BitCount)` if value is in range, `None` otherwise.
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        if value < 1 || value > 32 {
            return None;
        }
        // `value` is now known to be `1..=32`, which always fits in one
        // byte, so reading the low byte out via `to_le_bytes` (rather than
        // `u8::try_from`, which is not yet const-callable on this
        // toolchain -- `TryFrom`/`From` are not stable as const traits
        // here) is a lossless narrowing. `NonZeroU8::new` on it is total
        // for the same reason: `value` is also never zero here.
        let [lo, ..] = value.to_le_bytes();
        match NonZeroU8::new(lo) {
            Some(nz) => Some(Self(nz)),
            None => None,
        }
    }

    /// Gets the bit count value. Total: a single byte zero-extended into a
    /// `u32` can never lose information. Built with `from_le_bytes`
    /// (const-stable) rather than `u32::from` (a `From` trait method,
    /// which is not yet const-callable on this toolchain), so this can
    /// stay a `const fn`.
    #[must_use]
    pub const fn get(self) -> u32 {
        u32::from_le_bytes([self.0.get(), 0, 0, 0])
    }

    /// This count narrowed to `u8`. Total and needs no conversion at all,
    /// since the value is already stored as one.
    #[must_use]
    pub const fn get_u8(self) -> u8 {
        self.0.get()
    }
}

/// Number of bits already written into the current output byte: always
/// exactly one of these 8 states (`BitstreamWriter::write_bit` resets to
/// `B0` the instant an advance would go past `B7`). An enum, not a bare
/// `u8` whose range only a comment states and whose increment needs
/// arithmetic that could in principle overflow: [`Self::advance`] is a
/// total match over exactly 8 cases, with no `+`/`-` operator anywhere in
/// this type.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum BitPos {
    /// No bits written into the current byte yet.
    B0,
    /// 1 bit written.
    B1,
    /// 2 bits written.
    B2,
    /// 3 bits written.
    B3,
    /// 4 bits written.
    B4,
    /// 5 bits written.
    B5,
    /// 6 bits written.
    B6,
    /// 7 bits written; one more completes the byte.
    B7,
}

impl BitPos {
    /// No bits written yet.
    const ZERO: Self = Self::B0;

    /// One more bit was just written. Returns the new position, and
    /// whether the current byte is now full (was `B7`, wrapping back to
    /// `B0`).
    const fn advance(self) -> (Self, ByteFill) {
        match self {
            Self::B0 => (Self::B1, ByteFill::Partial),
            Self::B1 => (Self::B2, ByteFill::Partial),
            Self::B2 => (Self::B3, ByteFill::Partial),
            Self::B3 => (Self::B4, ByteFill::Partial),
            Self::B4 => (Self::B5, ByteFill::Partial),
            Self::B5 => (Self::B6, ByteFill::Partial),
            Self::B6 => (Self::B7, ByteFill::Partial),
            Self::B7 => (Self::B0, ByteFill::Full),
        }
    }
}

/// A writer for bits to a byte buffer.
#[derive(Debug, Clone)]
pub struct BitstreamWriter {
    /// Output buffer.
    buffer: ByteBuffer,
    /// Current byte being built, bit-shifted in from the low end as each
    /// new bit arrives.
    current_byte: PartialByte,
    /// Number of bits used in `current_byte`.
    bit_pos: BitPos,
}

impl BitstreamWriter {
    /// Creates a new empty bitstream writer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buffer: ByteBuffer(Vec::new()),
            current_byte: PartialByte(0),
            bit_pos: BitPos::ZERO,
        }
    }

    /// Writes bits from `value` to the bitstream.
    ///
    /// The most significant bits are written first.
    ///
    /// Implemented one bit at a time, rather than extracting and shifting
    /// a multi-bit group per iteration: every quantity involved (a single
    /// bit, and a bit position that never exceeds 8) is then trivially
    /// bounded well within its type's range, with no masking arithmetic
    /// that could in principle overflow or need a fallible narrowing
    /// conversion to justify.
    pub fn write_bits(&mut self, value: BitPattern, n_bits: BitCount) {
        let BitPattern(pattern) = value;
        let bit_count = n_bits.get();
        // `position` ranges over `0..bit_count`, and `bit_count` is at most 32, so
        // shifting the 32-bit `pattern` right by `position` is always in range
        // (shifting a `u32` by 32 or more is what panics; `position` never
        // reaches `bit_count` itself, let alone 32).
        for position in (0..bit_count).rev() {
            let bit = if (pattern >> position) & 1 == 1 {
                Bit::One
            } else {
                Bit::Zero
            };
            self.write_bit(bit);
        }
    }

    /// Writes a single bit, most-significant-bit-first within the current
    /// byte, flushing that byte once it fills.
    fn write_bit(&mut self, bit: Bit) {
        // The match is total over both bit values, so there is no fallible
        // case to handle here at all.
        let shifted = self.current_byte.0 << 1_i32;
        self.current_byte = PartialByte(match bit {
            Bit::Zero => shifted,
            Bit::One => shifted | 1,
        });
        let (next, fill) = self.bit_pos.advance();
        self.bit_pos = next;

        if fill == ByteFill::Full {
            self.flush_byte();
        }
    }

    /// Flushes the current byte to the buffer, handling JPEG byte stuffing.
    /// Only called right after [`Self::write_bit`] observes the byte just
    /// became full, so there is no separate "is it actually full" check
    /// here to get out of sync with that call site.
    fn flush_byte(&mut self) {
        let PartialByte(byte) = self.current_byte;
        self.buffer.0.push(byte);

        // Byte stuffing: if we wrote 0xFF, add 0x00
        if byte == 0xFF {
            self.buffer.0.push(0x00);
        }

        self.current_byte = PartialByte(0);
        self.bit_pos = BitPos::ZERO;
    }

    /// Pads the current byte with ones (per JPEG spec T.81 F.1.2.3, which
    /// requires trailing padding bits to be 1s -- decoders are required to
    /// ignore this padding regardless, so this was previously harmless as
    /// zeros, but this is the spec-correct value) and flushes it.
    ///
    /// Private, not `pub` or even `pub(crate)`: padding is only ever
    /// spec-correct at the very end of a scan's entropy-coded data, since
    /// it inserts bits with no coefficient meaning. Even a `pub(crate)`
    /// `flush` would let any code elsewhere in this crate pad partway
    /// through a scan and keep writing afterward, producing a well-formed
    /// but corrupted bitstream with no compiler error. The only real call
    /// is `into_bytes`'s, in this same module, which immediately consumes
    /// `self` afterward -- module-private is as narrow as this crate's
    /// visibility can state that.
    fn flush(&mut self) {
        // Each write moves the position one step toward `B0`, and the write
        // that leaves `B7` flushes the byte and resets it to `B0`, so this
        // loop writes exactly the bits that complete the current byte.
        while self.bit_pos != BitPos::ZERO {
            self.write_bit(Bit::One);
        }
    }

    /// Returns the complete, padded bitstream as a byte buffer. The
    /// only way to flush and pad the final byte, since `flush` itself is
    /// private to this module: consuming `self` here means no further
    /// bits can be written after padding, unlike a `pub fn flush(&mut
    /// self)` that left the writer usable afterward.
    #[must_use]
    pub fn into_bytes(mut self) -> ByteBuffer {
        self.flush();
        self.buffer
    }
}

impl Default for BitstreamWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
