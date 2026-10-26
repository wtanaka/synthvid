//! Deterministic pseudo-random number generation.
//!
//! [`Rng`] implements `SplitMix64` with its algorithm written out in full, so
//! the same [`Seed`] yields the same stream on every platform and
//! in every version.

use core::num::{NonZeroI64, NonZeroU64};

use crate::ratio::{Numerator, Overflow, Ratio};
use crate::units::Seed;

/// A small, fully specified 64-bit counter-based pseudo-random number generator.
///
/// This generator implements the `SplitMix64` algorithm (Steele et al., 2014).
/// The algorithm is specified below so its outputs can never drift between versions
/// or across target architectures:
///
/// 1. State: a single 64-bit unsigned integer initialized to `seed.get()`.
/// 2. Transition: `state = (state + 0x9e37_79b9_7f4a_7c15) mod 2^64`
/// 3. Output mixing, every product taken modulo 2^64:
///    - `z = state`
///    - `z = (z ^ (z >> 30)) * 0xbf58_476d_1ce4_e5b9`
///    - `z = (z ^ (z >> 27)) * 0x94d0_49bb_1331_11eb`
///    - result = `z ^ (z >> 31)`
///
/// The single field is the current 64-bit generator state; every `u64` is a
/// valid state.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Rng(u64);

/// One full 64-bit output of [`Rng::next_u64`]; every `u64` is a possible word.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct RandomWord(pub u64);

/// The exclusive upper limit of a draw from [`Rng::next_bounded`].
///
/// No value lies below a limit of zero, so the limit is non-zero by construction.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct DrawBound(pub NonZeroU64);

/// A value drawn by [`Rng::next_bounded`], strictly below its [`DrawBound`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct BoundedDraw(pub u64);

/// The low 64 bits of a 128-bit value: arithmetic modulo 2^64, written as a
/// wide operation followed by an exact truncation.
macro_rules! low_64 {
    ($wide:expr_2021) => {{
        let [b0, b1, b2, b3, b4, b5, b6, b7, ..] = ($wide).to_le_bytes();
        u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7])
    }};
}

pub(crate) use low_64;

impl Rng {
    /// Creates a deterministic pseudo-random number generator from a [`Seed`].
    #[must_use]
    pub const fn from_seed(seed: Seed) -> Self {
        Self(seed.get())
    }

    /// Generates the next pseudo-random 64-bit unsigned integer.
    pub fn next_u64(&mut self) -> RandomWord {
        self.0 = low_64!(u128::from(self.0) + 0x9e37_79b9_7f4a_7c15);
        let mut mixed = self.0;
        mixed = low_64!(u128::from(mixed ^ (mixed >> 30_u32)) * 0xbf58_476d_1ce4_e5b9);
        mixed = low_64!(u128::from(mixed ^ (mixed >> 27_u32)) * 0x94d0_49bb_1331_11eb);
        RandomWord(mixed ^ (mixed >> 31))
    }

    /// Generates a pseudo-random integer uniformly distributed in `[0, bound)`.
    ///
    /// Uses rejection sampling with a power-of-two bitmask to eliminate modulo bias.
    /// Returns 0, without drawing, if `bound` is 1.
    pub fn next_bounded(&mut self, bound: DrawBound) -> BoundedDraw {
        let limit = bound.0.get();
        let top = match limit.checked_sub(1) {
            None | Some(0) => return BoundedDraw(0),
            Some(top) => top,
        };
        // `bound > 1` here, so `top = bound - 1 >= 1` and its leading-zero count
        // is at most 63. Shifting `u64::MAX` right by that count leaves exactly the low
        // bits needed to represent `bound - 1`, which is the rejection mask. The
        // shift distance is never 64, so this cannot overflow and needs no
        // fallback.
        let mask = u64::MAX >> top.leading_zeros();
        loop {
            let candidate = self.next_u64().0 & mask;
            if candidate < limit {
                return BoundedDraw(candidate);
            }
        }
    }

    /// Generates a pseudo-random [`Ratio`] uniformly distributed in the unit interval `[0, 1)`.
    ///
    /// The rational is formed by sampling a 62-bit numerator and placing it over a
    /// denominator of 2^62, then reducing to lowest terms via [`Ratio::new`].
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` only if the reduced rational cannot be represented,
    /// which cannot happen for a numerator below 2^62 over 2^62.
    pub fn next_ratio_unit(&mut self) -> Result<Ratio, Overflow> {
        let RandomWord(raw) = self.next_u64();
        // Shift right by 2 to obtain 62 bits. Big-endian bytes of a value below 2^63
        // read back as the same non-negative i64, so this conversion has no failure case.
        let numer = i64::from_be_bytes((raw >> 2).to_be_bytes());
        // 2^62 = 0x4000_0000_0000_0000 is nonzero, so this discharge never fires.
        let denom = NonZeroI64::new(0x4000_0000_0000_0000).ok_or(Overflow)?;
        Ratio::new(Numerator(numer), denom)
    }
}

impl From<Seed> for Rng {
    fn from(seed: Seed) -> Self {
        Self::from_seed(seed)
    }
}

#[cfg(test)]
mod tests;
