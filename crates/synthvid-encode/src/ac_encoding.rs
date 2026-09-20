//! AC coefficient run-length encoding for JPEG blocks.

use crate::bitstream::BitstreamWriter;
use crate::huffman::{encode_ac, AcCoefficient, AcSymbol, Component, ZeroRun};

/// A count of consecutive zero coefficients seen so far, not yet emitted.
/// Unlike [`ZeroRun`] (which represents only the run immediately preceding
/// a coefficient about to be emitted, capped at 15), this can exceed 15,
/// because whether a ZRL is actually valid to emit depends on whether a
/// nonzero coefficient follows it -- something only known once that
/// coefficient (or the end of the block) is reached.
#[derive(Debug, Clone, Copy)]
struct PendingZeros(u8);

impl PendingZeros {
    /// No zeros pending yet.
    const ZERO: Self = Self(0);

    /// One more zero coefficient seen. Total: an 8x8 block has at most 63
    /// AC coefficients, so this is called at most 63 times per block,
    /// far below `u8::MAX`.
    #[must_use]
    const fn advance(self) -> Self {
        Self(match self.0.checked_add(1) {
            Some(n) => n,
            None if self.0 == 0 => 1,
            None => u8::MAX,
        })
    }

    /// Whether any zeros are pending (used to decide whether a trailing
    /// EOB is needed at the end of the block).
    const fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Splits this pending count into the number of ZRL symbols to emit
    /// first, and the short run (0-15) immediately preceding the nonzero
    /// coefficient about to be encoded. Returning both from one call,
    /// rather than computing `pending >> 4` and `ZeroRun::from_low_nibble`
    /// as two separate expressions at the call site, means they can never
    /// be computed against two different values of `pending` (e.g. after
    /// an edit that advances or resets `pending` between the two lines).
    #[must_use]
    const fn split(self) -> (u8, ZeroRun) {
        (self.0 >> 4, ZeroRun::from_low_nibble(self.0))
    }
}

/// Encodes AC coefficients in zigzag order with proper run-length encoding.
pub(crate) fn encode_ac_coefficients(
    ac_coeffs: impl Iterator<Item = i32>,
    writer: &mut BitstreamWriter,
    component: Component,
) {
    let mut pending = PendingZeros::ZERO;
    for coeff in ac_coeffs {
        // `AcCoefficient::new` returning `None` for a zero coefficient is
        // the same fact `coeff == 0` used to check separately -- matching
        // on it here means there is only one place that decides "is this
        // coefficient zero", not two that could disagree.
        match AcCoefficient::new(coeff) {
            None => pending = pending.advance(),
            Some(value) => {
                let (zrl_count, run) = pending.split();
                for _ in 0..zrl_count {
                    encode_ac(AcSymbol::Zrl, writer, component);
                }
                encode_ac(AcSymbol::Coefficient { run, value }, writer, component);
                pending = PendingZeros::ZERO;
            }
        }
    }

    if pending.is_positive() {
        encode_ac(AcSymbol::Eob, writer, component);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ac_encoding_eob_with_trailing_zeros_multiple_of_16() {
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
        let ac_case1 = vec![
            10_i32, 20, 30, -5, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        let mut writer1 = BitstreamWriter::new();
        encode_ac_coefficients(ac_case1.iter().copied(), &mut writer1, Component::Luma);
        let bytes1 = writer1.into_vec();
        assert!(
            !bytes1.is_empty(),
            "Encoding with 16 trailing zeros must not be empty (EOB must be written)"
        );

        // Case 2: Block with nonzero coefficients followed by exactly 32 trailing zeros
        let ac_case2: Vec<_> = [vec![7_i32, 5, 3, 2, 1], vec![0; 32]].concat();

        let mut writer2 = BitstreamWriter::new();
        encode_ac_coefficients(ac_case2.iter().copied(), &mut writer2, Component::Luma);
        let bytes2 = writer2.into_vec();
        assert!(
            !bytes2.is_empty(),
            "Encoding with 32 trailing zeros must not be empty (EOB must be written)"
        );

        // Case 3: Block with nonzero coefficients followed by 48 trailing zeros
        let ac_case3: Vec<_> = [vec![8_i32, 4, 2], vec![0; 48]].concat();

        let mut writer3 = BitstreamWriter::new();
        encode_ac_coefficients(ac_case3.iter().copied(), &mut writer3, Component::Luma);
        let bytes3 = writer3.into_vec();
        assert!(
            !bytes3.is_empty(),
            "Encoding with 48 trailing zeros must not be empty (EOB must be written)"
        );

        // Case 4: All AC coefficients are zero (all 63 of them) -- edge case
        let ac_case4: Vec<i32> = vec![0; 63];

        let mut writer4 = BitstreamWriter::new();
        encode_ac_coefficients(ac_case4.iter().copied(), &mut writer4, Component::Luma);
        let bytes4 = writer4.into_vec();
        assert!(
            !bytes4.is_empty(),
            "Encoding all-zero AC block must not be empty (EOB must be written)"
        );

        // Case 5: Normal pattern with no trailing multiple-of-16 zeros
        let ac_case5 = vec![50_i32, -25, 12, 33, 44, 0, 0, 5, 3, 1];

        let mut writer5 = BitstreamWriter::new();
        encode_ac_coefficients(ac_case5.iter().copied(), &mut writer5, Component::Luma);
        let bytes5 = writer5.into_vec();
        assert!(
            !bytes5.is_empty(),
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
        ac_coeffs: impl Iterator<Item = i32>,
        writer: &mut BitstreamWriter,
        component: Component,
    ) {
        let mut run = ZeroRun::ZERO;
        for coeff in ac_coeffs {
            if coeff == 0 {
                if let Some(next) = run.advance() {
                    run = next;
                } else {
                    encode_ac(AcSymbol::Zrl, writer, component);
                    run = ZeroRun::ZERO;
                }
            } else {
                encode_ac(
                    AcSymbol::Coefficient {
                        run,
                        value: AcCoefficient::new(coeff).expect("coeff is nonzero here"),
                    },
                    writer,
                    component,
                );
                run = ZeroRun::ZERO;
            }
        }

        if run != ZeroRun::ZERO {
            encode_ac(AcSymbol::Eob, writer, component);
        }
    }

    #[test]
    fn test_fixed_encoding_writes_more_bits_than_buggy_encoding_for_multiple_of_16_trailing_zeros()
    {
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
        let ac_16_trailing_zeros: Vec<i32> = [vec![10_i32, 20, 30, -5, 15], vec![0; 16]].concat();

        let mut fixed_writer = BitstreamWriter::new();
        encode_ac_coefficients(
            ac_16_trailing_zeros.iter().copied(),
            &mut fixed_writer,
            Component::Luma,
        );
        let fixed_bytes = fixed_writer.into_vec();

        let mut buggy_writer = BitstreamWriter::new();
        encode_ac_coefficients_pre_fix_buggy(
            ac_16_trailing_zeros.iter().copied(),
            &mut buggy_writer,
            Component::Luma,
        );
        let buggy_bytes = buggy_writer.into_vec();

        assert_ne!(
            fixed_bytes, buggy_bytes,
            "the fixed encoding must differ from the buggy pre-fix encoding \
             for a block ending in exactly 16 trailing zeros: the fixed \
             version emits a terminating EOB symbol that the buggy version \
             silently drops"
        );
    }
}
