//! Types the bitstream writer is built from: the outcome of advancing a bit
//! position, the byte a writer is filling, the bits it is handed, and the
//! byte buffer it appends its finished bytes to.

/// Whether a bit write completed the byte being built.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ByteFill {
    /// The byte still has room for more bits.
    Partial,
    /// The byte is now full and must be flushed.
    Full,
}

/// The finished bytes a [`super::BitstreamWriter`] has produced so far, with
/// JPEG byte stuffing already applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteBuffer(pub(crate) Vec<u8>);

/// The byte a [`super::BitstreamWriter`] is still filling, its bits shifted
/// in from the low end as each one arrives.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct PartialByte(pub(super) u8);

/// The bits [`super::BitstreamWriter::write_bits`] writes, right-aligned:
/// only the low bits named by the accompanying `BitCount` are written.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BitPattern(pub(crate) u32);
