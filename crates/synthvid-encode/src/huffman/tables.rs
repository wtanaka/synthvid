//! Standard JPEG Huffman table data (the fixed bits/values arrays T.81
//! Annex K defines) and the canonical-table builder derived from them.

use super::HuffmanCode;
use crate::bitstream::BitCount;

/// Standard DC luminance table bits (counts of codes of length 1..16)
const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
/// Standard DC luminance table values
pub(super) const DC_LUMA_VALUES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

/// Standard DC chrominance table bits
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
/// Standard DC chrominance table values
pub(super) const DC_CHROMA_VALUES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

/// Standard AC luminance table bits
const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
/// Standard AC luminance table values (162 entries)
pub(super) const AC_LUMA_VALUES: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

/// Standard AC chrominance table bits
const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
/// Standard AC chrominance table values (162 entries)
pub(super) const AC_CHROMA_VALUES: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

/// A canonical Huffman table, indexed by symbol byte.
pub(super) type SymbolTable = [Option<HuffmanCode>; 256];

/// Builds a canonical Huffman table from bits and values.
///
/// `code` is `u16`, not `u32`: a canonical Huffman code of length
/// `code_len` (`1..=16` here) is always `< 2^code_len <= 2^16` -- a
/// property of canonical Huffman construction itself, not specific to this
/// crate's four fixed tables -- so it always fits `u16` directly, with no
/// narrowing conversion (and no fallback for a failure case that isn't
/// just "unlikely" but mathematically excluded) needed at all.
fn build_huffman_table(bits: &[u8; 16], values: &[u8]) -> SymbolTable {
    let mut table: SymbolTable = [None; 256];
    let mut code: u16 = 0;
    let mut values_iter = values.iter();

    // `bits` is fixed at 16 elements, so zipping it against `1_u8..=16`
    // gives `code_len` directly as a `u8` in `1..=16` -- total by
    // construction, rather than computing an index as `usize` and then
    // needing a fallible narrowing back to `u8` that can't actually fail.
    for (code_len, &num_codes_u8) in (1_u8..=16).zip(bits.iter()) {
        let num_codes = usize::from(num_codes_u8);

        for _ in 0..num_codes {
            // Advances through `values` in lockstep with the codes being
            // assigned, rather than a manually incremented index: there is
            // no counter here to prove bounded, only an iterator that
            // simply runs out (handled below) when a table's `bits` and
            // `values` disagree in length.
            let Some(&symbol) = values_iter.next() else {
                break;
            };
            if let Some(slot) = table.get_mut(usize::from(symbol)) {
                if let Some(bit_count) = BitCount::new(u32::from(code_len)) {
                    *slot = HuffmanCode::new(code, bit_count);
                }
            }
            // A canonical code of length `code_len` is always `<
            // 2^code_len`, so it is always `<= 2^16 - 2` before this last
            // increment within the group -- this can only reach `u16`'s
            // own limit if `code_len` itself is already 16, the one length
            // this loop cannot advance past afterward anyway.
            code = match code.checked_add(1) {
                Some(c) => c,
                None if code_len < 16 => u16::MAX,
                None => u16::MAX,
            };
        }

        code <<= 1;
    }

    table
}

/// Computes the DC luma table.
pub(super) fn get_dc_luma_table() -> SymbolTable {
    build_huffman_table(&DC_LUMA_BITS, &DC_LUMA_VALUES)
}

/// Computes the DC chroma table.
pub(super) fn get_dc_chroma_table() -> SymbolTable {
    build_huffman_table(&DC_CHROMA_BITS, &DC_CHROMA_VALUES)
}

/// Computes the AC luma table.
pub(super) fn get_ac_luma_table() -> SymbolTable {
    build_huffman_table(&AC_LUMA_BITS, &AC_LUMA_VALUES)
}

/// Computes the AC chroma table.
pub(super) fn get_ac_chroma_table() -> SymbolTable {
    build_huffman_table(&AC_CHROMA_BITS, &AC_CHROMA_VALUES)
}

/// Returns the DC luminance table bits for DHT marker.
#[must_use]
pub const fn get_dc_luma_bits() -> &'static [u8; 16] {
    &DC_LUMA_BITS
}

/// Returns the DC luminance table values for DHT marker.
#[must_use]
pub const fn get_dc_luma_values() -> &'static [u8; 12] {
    &DC_LUMA_VALUES
}

/// Returns the DC chrominance table bits for DHT marker.
#[must_use]
pub const fn get_dc_chroma_bits() -> &'static [u8; 16] {
    &DC_CHROMA_BITS
}

/// Returns the DC chrominance table values for DHT marker.
#[must_use]
pub const fn get_dc_chroma_values() -> &'static [u8; 12] {
    &DC_CHROMA_VALUES
}

/// Returns the AC luminance table bits for DHT marker.
#[must_use]
pub const fn get_ac_luma_bits() -> &'static [u8; 16] {
    &AC_LUMA_BITS
}

/// Returns the AC luminance table values for DHT marker.
#[must_use]
pub const fn get_ac_luma_values() -> &'static [u8; 162] {
    &AC_LUMA_VALUES
}

/// Returns the AC chrominance table bits for DHT marker.
#[must_use]
pub const fn get_ac_chroma_bits() -> &'static [u8; 16] {
    &AC_CHROMA_BITS
}

/// Returns the AC chrominance table values for DHT marker.
#[must_use]
pub const fn get_ac_chroma_values() -> &'static [u8; 162] {
    &AC_CHROMA_VALUES
}
