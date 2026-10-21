//! AC coefficient run-length encoding for JPEG blocks.

use crate::bitstream::BitstreamWriter;
use crate::huffman::{encode_ac, AcCoefficient, AcSymbol, Component, ZeroRun};
use core::iter::repeat_n;

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
            Some(incremented) => incremented,
            None if self.0 == 0 => 1,
            None => u8::MAX,
        })
    }

    /// Splits this pending count into the ZRL symbols to emit first, and the short run (0-15) immediately preceding the nonzero
    /// coefficient about to be encoded. Returning both from one call,
    /// rather than computing `pending >> 4` and `ZeroRun::from_low_nibble`
    /// as two separate expressions at the call site, means they can never
    /// be computed against two different values of `pending` (e.g. after
    /// an edit that advances or resets `pending` between the two lines).
    #[must_use]
    const fn split(self) -> (ZrlCount, ZeroRun) {
        (
            ZrlCount(self.0.div_euclid(16)),
            ZeroRun::from_low_nibble(self.0),
        )
    }
}

/// How many ZRL symbols (16 zeros each) precede a nonzero coefficient.
#[derive(Debug, Clone, Copy)]
struct ZrlCount(u8);

impl ZrlCount {
    /// One ZRL symbol per counted run of 16 zeros.
    fn symbols(self) -> impl Iterator<Item = AcSymbol> {
        repeat_n(AcSymbol::Zrl, usize::from(self.0))
    }
}

/// Encodes AC coefficients in zigzag order with proper run-length encoding.
///
/// Each coefficient arrives as `AcCoefficient::new`'s result, whose `None`
/// for a zero coefficient is the same fact `coeff == 0` used to check
/// separately -- matching on it here means there is only one place that
/// decides "is this coefficient zero", not two that could disagree.
pub(crate) fn encode_ac_coefficients(
    ac_coeffs: impl Iterator<Item = Option<AcCoefficient>>,
    writer: &mut BitstreamWriter,
    component: Component,
) {
    let mut pending = PendingZeros::ZERO;
    for coeff in ac_coeffs {
        match coeff {
            None => pending = pending.advance(),
            Some(value) => {
                let (zrl_count, run) = pending.split();
                for zrl in zrl_count.symbols() {
                    encode_ac(zrl, writer, component);
                }
                encode_ac(AcSymbol::Coefficient { run, value }, writer, component);
                pending = PendingZeros::ZERO;
            }
        }
    }

    // Any zeros still pending end the block, and need a trailing EOB.
    if pending.0 > 0 {
        encode_ac(AcSymbol::Eob, writer, component);
    }
}

#[cfg(test)]
mod tests;
