//! Bitstream writing for JPEG data.

use core::num::NonZeroU8;

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
    const fn advance(self) -> (Self, bool) {
        match self {
            Self::B0 => (Self::B1, false),
            Self::B1 => (Self::B2, false),
            Self::B2 => (Self::B3, false),
            Self::B3 => (Self::B4, false),
            Self::B4 => (Self::B5, false),
            Self::B5 => (Self::B6, false),
            Self::B6 => (Self::B7, false),
            Self::B7 => (Self::B0, true),
        }
    }

    /// How many more bits are needed to fill the current byte. Only
    /// meaningful (and only called) when not [`Self::ZERO`]; used solely
    /// to pad the final byte's remaining bits before flushing.
    const fn pending(self) -> u8 {
        match self {
            Self::B0 => 8,
            Self::B1 => 7,
            Self::B2 => 6,
            Self::B3 => 5,
            Self::B4 => 4,
            Self::B5 => 3,
            Self::B6 => 2,
            Self::B7 => 1,
        }
    }
}

/// A writer for bits to a byte buffer.
#[derive(Debug, Clone)]
pub struct BitstreamWriter {
    /// Output buffer.
    buffer: Vec<u8>,
    /// Current byte being built, bit-shifted in from the low end as each
    /// new bit arrives.
    current_byte: u8,
    /// Number of bits used in `current_byte`.
    bit_pos: BitPos,
}

impl BitstreamWriter {
    /// Creates a new empty bitstream writer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buffer: Vec::new(),
            current_byte: 0,
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
    pub fn write_bits(&mut self, value: u32, n_bits: BitCount) {
        let n_bits = n_bits.get();
        // `i` ranges over `0..n_bits`, and `n_bits` is at most 32, so
        // shifting the 32-bit `value` right by `i` is always in range
        // (shifting a `u32` by 32 or more is what panics; `i` never
        // reaches `n_bits` itself, let alone 32).
        for i in (0..n_bits).rev() {
            let bit = (value >> i) & 1 == 1;
            self.write_bit(bit);
        }
    }

    /// Writes a single bit, most-significant-bit-first within the current
    /// byte, flushing that byte once it fills.
    fn write_bit(&mut self, bit: bool) {
        // `u8::from(bool)` is total (0 or 1), unlike a `try_from` on an
        // arbitrary integer -- there is no fallible case to handle here
        // at all.
        self.current_byte = (self.current_byte << 1) | u8::from(bit);
        let (next, full) = self.bit_pos.advance();
        self.bit_pos = next;

        if full {
            self.flush_byte();
        }
    }

    /// Flushes the current byte to the buffer, handling JPEG byte stuffing.
    /// Only called right after [`Self::write_bit`] observes the byte just
    /// became full, so there is no separate "is it actually full" check
    /// here to get out of sync with that call site.
    fn flush_byte(&mut self) {
        let byte = self.current_byte;
        self.buffer.push(byte);

        // Byte stuffing: if we wrote 0xFF, add 0x00
        if byte == 0xFF {
            self.buffer.push(0x00);
        }

        self.current_byte = 0;
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
    /// is `into_vec`'s, in this same module, which immediately consumes
    /// `self` afterward -- module-private is as narrow as this crate's
    /// visibility can state that.
    fn flush(&mut self) {
        if self.bit_pos != BitPos::ZERO {
            for _ in 0..self.bit_pos.pending() {
                self.write_bit(true);
            }
        }
    }

    /// Returns the complete, padded bitstream as a vector of bytes. The
    /// only way to flush and pad the final byte, since `flush` itself is
    /// private to this module: consuming `self` here means no further
    /// bits can be written after padding, unlike a `pub fn flush(&mut
    /// self)` that left the writer usable afterward.
    #[must_use]
    pub fn into_vec(mut self) -> Vec<u8> {
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
mod tests {
    use super::*;

    #[test]
    fn test_write_single_bit() {
        let mut writer = BitstreamWriter::new();
        writer.write_bits(1, BitCount::new(1).unwrap());
        let result = writer.into_vec();
        // After writing 1 bit (value 1): current_byte = 0b10000000 = 0x80, bit_pos = 1
        // On flush: padding = 8 - 1 = 7 bits, filled with 1s per JPEG spec
        // So current_byte becomes 0b11111111 = 0xFF, which triggers byte stuffing (0xFF -> 0xFF 0x00)
        assert_eq!(result, vec![0xFF, 0x00]);
    }

    #[test]
    fn test_write_byte_aligned() {
        let mut writer = BitstreamWriter::new();
        writer.write_bits(0xFF, BitCount::new(8).unwrap());
        let result = writer.into_vec();
        // 0xFF should be stuffed as 0xFF 0x00
        assert_eq!(result, vec![0xFF, 0x00]);
    }

    #[test]
    fn test_write_multiple_bytes() {
        let mut writer = BitstreamWriter::new();
        writer.write_bits(0xAB, BitCount::new(8).unwrap());
        writer.write_bits(0xCD, BitCount::new(8).unwrap());
        let result = writer.into_vec();
        assert_eq!(result, vec![0xAB, 0xCD]);
    }

    #[test]
    fn test_byte_stuffing() {
        let mut writer = BitstreamWriter::new();
        writer.write_bits(0xFF, BitCount::new(8).unwrap());
        let result = writer.into_vec();
        assert_eq!(result, vec![0xFF, 0x00]);
    }
}
