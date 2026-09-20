//! Deterministic pseudo-random number generation.
//!
//! [`Rng`] implements `SplitMix64` with its algorithm written out in full, so
//! the same [`Seed`] yields the same stream on every platform and
//! in every version.

use core::num::NonZeroI64;

use crate::ratio::{int_ratio, Ratio};
use crate::units::Seed;

/// A small, fully specified 64-bit counter-based pseudo-random number generator.
///
/// This generator implements the `SplitMix64` algorithm (Steele et al., 2014).
/// The algorithm is specified below so its outputs can never drift between versions
/// or across target architectures:
///
/// 1. State: a single 64-bit unsigned integer initialized to `seed.get()`.
/// 2. Transition:
///    `state = state.wrapping_add(0x9e37_79b9_7f4a_7c15)`
/// 3. Output mixing:
///    - `z = state`
///    - `z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9)`
///    - `z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb)`
///    - result = `z ^ (z >> 31)`
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Rng {
    /// The current 64-bit internal generator state.
    state: u64,
}

impl Rng {
    /// Creates a deterministic pseudo-random number generator from a [`Seed`].
    #[must_use]
    pub const fn from_seed(seed: Seed) -> Self {
        Self { state: seed.get() }
    }

    /// Generates the next pseudo-random 64-bit unsigned integer.
    pub const fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Generates a pseudo-random integer uniformly distributed in `[0, bound)`.
    ///
    /// Uses rejection sampling with a power-of-two bitmask to eliminate modulo bias.
    /// Returns 0 if `bound <= 1`.
    pub fn next_bounded(&mut self, bound: u64) -> u64 {
        if bound <= 1 {
            return 0;
        }
        let bits = 64_u32
            .checked_sub((bound.wrapping_sub(1)).leading_zeros())
            .unwrap_or(64);
        let mask = if bits >= 64 {
            u64::MAX
        } else {
            (1_u64.checked_shl(bits).unwrap_or(0)).wrapping_sub(1)
        };
        loop {
            let candidate = self.next_u64() & mask;
            if candidate < bound {
                return candidate;
            }
        }
    }

    /// Generates a pseudo-random [`Ratio`] uniformly distributed in the unit interval `[0, 1)`.
    ///
    /// The rational is formed by sampling a 62-bit numerator and placing it over a
    /// denominator of 2^62, then reducing to lowest terms via [`Ratio::new`].
    #[must_use]
    pub fn next_ratio_unit(&mut self) -> Ratio {
        let raw = self.next_u64();
        // Shift right by 2 to obtain 62 bits, fitting strictly within positive i64 bounds.
        let numer = i64::try_from(raw >> 2).unwrap_or_default();
        // 2^62 = 0x4000_0000_0000_0000 fits in positive i64.
        let denom = NonZeroI64::new(0x4000_0000_0000_0000).unwrap_or(NonZeroI64::MIN);
        Ratio::new(numer, denom).unwrap_or_else(|| int_ratio(0))
    }
}

impl From<Seed> for Rng {
    fn from(seed: Seed) -> Self {
        Self::from_seed(seed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rng_frozen_vectors() {
        let expected_seed_0: [u64; 16] = [
            0xe220_a839_7b1d_cdaf,
            0x6e78_9e6a_a1b9_65f4,
            0x06c4_5d18_8009_454f,
            0xf88b_b8a8_724c_81ec,
            0x1b39_896a_51a8_749b,
            0x53cb_9f0c_747e_a2ea,
            0x2c82_9abe_1f45_32e1,
            0xc584_133a_c916_ab3c,
            0x3ee5_7890_41c9_8ac3,
            0xf3b8_488c_368c_b0a6,
            0x657e_ecdd_3cb1_3d09,
            0xc2d3_26e0_055b_def6,
            0x8621_a03f_e0bb_db7b,
            0x8e1f_7555_983a_a92f,
            0xb54e_0f16_00cc_4d19,
            0x84bb_3f97_971d_80ab,
        ];

        let expected_seed_1: [u64; 16] = [
            0x910a_2dec_8902_5cc1,
            0xbeeb_8da1_658e_ec67,
            0xf893_a2ee_fb32_555e,
            0x71c1_8690_ee42_c90b,
            0x71bb_54d8_d101_b5b9,
            0xc34d_0bff_9015_0280,
            0xe099_ec6c_d736_3ca5,
            0x85e7_bb0f_1227_8575,
            0x4917_18de_357e_3da8,
            0xcb43_5c8e_7461_6796,
            0x6775_dc77_0156_4f61,
            0x9afc_d44d_14cf_8bfe,
            0x7476_cf8a_4baa_5dc0,
            0x87b3_41d6_90d7_a28a,
            0x6f9b_6dae_6f4c_57a8,
            0x2ac2_ce17_a579_4a3b,
        ];

        let expected_seed_deadbeef: [u64; 16] = [
            0x4adf_b90f_68c9_eb9b,
            0xde58_6a31_41a1_0922,
            0x021f_bc2f_8e1c_fc1d,
            0x7466_ce73_7be1_6790,
            0x3bfa_8764_f685_bd1c,
            0xab20_3e50_3cb5_5b3f,
            0x5a2f_dc2b_f68c_edb3,
            0xb30a_4ccf_430b_1b5a,
            0x0a90_4150_39bd_5985,
            0x26ae_5084_7745_eb7e,
            0xe239_ed30_6d9b_1929,
            0xfb7d_9a8d_444d_41bc,
            0x1bb5_2e52_3960_d559,
            0xcf86_31b4_0292_b5d5,
            0xf618_6c41_b838_b122,
            0x4324_97ff_b78c_1173,
        ];

        let mut rng0 = Rng::from_seed(Seed::new(0));
        for &expected in &expected_seed_0 {
            assert_eq!(rng0.next_u64(), expected, "seed 0 frozen vector mismatch");
        }

        let mut rng1 = Rng::from_seed(Seed::new(1));
        for &expected in &expected_seed_1 {
            assert_eq!(rng1.next_u64(), expected, "seed 1 frozen vector mismatch");
        }

        let mut rng_db = Rng::from_seed(Seed::new(0xdead_beef));
        for &expected in &expected_seed_deadbeef {
            assert_eq!(
                rng_db.next_u64(),
                expected,
                "seed 0xDEADBEEF frozen vector mismatch"
            );
        }
    }

    #[test]
    fn test_rng_bounded() {
        let mut rng = Rng::from_seed(Seed::new(42));
        assert_eq!(rng.next_bounded(0), 0, "bound 0 must return 0");
        assert_eq!(rng.next_bounded(1), 0, "bound 1 must return 0");

        for _ in 0..1000 {
            let val = rng.next_bounded(7);
            assert!(val < 7, "bounded value must be strictly less than bound 7");
        }

        for _ in 0..1000 {
            let val = rng.next_bounded(100);
            assert!(
                val < 100,
                "bounded value must be strictly less than bound 100"
            );
        }

        for _ in 0..1000 {
            let val = rng.next_bounded(1024);
            assert!(
                val < 1024,
                "bounded value must be strictly less than bound 1024"
            );
        }

        // Test determinism
        let mut rng_a = Rng::from_seed(Seed::new(999));
        let mut rng_b = Rng::from_seed(Seed::new(999));
        for _ in 0..100 {
            assert_eq!(
                rng_a.next_bounded(50),
                rng_b.next_bounded(50),
                "identical seeds must yield identical bounded sequence"
            );
        }
    }

    #[test]
    fn test_rng_ratio_unit() {
        let mut rng = Rng::from_seed(Seed::new(123));
        let Some(one) = Ratio::from_integer(1) else {
            return;
        };
        let Some(zero) = Ratio::from_integer(0) else {
            return;
        };

        for _ in 0..1000 {
            let r = rng.next_ratio_unit();
            assert!(r >= zero, "unit ratio must be non-negative");
            assert!(r < one, "unit ratio must be strictly less than 1");
            assert!(r.numer() >= 0, "unit ratio numerator must be non-negative");
            assert!(
                r.denom().get() > 0,
                "unit ratio denominator must be strictly positive"
            );
        }

        // Test determinism
        let mut rng_a = Rng::from_seed(Seed::new(777));
        let mut rng_b = Rng::from_seed(Seed::new(777));
        for _ in 0..100 {
            assert_eq!(
                rng_a.next_ratio_unit(),
                rng_b.next_ratio_unit(),
                "identical seeds must yield identical ratio unit sequence"
            );
        }
    }
}
