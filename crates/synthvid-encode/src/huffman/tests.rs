use super::*;
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};
use tables::{AC_CHROMA_VALUES, AC_LUMA_VALUES, DC_CHROMA_VALUES, DC_LUMA_VALUES};

#[test]
fn encode_signed_value_positive() {
    assert_eq!(check_encode_signed_value_positive(), Ok(()));
}

fn check_encode_signed_value_positive() -> Result<(), Fail> {
    let result = encode_signed_value(SignedValue(5));
    ensure_eq!(result.magnitude(), SizeCategory(3));
    ensure_eq!(
        result.sign_bits,
        Some((BitCount::new(3).or_fail()?, SignPattern(5)))
    );
    Ok(())
}

#[test]
fn encode_signed_value_zero() {
    let result = encode_signed_value(SignedValue(0));
    assert_eq!(result.magnitude(), SizeCategory(0));
    assert_eq!(result.sign_bits, None);
}

#[test]
fn dc_luma_codes() {
    let table = tables::get_dc_luma_table();
    // DC symbol 0 should exist
    assert!(table.first().copied().flatten().is_some());
}

#[test]
fn clamp_to_magnitude_keeps_symbol_in_range() {
    // A value whose natural magnitude exceeds what the table defines
    // must clamp down to one that fits, not pass through unclamped.
    let dc_clamped = clamp_to_magnitude(SignedValue(100_000), MAX_DC_MAGNITUDE);
    let dc_bits = encode_signed_value(dc_clamped);
    assert!(dc_bits.magnitude() <= MAX_DC_MAGNITUDE);

    let ac_clamped = clamp_to_magnitude(SignedValue(-100_000), MAX_AC_MAGNITUDE);
    let ac_bits = encode_signed_value(ac_clamped);
    assert!(ac_bits.magnitude() <= MAX_AC_MAGNITUDE);
}

#[test]
fn clamp_to_magnitude_leaves_in_range_values_untouched() {
    assert_eq!(
        clamp_to_magnitude(SignedValue(5), MAX_DC_MAGNITUDE),
        SignedValue(5)
    );
    assert_eq!(
        clamp_to_magnitude(SignedValue(-5), MAX_DC_MAGNITUDE),
        SignedValue(-5)
    );
    assert_eq!(
        clamp_to_magnitude(SignedValue(0), MAX_AC_MAGNITUDE),
        SignedValue(0)
    );
}

#[test]
fn encode_dc_never_silently_drops_an_out_of_range_coefficient() {
    // Before the clamp fix, a DC delta needing more than 11 bits would
    // silently write nothing at all. Confirm a huge delta still writes
    // some bits to the stream instead of desyncing it silently.
    // 30_000 is the largest value that both exceeds MAX_DC_MAGNITUDE's
    // +/-2047 bound and fits in DcDelta's own i16 range.
    let mut writer = BitstreamWriter::new();
    let encoded = encode_dc(DcDelta::new(30_000), &mut writer, Component::Luma);
    // 2^MAX_DC_MAGNITUDE - 1 = 2^11 - 1.
    let max_dc = 2_047_i32;
    assert!(encoded.get().abs() <= max_dc && !writer.into_bytes().0.is_empty());
}

#[test]
fn encode_ac_never_silently_drops_an_out_of_range_coefficient() {
    assert_eq!(
        check_encode_ac_never_silently_drops_an_out_of_range_coefficient(),
        Ok(())
    );
}

fn check_encode_ac_never_silently_drops_an_out_of_range_coefficient() -> Result<(), Fail> {
    let mut writer = BitstreamWriter::new();
    encode_ac(
        AcSymbol::Coefficient {
            run: ZeroRun::new(0).or_fail()?,
            value: AcCoefficient::new(1_000_000).or_fail()?,
        },
        &mut writer,
        Component::Luma,
    );
    ensure!(!writer.into_bytes().0.is_empty());
    Ok(())
}

#[test]
fn max_dc_magnitude_matches_table_values() {
    // Re-derive the expected max DC magnitude directly from the tables
    let mut dc_luma_max = 0_u8;
    for &val in DC_LUMA_VALUES.0 {
        if val > dc_luma_max {
            dc_luma_max = val;
        }
    }
    let mut dc_chroma_max = 0_u8;
    for &val in DC_CHROMA_VALUES.0 {
        if val > dc_chroma_max {
            dc_chroma_max = val;
        }
    }
    let expected_max = if dc_luma_max > dc_chroma_max {
        dc_luma_max
    } else {
        dc_chroma_max
    };
    assert_eq!(
        MAX_DC_MAGNITUDE,
        SizeCategory(expected_max),
        "MAX_DC_MAGNITUDE must match the max value in the DC Huffman tables"
    );
}

#[test]
fn max_ac_magnitude_matches_table_values() {
    // Re-derive the expected max AC magnitude (low nibble) directly from the tables,
    // excluding special bytes 0x00 (EOB) and 0xF0 (ZRL)
    let mut ac_luma_max = 0_u8;
    for &byte in AC_LUMA_VALUES.0 {
        if byte != 0x00 && byte != 0xF0 {
            let size = byte.rem_euclid(0x10);
            if size > ac_luma_max {
                ac_luma_max = size;
            }
        }
    }
    let mut ac_chroma_max = 0_u8;
    for &byte in AC_CHROMA_VALUES.0 {
        if byte != 0x00 && byte != 0xF0 {
            let size = byte.rem_euclid(0x10);
            if size > ac_chroma_max {
                ac_chroma_max = size;
            }
        }
    }
    let expected_max = if ac_luma_max > ac_chroma_max {
        ac_luma_max
    } else {
        ac_chroma_max
    };
    assert_eq!(
        MAX_AC_MAGNITUDE,
        SizeCategory(expected_max),
        "MAX_AC_MAGNITUDE must match the max AC size category in the Huffman tables"
    );
}

/// Regression test for `encode_signed_value`: must be exactly invertible via
/// the JPEG T.81 EXTEND procedure for both positive and negative values.
/// A previous bug computed the wrong sign-bit pattern for every negative
/// value (either a full sign flip when |V| was a power of two, or an
/// off-by-one error toward zero otherwise), which this round-trip test
/// would have caught immediately since the existing tests never covered
/// a negative case.
#[test]
fn encode_signed_value_round_trip_with_negative_values() {
    /// Decodes a value that was encoded with `encode_signed_value`, using
    /// the JPEG T.81 F.1.2.1.3 EXTEND procedure. Given magnitude S and
    /// the unsigned S-bit value that was transmitted, returns the
    /// original signed value.
    ///
    /// `None` only for a magnitude too large to shift by, which no
    /// standard table produces.
    #[must_use]
    fn decode_extend(category: SizeCategory, pattern: SignPattern) -> Option<Decoded> {
        let SizeCategory(magnitude) = category;
        let SignPattern(sign_bits) = pattern;
        if magnitude == 0 {
            return Some(Decoded(0));
        }
        let half = 1_u32.checked_shl(u32::from(magnitude.checked_sub(1)?))?;
        if sign_bits < half {
            // Negative case: V = V' - (1<<magnitude) + 1
            let span = 1_u32.checked_shl(u32::from(magnitude))?;
            i64::from(sign_bits)
                .checked_sub(i64::from(span))?
                .checked_add(1)
                .map(Decoded)
        } else {
            // Positive case: V = V'
            Some(Decoded(i64::from(sign_bits)))
        }
    }

    /// A value `decode_extend` reconstructed.
    #[derive(Debug, PartialEq, Eq)]
    struct Decoded(i64);

    // Test critical values that the buggy code would have corrupted:
    // V = -1 (S=1, mask=1): correct = 1 XOR 1 = 0. Buggy = (1-1) XOR 1 = 1.
    // V = -2 (S=2, mask=3): correct = 2 XOR 3 = 1. Buggy = (2-1) XOR 3 = 2.
    // V = -5 (S=3, mask=7): correct = 5 XOR 7 = 2. Buggy = (5-1) XOR 7 = 3.
    let test_values = [
        -1_i32, -2_i32, -3_i32, -4_i32, -5_i32, -7_i32, -15_i32, -31_i32, -63_i32, -127_i32,
        -255_i32, -511_i32, -1_023_i32, -2_047_i32, 1_i32, 2_i32, 3_i32, 4_i32, 5_i32, 7_i32,
        15_i32, 31_i32, 63_i32, 127_i32, 255_i32, 511_i32, 1_023_i32, 2_047_i32, 0_i32,
    ];

    for original in &test_values {
        let encoded = encode_signed_value(SignedValue(*original));
        let magnitude = encoded.magnitude();
        let sign_bits = match encoded.sign_bits {
            Some((_, pattern)) => pattern,
            None => SignPattern(0), // magnitude == 0 case
        };

        let decoded = decode_extend(magnitude, sign_bits);
        assert_eq!(
            decoded,
            Some(Decoded(i64::from(*original))),
            "Round-trip failed for value {original}: encoded as magnitude={magnitude:?}, \
             sign_bits={sign_bits:?}, decoded as {decoded:?}"
        );
    }
}
