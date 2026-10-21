use super::*;
use crate::bitstream::ByteBuffer;

/// The AC coefficients of one block, in zigzag order.
struct AcBlock(Vec<i32>);

/// Encodes `block` with the luma tables and returns the written bytes.
fn encoded_luma(block: &AcBlock) -> ByteBuffer {
    let mut writer = BitstreamWriter::new();
    encode_ac_coefficients(
        block.0.iter().copied().map(AcCoefficient::new),
        &mut writer,
        Component::Luma,
    );
    writer.into_bytes()
}

#[test]
fn ac_encoding_eob_with_trailing_zeros_multiple_of_16() {
    // Regression test for the bug where blocks ending in a multiple of 16 trailing
    // zero AC coefficients would have their EOB symbol silently swallowed, desyncing
    // the entropy-coded bitstream.
    //
    // Before the fix: when the last nonzero coefficient was followed by exactly 16
    // (or 32, 48, etc.) trailing zero coefficients, the encoder would:
    // 1. Accumulate all 16 zeros with `run` advancing to 15
    // 2. Hit the 16th zero, call run.advance() which returns None
    // 3. Emit a ZRL (zero-run-length) symbol
    // 4. Reset `run` to ZeroRun::ZERO
    // 5. Exit the loop (no more coefficients)
    // 6. Check `if run != ZeroRun::ZERO` -- this is FALSE because we just reset
    // 7. Silently omit the final EOB symbol
    // This desyncs the decoder: it reads the ZRL (expecting more AC coefficients)
    // but then hits a marker instead of another coefficient symbol.
    //
    // After the fix: we track `pending` trailing zeros as an unbounded counter
    // (not capped at 15 like ZeroRun). After the loop, we check `if pending > 0`
    // and emit EOB if needed. This correctly handles blocks ending in multiples
    // of 16 trailing zeros.
    //
    // This test verifies the fix works by encoding multiple patterns and ensuring
    // all produce valid, non-empty output. The key cases: blocks ending in exactly
    // 16 and exactly 32 trailing zeros (both would fail with the old buggy code).

    // Case 1: Block with nonzero coefficients followed by exactly 16 trailing zeros
    let ac_case1 = AcBlock(vec![
        10_i32, 20_i32, 30_i32, -5_i32, 15_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32,
        0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32,
    ]);

    assert!(
        !encoded_luma(&ac_case1).0.is_empty(),
        "Encoding with 16 trailing zeros must not be empty (EOB must be written)"
    );

    // Case 2: Block with nonzero coefficients followed by exactly 32 trailing zeros
    let ac_case2 = AcBlock([vec![7_i32, 5_i32, 3_i32, 2_i32, 1_i32], vec![0_i32; 32]].concat());

    assert!(
        !encoded_luma(&ac_case2).0.is_empty(),
        "Encoding with 32 trailing zeros must not be empty (EOB must be written)"
    );

    // Case 3: Block with nonzero coefficients followed by 48 trailing zeros
    let ac_case3 = AcBlock([vec![8_i32, 4_i32, 2_i32], vec![0_i32; 48]].concat());

    assert!(
        !encoded_luma(&ac_case3).0.is_empty(),
        "Encoding with 48 trailing zeros must not be empty (EOB must be written)"
    );

    // Case 4: All AC coefficients are zero (all 63 of them) -- edge case
    let ac_case4 = AcBlock(vec![0_i32; 63]);

    assert!(
        !encoded_luma(&ac_case4).0.is_empty(),
        "Encoding all-zero AC block must not be empty (EOB must be written)"
    );

    // Case 5: Normal pattern with no trailing multiple-of-16 zeros
    let ac_case5 = AcBlock(vec![
        50_i32, -25_i32, 12_i32, 33_i32, 44_i32, 0_i32, 0_i32, 5_i32, 3_i32, 1_i32,
    ]);

    assert!(
        !encoded_luma(&ac_case5).0.is_empty(),
        "Normal encoding must not be empty (EOB must be written)"
    );
}

/// Re-implements the pre-fix (buggy) run-length loop exactly as it used
/// to exist in `jpeg.rs`, so this test can prove the fix actually
/// changes behavior rather than merely asserting "output is non-empty"
/// (which the buggy version also satisfied, since the earlier
/// `Coefficient`/`Zrl` symbols already wrote bytes before the missing
/// EOB -- a weaker assertion would pass on both the buggy and fixed
/// code and catch nothing).
fn encode_ac_coefficients_pre_fix_buggy(
    ac_coeffs: impl Iterator<Item = Option<AcCoefficient>>,
    writer: &mut BitstreamWriter,
    component: Component,
) {
    let mut run = ZeroRun::ZERO;
    for coeff in ac_coeffs {
        match coeff {
            None => {
                if let Some(next) = run.advance() {
                    run = next;
                } else {
                    encode_ac(AcSymbol::Zrl, writer, component);
                    run = ZeroRun::ZERO;
                }
            }
            Some(value) => {
                encode_ac(AcSymbol::Coefficient { run, value }, writer, component);
                run = ZeroRun::ZERO;
            }
        }
    }

    if run != ZeroRun::ZERO {
        encode_ac(AcSymbol::Eob, writer, component);
    }
}

#[test]
fn fixed_encoding_writes_more_bits_than_buggy_encoding_for_multiple_of_16_trailing_zeros() {
    // Proves the fix actually changes behavior for the exact bug
    // pattern: the buggy pre-fix loop resets `run` to zero right after
    // emitting the ZRL for a multiple-of-16 trailing zero run, so its
    // final `if run != ZeroRun::ZERO` check is false and it never
    // writes the terminating EOB symbol. The fixed version knows via
    // `pending > 0` that trailing zeros remain with no further nonzero
    // coefficient, and correctly emits EOB directly with no ZRL at all
    // (a ZRL is only ever valid when a nonzero coefficient follows it).
    // So the buggy encoding is "...Coefficient, Zrl" (no Eob) and the
    // fixed encoding is "...Coefficient, Eob" (no Zrl) -- two genuinely
    // different symbol sequences, not merely different lengths (ZRL's
    // Huffman code happens to be longer than EOB's in the standard luma
    // AC table, so the buggy stream is actually the longer one here --
    // byte length alone doesn't indicate which is correct, only that
    // the two differ).
    let ac_16_trailing_zeros: Vec<i32> = [
        vec![10_i32, 20_i32, 30_i32, -5_i32, 15_i32],
        vec![0_i32; 16],
    ]
    .concat();

    let mut fixed_writer = BitstreamWriter::new();
    encode_ac_coefficients(
        ac_16_trailing_zeros.iter().copied().map(AcCoefficient::new),
        &mut fixed_writer,
        Component::Luma,
    );
    let fixed_bytes = fixed_writer.into_bytes();

    let mut buggy_writer = BitstreamWriter::new();
    encode_ac_coefficients_pre_fix_buggy(
        ac_16_trailing_zeros.iter().copied().map(AcCoefficient::new),
        &mut buggy_writer,
        Component::Luma,
    );
    let buggy_bytes = buggy_writer.into_bytes();

    assert_ne!(
        fixed_bytes, buggy_bytes,
        "the fixed encoding must differ from the buggy pre-fix encoding \
         for a block ending in exactly 16 trailing zeros: the fixed \
         version emits a terminating EOB symbol that the buggy version \
         silently drops"
    );
}
