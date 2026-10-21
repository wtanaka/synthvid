//! Newtypes the Huffman coder and its standard tables are built from: a table
//! destination index, a canonical code word, a sign-bit pattern, the
//! code-length counts and symbol values of a standard table, a magnitude
//! category, and a value about to be coded.

use crate::bitstream::BitPattern;

/// The bits of one canonical Huffman code, right-aligned. Its length is
/// checked against the code's bit count by `HuffmanCode::new`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Codeword(pub(crate) u16);

impl Codeword {
    /// These bits as the pattern a [`BitstreamWriter`](crate::bitstream::BitstreamWriter)
    /// writes.
    #[must_use]
    pub(crate) fn bits(self) -> BitPattern {
        BitPattern(u32::from(self.0))
    }
}

/// The sign bits appended after a Huffman symbol for a nonzero value: the
/// low `magnitude` bits of the EXTEND-encoded value, right-aligned.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct SignPattern(pub(super) u32);

/// The code-length counts of a standard Huffman table: how many codes have
/// each length from 1 to 16.
#[derive(Debug, Clone, Copy)]
pub struct HuffmanBits(pub(crate) [u8; 16]);

/// The symbol values of a standard Huffman table, in code order.
#[derive(Debug, Clone, Copy)]
pub struct HuffmanValues(pub(crate) &'static [u8]);

/// A JPEG magnitude (size) category: how many bits a value's absolute value
/// needs, and so which Huffman symbol it is coded under (T.81 F.1.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SizeCategory(pub(crate) u8);

/// A signed value as the entropy coder sees it: a DC delta or an AC
/// coefficient, before it is split into a magnitude category and sign bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SignedValue(pub(super) i32);
