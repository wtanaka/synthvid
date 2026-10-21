use super::*;
use synthvid_testkit::{Fail, OrFail as _};

/// The bytes a writer produced.
#[derive(Debug, PartialEq, Eq)]
struct Written(Vec<u8>);

/// The bytes a writer produces after writing each `(value, bit count)`.
fn written(bits: &[(BitValue, BitWidth)]) -> Result<Written, Fail> {
    let mut writer = BitstreamWriter::new();
    for &(BitValue(value), BitWidth(width)) in bits {
        writer.write_bits(BitPattern(value), BitCount::new(width).or_fail()?);
    }
    Ok(Written(writer.into_bytes().0))
}

/// A value to write, in a test.
#[derive(Clone, Copy)]
struct BitValue(u32);

/// How many bits of a value to write, in a test.
#[derive(Clone, Copy)]
struct BitWidth(u32);

#[test]
fn write_single_bit() {
    // One bit (1) then seven padding ones: 0xFF, which is stuffed as 0xFF 0x00.
    assert_eq!(
        written(&[(BitValue(1), BitWidth(1))]),
        Ok(Written(vec![0xFF, 0x00]))
    );
}

#[test]
fn write_byte_aligned() {
    // 0xFF should be stuffed as 0xFF 0x00
    assert_eq!(
        written(&[(BitValue(0xFF), BitWidth(8))]),
        Ok(Written(vec![0xFF, 0x00]))
    );
}

#[test]
fn write_multiple_bytes() {
    assert_eq!(
        written(&[(BitValue(0xAB), BitWidth(8)), (BitValue(0xCD), BitWidth(8))]),
        Ok(Written(vec![0xAB, 0xCD]))
    );
}

#[test]
fn byte_stuffing() {
    assert_eq!(
        written(&[(BitValue(0xFF), BitWidth(8))]),
        Ok(Written(vec![0xFF, 0x00]))
    );
}
