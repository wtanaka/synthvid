use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

use super::*;

#[test]
fn normal_from_uniform_draws_frozen_vectors() {
    assert_eq!(check_normal_from_uniform_draws_frozen_vectors(), Ok(()));
}

fn check_normal_from_uniform_draws_frozen_vectors() -> Result<(), Fail> {
    // Twelve draws of 128 sum to 1536, which is 6 above the center of 1530, so the sample is 6.
    ensure_eq!(
        normal_from_uniform_draws([UniformByte(128); 12]),
        NormalDraw(6),
        "all-128 draws must sit 6 above zero"
    );
    // Twelve draws of 255 sum to 3060, so the sample is 3060 - 1530 = 1530.
    ensure_eq!(
        normal_from_uniform_draws([UniformByte(255); 12]),
        NormalDraw(1530),
        "all-255 draws"
    );
    // Twelve draws of 0 sum to 0, so the sample is -1530.
    ensure_eq!(
        normal_from_uniform_draws([UniformByte(0); 12]),
        NormalDraw(-1530),
        "all-0 draws"
    );
    // Draws 1 through 12 sum to 78, so the sample is 78 - 1530 = -1452.
    ensure_eq!(
        normal_from_uniform_draws([
            UniformByte(1),
            UniformByte(2),
            UniformByte(3),
            UniformByte(4),
            UniformByte(5),
            UniformByte(6),
            UniformByte(7),
            UniformByte(8),
            UniformByte(9),
            UniformByte(10),
            UniformByte(11),
            UniformByte(12),
        ]),
        NormalDraw(-1452),
        "ascending draws"
    );
    // Six draws of 255 and six of 0 sum to 1530, which is exactly the center, so the sample is 0.
    ensure_eq!(
        normal_from_uniform_draws([
            UniformByte(255),
            UniformByte(0),
            UniformByte(255),
            UniformByte(0),
            UniformByte(255),
            UniformByte(0),
            UniformByte(255),
            UniformByte(0),
            UniformByte(255),
            UniformByte(0),
            UniformByte(255),
            UniformByte(0),
        ]),
        NormalDraw(0),
        "alternating draws"
    );
    Ok(())
}

#[test]
fn isqrt_frozen_vectors() {
    assert_eq!(check_isqrt_frozen_vectors(), Ok(()));
}

fn check_isqrt_frozen_vectors() -> Result<(), Fail> {
    let inputs = [0_u64, 1, 2, 3, 4, 15, 16, 17, 255, 256, 0xFFFF, 0x0001_0000];
    let expected = [0_u64, 1, 1, 1, 2, 3, 4, 4, 15, 16, 255, 256];
    for (input, root) in inputs.into_iter().zip(expected) {
        ensure_eq!(
            isqrt(IntSqrtInput(input)),
            IntSqrtOutput(root),
            "isqrt of {input}"
        );
    }
    ensure_eq!(
        isqrt(IntSqrtInput(0x0001_0000_0000)),
        IntSqrtOutput(0x0001_0000),
        "isqrt of 2^32"
    );
    ensure_eq!(
        isqrt(IntSqrtInput(u64::MAX)),
        IntSqrtOutput(0xFFFF_FFFF),
        "isqrt of u64::MAX"
    );
    Ok(())
}

#[test]
fn isqrt_brackets_every_small_input() {
    assert_eq!(check_isqrt_brackets_every_small_input(), Ok(()));
}

fn check_isqrt_brackets_every_small_input() -> Result<(), Fail> {
    for input in 0_u64..2000 {
        let IntSqrtOutput(root) = isqrt(IntSqrtInput(input));
        let square = root.checked_mul(root).or_fail()?;
        let next = root.checked_add(1).or_fail()?;
        let next_square = next.checked_mul(next).or_fail()?;
        ensure!(
            square <= input,
            "root {root} of {input} squares above the input"
        );
        ensure!(
            input < next_square,
            "root {root} of {input} is below the floor"
        );
    }
    Ok(())
}

/// The mirror image of twelve draws, each draw replaced by `255 - d`.
fn mirrored(draws: [UniformByte; 12]) -> Result<[UniformByte; 12], Fail> {
    let mut out = [UniformByte(0); 12];
    for (slot, UniformByte(byte)) in out.iter_mut().zip(draws) {
        *slot = UniformByte(u8::MAX.checked_sub(byte).or_fail()?);
    }
    Ok(out)
}

#[test]
fn irwin_hall_sample_is_antisymmetric_about_the_center() {
    assert_eq!(
        check_irwin_hall_sample_is_antisymmetric_about_the_center(),
        Ok(())
    );
}

fn check_irwin_hall_sample_is_antisymmetric_about_the_center() -> Result<(), Fail> {
    // The sum of the twelve draws minus 1530. Mirroring every draw to 255 - d replaces the sum S
    // by 12 * 255 - S = 3060 - S, so the sample becomes 1530 - S, which is the negation of S - 1530.
    // Each case gives its sample by hand: 1224 - 1530 = -306, 823 - 1530 = -707, and
    // 1428 - 1530 = -102.
    let cases: [([u8; 12], i32); 3] = [
        ([3, 250, 17, 99, 201, 0, 128, 64, 230, 7, 180, 45], -306),
        ([1, 2, 4, 8, 16, 32, 64, 128, 255, 200, 100, 13], -707),
        ([255, 254, 0, 1, 77, 78, 179, 180, 33, 222, 9, 140], -102),
    ];
    for (bytes, expected) in cases {
        let draws = bytes.map(UniformByte);
        let NormalDraw(found) = irwin_hall_sample(draws);
        ensure_eq!(found, expected, "sample of {bytes:?}");
        let NormalDraw(reflected) = irwin_hall_sample(mirrored(draws)?);
        ensure_eq!(
            reflected,
            found.checked_neg().or_fail()?,
            "mirrored sample of {bytes:?}"
        );
    }
    Ok(())
}

#[test]
fn normal_draw_has_unit_spread() {
    assert_eq!(check_normal_draw_has_unit_spread(), Ok(()));
}

fn check_normal_draw_has_unit_spread() -> Result<(), Fail> {
    // The center of the distribution is proven exactly by the symmetry test above, so this test
    // checks only the spread of a seeded sequence.
    const SAMPLES: i64 = 20_000;
    let mut rng = Rng::from_seed(Seed::new(12345));
    let mut sum_of_squares: i64 = 0;
    for _ in 0_u32..20_000 {
        let NormalDraw(sample) = normal_draw(&mut rng).or_fail()?;
        let wide = i64::from(sample);
        let square = wide.checked_mul(wide).or_fail()?;
        sum_of_squares = sum_of_squares.checked_add(square).or_fail()?;
    }
    // Sample variance sum_of_squares / n between 240^2 and 272^2.
    let low_variance = 57_600_i64.checked_mul(SAMPLES).or_fail()?;
    let high_variance = 73_984_i64.checked_mul(SAMPLES).or_fail()?;
    ensure!(
        sum_of_squares >= low_variance,
        "sample standard deviation below 240 in 256ths"
    );
    ensure!(
        sum_of_squares <= high_variance,
        "sample standard deviation above 272 in 256ths"
    );
    Ok(())
}

#[test]
fn stream_seed_is_deterministic() {
    assert_eq!(check_stream_seed_is_deterministic(), Ok(()));
}

fn check_stream_seed_is_deterministic() -> Result<(), Fail> {
    let base = Seed::new(0x0123_4567_89ab_cdef);
    ensure_eq!(
        stream_seed(base, FrameIndex::new(3), StreamIndex(7)),
        stream_seed(base, FrameIndex::new(3), StreamIndex(7)),
        "identical inputs must give the same seed"
    );
    Ok(())
}

#[test]
fn stream_seed_responds_to_each_input() {
    assert_eq!(check_stream_seed_responds_to_each_input(), Ok(()));
}

fn check_stream_seed_responds_to_each_input() -> Result<(), Fail> {
    let base = Seed::new(0x0123_4567_89ab_cdef);
    let reference = stream_seed(base, FrameIndex::new(3), StreamIndex(7));
    ensure!(
        stream_seed(base, FrameIndex::new(4), StreamIndex(7)) != reference,
        "changing only the frame must change the seed"
    );
    ensure!(
        stream_seed(base, FrameIndex::new(3), StreamIndex(8)) != reference,
        "changing only the index must change the seed"
    );
    ensure!(
        stream_seed(
            Seed::new(0x0123_4567_89ab_cdee),
            FrameIndex::new(3),
            StreamIndex(7)
        ) != reference,
        "changing only the base must change the seed"
    );
    ensure!(
        stream_seed(base, FrameIndex::new(7), StreamIndex(3)) != reference,
        "swapping frame and index must change the seed"
    );
    Ok(())
}

#[test]
fn stream_seed_distinct_over_a_grid() {
    assert_eq!(check_stream_seed_distinct_over_a_grid(), Ok(()));
}

fn check_stream_seed_distinct_over_a_grid() -> Result<(), Fail> {
    let base = Seed::new(0x0123_4567_89ab_cdef);
    let mut words = Vec::new();
    for frame in 0_u32..64 {
        for index in 0_u64..64 {
            let Seed(word) = stream_seed(base, FrameIndex::new(frame), StreamIndex(index));
            words.push(word);
        }
    }
    words.sort_unstable();
    ensure!(
        words.windows(2).all(|pair| pair.first() != pair.last()),
        "two frame and index pairs in the 64 by 64 grid share a seed"
    );
    Ok(())
}
