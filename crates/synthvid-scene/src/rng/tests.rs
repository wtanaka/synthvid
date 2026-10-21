use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// The rejection mask was reformulated to drop a dead fallback. This pins
/// the new form to the old one across every bit width and the boundaries
/// either side of each, so the reformulation cannot have changed a draw.
#[test]
fn rejection_mask_matches_the_previous_formulation() {
    assert_eq!(
        check_rejection_mask_matches_the_previous_formulation(),
        Ok(())
    );
}

fn check_rejection_mask_matches_the_previous_formulation() -> Result<(), Fail> {
    // Both forms are only ever asked about a bound above one.
    let previous = |bound: u64| -> Result<u64, Fail> {
        let bits = 64_u32
            .checked_sub((bound.checked_sub(1).or_fail()?).leading_zeros())
            .or_fail()?;
        if bits >= 64 {
            Ok(u64::MAX)
        } else {
            1_u64.checked_shl(bits).or_fail()?.checked_sub(1).or_fail()
        }
    };
    let current = |bound: u64| -> Result<u64, Fail> {
        u64::MAX
            .checked_shr(bound.checked_sub(1).or_fail()?.leading_zeros())
            .or_fail()
    };

    let mut bounds = vec![2_u64, 3, 4, 5, 255, 256, 257, u64::MAX];
    bounds.push(u64::MAX.checked_sub(1).or_fail()?);
    for shift in 1_u32..64 {
        let power = 1_u64.checked_shl(shift).or_fail()?;
        bounds.push(power);
        bounds.push(power.checked_add(1).or_fail()?);
        bounds.push(power.checked_sub(1).or_fail()?);
    }
    for bound in bounds {
        if bound <= 1 {
            continue;
        }
        ensure_eq!(previous(bound)?, current(bound)?, "mask for bound {bound}");
    }
    Ok(())
}

/// Every draw must land inside the requested bound, including at the
/// power-of-two boundaries where the mask changes width.
#[test]
fn next_bounded_stays_within_its_bound() {
    assert_eq!(check_next_bounded_stays_within_its_bound(), Ok(()));
}

fn check_next_bounded_stays_within_its_bound() -> Result<(), Fail> {
    let mut rng = Rng::from_seed(Seed::new(0x1234_5678_9abc_def0));
    for bound in [2_u64, 3, 7, 8, 9, 255, 256, 1000, u64::MAX] {
        let limit = DrawBound(NonZeroU64::new(bound).or_fail()?);
        for _ in 0_u32..64 {
            let BoundedDraw(drawn) = rng.next_bounded(limit);
            ensure!(drawn < bound, "draw {drawn} not below bound {bound}");
        }
    }
    Ok(())
}
use super::*;

/// The first sixteen words drawn from seed 0.
const EXPECTED_SEED_0: [u64; 16] = [
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

/// The first sixteen words drawn from seed 1.
const EXPECTED_SEED_1: [u64; 16] = [
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

/// The first sixteen words drawn from seed `0xdead_beef`.
const EXPECTED_SEED_DEADBEEF: [u64; 16] = [
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

#[test]
fn rng_frozen_vectors() {
    assert_eq!(check_rng_frozen_vectors(), Ok(()));
}

fn check_rng_frozen_vectors() -> Result<(), Fail> {
    let sequences = [
        (Seed::new(0), &EXPECTED_SEED_0),
        (Seed::new(1), &EXPECTED_SEED_1),
        (Seed::new(0xdead_beef), &EXPECTED_SEED_DEADBEEF),
    ];
    for (seed, words) in sequences {
        let mut rng = Rng::from_seed(seed);
        let seed_value = seed.get();
        for &expected in words {
            ensure_eq!(
                rng.next_u64(),
                RandomWord(expected),
                "seed {seed_value:#x} frozen vector mismatch"
            );
        }
    }
    Ok(())
}

#[test]
fn rng_bounded() {
    assert_eq!(check_rng_bounded(), Ok(()));
}

fn check_rng_bounded() -> Result<(), Fail> {
    let mut rng = Rng::from_seed(Seed::new(42));
    ensure_eq!(NonZeroU64::new(0), None, "bound 0 must not be a bound");
    ensure_eq!(
        rng.next_bounded(DrawBound(NonZeroU64::MIN)),
        BoundedDraw(0),
        "bound 1 must return 0"
    );

    let seven = DrawBound(NonZeroU64::new(7).or_fail()?);
    for _ in 0_u32..1000 {
        let BoundedDraw(val) = rng.next_bounded(seven);
        ensure!(val < 7, "bounded value must be strictly less than bound 7");
    }

    let hundred = DrawBound(NonZeroU64::new(100).or_fail()?);
    for _ in 0_u32..1000 {
        let BoundedDraw(val) = rng.next_bounded(hundred);
        ensure!(
            val < 100,
            "bounded value must be strictly less than bound 100"
        );
    }

    let kibi = DrawBound(NonZeroU64::new(1024).or_fail()?);
    for _ in 0_u32..1000 {
        let BoundedDraw(val) = rng.next_bounded(kibi);
        ensure!(
            val < 1024,
            "bounded value must be strictly less than bound 1024"
        );
    }

    // Test determinism
    let fifty = DrawBound(NonZeroU64::new(50).or_fail()?);
    let mut rng_a = Rng::from_seed(Seed::new(999));
    let mut rng_b = Rng::from_seed(Seed::new(999));
    for _ in 0_u32..100 {
        ensure_eq!(
            rng_a.next_bounded(fifty),
            rng_b.next_bounded(fifty),
            "identical seeds must yield identical bounded sequence"
        );
    }
    Ok(())
}

#[test]
fn rng_ratio_unit() {
    assert_eq!(check_rng_ratio_unit(), Ok(()));
}

fn check_rng_ratio_unit() -> Result<(), Fail> {
    let mut rng = Rng::from_seed(Seed::new(123));
    let one = Ratio::from_integer(Numerator(1));
    let zero = Ratio::from_integer(Numerator(0));

    for _ in 0_u32..1000 {
        let draw = rng.next_ratio_unit().or_fail()?;
        ensure!(draw >= zero, "unit ratio must be non-negative");
        ensure!(draw < one, "unit ratio must be strictly less than 1");
        ensure!(
            draw.numer().0 >= 0,
            "unit ratio numerator must be non-negative"
        );
        ensure!(
            draw.denom().get() > 0,
            "unit ratio denominator must be strictly positive"
        );
    }

    // Test determinism
    let mut rng_a = Rng::from_seed(Seed::new(777));
    let mut rng_b = Rng::from_seed(Seed::new(777));
    for _ in 0_u32..100 {
        ensure_eq!(
            rng_a.next_ratio_unit().or_fail()?,
            rng_b.next_ratio_unit().or_fail()?,
            "identical seeds must yield identical ratio unit sequence"
        );
    }
    Ok(())
}
