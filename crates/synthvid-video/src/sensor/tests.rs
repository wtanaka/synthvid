use super::*;
use crate::medium::{SensorNoiseError, SensorSeeds, TemporalSeed};
use synthvid_scene::{Height, Seed, Width};
use synthvid_testkit::{ensure, ensure_eq, ensure_ne, Fail, OrFail as _};

/// A frame size, named by its columns and rows so the two counts cannot be swapped.
#[derive(Copy, Clone, Debug)]
struct Size {
    /// Number of sensels across the frame.
    columns: Width,
    /// Number of sensels down the frame.
    rows: Height,
}

/// Dimensions of a frame of the given size.
fn dims(size: Size) -> Dimensions {
    Dimensions::new(size.columns, size.rows)
}

/// A frame of the given size with every channel set to `value`.
fn flat_frame(size: Size, value: Channel) -> Result<Frame, Fail> {
    let color = Rgb8 {
        red: value,
        green: value,
        blue: value,
    };
    Frame::from_color(dims(size), color).or_fail()
}

fn temporal(seed: Seed) -> TemporalSeed {
    TemporalSeed::new(seed)
}

fn pattern(seed: Seed) -> PatternSeed {
    PatternSeed::new(seed)
}

fn seeds(temporal: TemporalSeed, pattern: PatternSeed) -> SensorSeeds {
    SensorSeeds::new(temporal, pattern)
}

/// Seeds with awkward bit patterns, so no test relies on a convenient value.
fn awkward_seeds() -> SensorSeeds {
    seeds(
        temporal(Seed(0x0BAD_5EED_1234_5677)),
        pattern(Seed(0xC0FF_EE00_DEAD_BEE3)),
    )
}

/// Noise with only read noise present, of the given standard deviation.
fn read_only(read: ReadSigma, seeds: SensorSeeds) -> Result<SensorNoise, Fail> {
    SensorNoise::new(
        ShotGain(Q8_8(0)),
        read,
        PrnuSpread(0),
        ColumnSigma(Q8_8(0)),
        seeds,
    )
    .or_fail()
}

/// Noise with only the per-sensel gain variation present.
fn prnu_only(spread: PrnuSpread, seeds: SensorSeeds) -> Result<SensorNoise, Fail> {
    SensorNoise::new(
        ShotGain(Q8_8(0)),
        ReadSigma(Q8_8(0)),
        spread,
        ColumnSigma(Q8_8(0)),
        seeds,
    )
    .or_fail()
}

/// Noise with only the column offsets present, of the given standard deviation.
fn column_only(sigma: ColumnSigma, seeds: SensorSeeds) -> Result<SensorNoise, Fail> {
    SensorNoise::new(
        ShotGain(Q8_8(0)),
        ReadSigma(Q8_8(0)),
        PrnuSpread(0),
        sigma,
        seeds,
    )
    .or_fail()
}

/// Noise with only shot noise present, of the given per-sensel slope.
fn shot_only(slope: ShotGain, seeds: SensorSeeds) -> Result<SensorNoise, Fail> {
    SensorNoise::new(
        slope,
        ReadSigma(Q8_8(0)),
        PrnuSpread(0),
        ColumnSigma(Q8_8(0)),
        seeds,
    )
    .or_fail()
}

/// Applies the noise to a copy of `frame` at the given frame index.
fn applied(frame: &Frame, noise: &SensorNoise, index: FrameIndex) -> Result<Frame, Fail> {
    let mut copy = frame.clone();
    apply_sensor_noise(&mut copy, noise, index).or_fail()?;
    Ok(copy)
}

/// The color at the given coordinate.
fn pixel(frame: &Frame, coord: PixelCoord) -> Result<Rgb8, Fail> {
    frame.get_pixel(coord).or_fail()
}

/// The normal draw of the stream `stream` of the seed `seed` at frame `frame`.
fn draw_on(seed: Seed, frame: FrameIndex, stream: StreamIndex) -> Result<NormalDraw, Fail> {
    let mut rng = Rng::from_seed(stream_seed(seed, frame, stream));
    match normal_draw(&mut rng) {
        Ok(draw) => Ok(draw),
        Err(Overflow) => Err(Fail::here(format_args!("a normal draw overflowed"))),
    }
}

mod probe;
mod shot;

#[test]
fn combine_level_matches_hand_computed_vectors() {
    assert_eq!(check_combine_level_matches_hand_computed_vectors(), Ok(()));
}

fn check_combine_level_matches_hand_computed_vectors() -> Result<(), Fail> {
    // Level 25600 is value 100 at unit gain: no offsets gives 100.
    ensure_eq!(
        combine_level(Level(25_600), ColumnOffset(0), ShotNoise(0), ReadNoise(0)).or_fail()?,
        Channel(100)
    );
    // 51200 + 1536 = 52736, and (52736 + 128) / 256 = 206.5, floored to 206.
    ensure_eq!(
        combine_level(
            Level(51_200),
            ColumnOffset(0),
            ShotNoise(1_536),
            ReadNoise(0)
        )
        .or_fail()?,
        Channel(206)
    );
    // 64000 + 1536 = 65536 rounds to 256 and clamps to full scale.
    ensure_eq!(
        combine_level(
            Level(64_000),
            ColumnOffset(0),
            ShotNoise(1_536),
            ReadNoise(0)
        )
        .or_fail()?,
        Channel(255)
    );
    // 768 - 1536 = -768 is below zero and clamps to 0.
    ensure_eq!(
        combine_level(Level(768), ColumnOffset(0), ShotNoise(-1_536), ReadNoise(0)).or_fail()?,
        Channel(0)
    );
    // Level 2560 is value 10, and the ties round toward positive infinity.
    ensure_eq!(
        combine_level(Level(2_560), ColumnOffset(0), ShotNoise(127), ReadNoise(0)).or_fail()?,
        Channel(10)
    );
    ensure_eq!(
        combine_level(Level(2_560), ColumnOffset(0), ShotNoise(128), ReadNoise(0)).or_fail()?,
        Channel(11)
    );
    ensure_eq!(
        combine_level(Level(2_560), ColumnOffset(0), ShotNoise(-128), ReadNoise(0)).or_fail()?,
        Channel(10)
    );
    ensure_eq!(
        combine_level(Level(2_560), ColumnOffset(0), ShotNoise(-129), ReadNoise(0)).or_fail()?,
        Channel(9)
    );
    Ok(())
}

#[test]
fn combine_level_sums_all_four_terms() {
    assert_eq!(check_combine_level_sums_all_four_terms(), Ok(()));
}

fn check_combine_level_sums_all_four_terms() -> Result<(), Fail> {
    // 2560 + 128 + 0 - 128 = 2560, which is value 10.
    ensure_eq!(
        combine_level(
            Level(2_560),
            ColumnOffset(128),
            ShotNoise(0),
            ReadNoise(-128)
        )
        .or_fail()?,
        Channel(10)
    );
    // 2560 - 64 + 192 = 2688, and (2688 + 128) / 256 = 11.
    ensure_eq!(
        combine_level(
            Level(2_560),
            ColumnOffset(-64),
            ShotNoise(0),
            ReadNoise(192)
        )
        .or_fail()?,
        Channel(11)
    );
    Ok(())
}

#[test]
fn level_floors_the_gained_value() {
    assert_eq!(check_level_floors_the_gained_value(), Ok(()));
}

fn check_level_floors_the_gained_value() -> Result<(), Fail> {
    // 3 * 256 * 40000 / 65536 = 468.75, floored to 468.
    ensure_eq!(level_of(Channel(3), Gain(40_000)).or_fail()?, Level(468));
    // 3 * 256 * -1 / 65536 = -0.0117, which floors to -1 rather than truncating to 0.
    ensure_eq!(level_of(Channel(3), Gain(-1)).or_fail()?, Level(-1));
    Ok(())
}

#[test]
fn all_zero_sensor_noise_is_refused() {
    assert_eq!(check_all_zero_sensor_noise_is_refused(), Ok(()));
}

fn check_all_zero_sensor_noise_is_refused() -> Result<(), Fail> {
    let refused = SensorNoise::new(
        ShotGain(Q8_8(0)),
        ReadSigma(Q8_8(0)),
        PrnuSpread(0),
        ColumnSigma(Q8_8(0)),
        awkward_seeds(),
    );
    ensure_eq!(refused, Err(SensorNoiseError::AllTermsAbsent));
    Ok(())
}

#[test]
fn a_single_nonzero_term_is_a_sensor_noise() {
    assert_eq!(check_a_single_nonzero_term_is_a_sensor_noise(), Ok(()));
}

fn check_a_single_nonzero_term_is_a_sensor_noise() -> Result<(), Fail> {
    let shot = shot_only(ShotGain(Q8_8(1)), awkward_seeds())?;
    ensure_eq!(shot.shot(), ShotGain(Q8_8(1)));
    let column = column_only(ColumnSigma(Q8_8(1)), awkward_seeds())?;
    ensure_eq!(column.column(), ColumnSigma(Q8_8(1)));
    Ok(())
}

#[test]
fn same_inputs_give_identical_frames() {
    assert_eq!(check_same_inputs_give_identical_frames(), Ok(()));
}

fn check_same_inputs_give_identical_frames() -> Result<(), Fail> {
    let frame = flat_frame(
        Size {
            columns: Width::new(17).or_fail()?,
            rows: Height::new(31).or_fail()?,
        },
        Channel(128),
    )?;
    let noise = SensorNoise::new(
        ShotGain(Q8_8(64)),
        ReadSigma(Q8_8(900)),
        PrnuSpread(3_001),
        ColumnSigma(Q8_8(77)),
        awkward_seeds(),
    )
    .or_fail()?;
    ensure_eq!(
        applied(&frame, &noise, FrameIndex::new(9))?,
        applied(&frame, &noise, FrameIndex::new(9))?
    );
    Ok(())
}

#[test]
fn temporal_seed_changes_a_read_noise_frame() {
    assert_eq!(check_temporal_seed_changes_a_read_noise_frame(), Ok(()));
}

fn check_temporal_seed_changes_a_read_noise_frame() -> Result<(), Fail> {
    let frame = flat_frame(
        Size {
            columns: Width::new(17).or_fail()?,
            rows: Height::new(31).or_fail()?,
        },
        Channel(128),
    )?;
    let first = read_only(
        ReadSigma(Q8_8(1_280)),
        seeds(temporal(Seed(0x1357_9BDF_0246_8ACE)), pattern(Seed(7))),
    )?;
    let second = read_only(
        ReadSigma(Q8_8(1_280)),
        seeds(temporal(Seed(0x2468_ACE0_1357_9BDF)), pattern(Seed(7))),
    )?;
    ensure_ne!(
        applied(&frame, &first, FrameIndex::new(2))?,
        applied(&frame, &second, FrameIndex::new(2))?
    );
    Ok(())
}

#[test]
fn pattern_seed_does_not_change_a_read_noise_frame() {
    assert_eq!(
        check_pattern_seed_does_not_change_a_read_noise_frame(),
        Ok(())
    );
}

fn check_pattern_seed_does_not_change_a_read_noise_frame() -> Result<(), Fail> {
    let frame = flat_frame(
        Size {
            columns: Width::new(17).or_fail()?,
            rows: Height::new(31).or_fail()?,
        },
        Channel(128),
    )?;
    let first = read_only(
        ReadSigma(Q8_8(1_280)),
        seeds(temporal(Seed(0x1357_9BDF_0246_8ACE)), pattern(Seed(11))),
    )?;
    let second = read_only(
        ReadSigma(Q8_8(1_280)),
        seeds(temporal(Seed(0x1357_9BDF_0246_8ACE)), pattern(Seed(29))),
    )?;
    ensure_eq!(
        applied(&frame, &first, FrameIndex::new(2))?,
        applied(&frame, &second, FrameIndex::new(2))?
    );
    Ok(())
}

#[test]
fn pattern_seed_changes_a_fixed_pattern_frame() {
    assert_eq!(check_pattern_seed_changes_a_fixed_pattern_frame(), Ok(()));
}

fn check_pattern_seed_changes_a_fixed_pattern_frame() -> Result<(), Fail> {
    let frame = flat_frame(
        Size {
            columns: Width::new(17).or_fail()?,
            rows: Height::new(31).or_fail()?,
        },
        Channel(128),
    )?;
    let prnu_first = prnu_only(
        PrnuSpread(40_000),
        seeds(temporal(Seed(5)), pattern(Seed(11))),
    )?;
    let prnu_second = prnu_only(
        PrnuSpread(40_000),
        seeds(temporal(Seed(5)), pattern(Seed(29))),
    )?;
    ensure_ne!(
        applied(&frame, &prnu_first, FrameIndex::new(0))?,
        applied(&frame, &prnu_second, FrameIndex::new(0))?
    );
    let column_first = column_only(
        ColumnSigma(Q8_8(600)),
        seeds(temporal(Seed(5)), pattern(Seed(11))),
    )?;
    let column_second = column_only(
        ColumnSigma(Q8_8(600)),
        seeds(temporal(Seed(5)), pattern(Seed(29))),
    )?;
    ensure_ne!(
        applied(&frame, &column_first, FrameIndex::new(0))?,
        applied(&frame, &column_second, FrameIndex::new(0))?
    );
    Ok(())
}

#[test]
fn fixed_pattern_is_identical_on_every_frame() {
    assert_eq!(check_fixed_pattern_is_identical_on_every_frame(), Ok(()));
}

fn check_fixed_pattern_is_identical_on_every_frame() -> Result<(), Fail> {
    let frame = flat_frame(
        Size {
            columns: Width::new(17).or_fail()?,
            rows: Height::new(31).or_fail()?,
        },
        Channel(128),
    )?;
    let prnu = prnu_only(PrnuSpread(40_000), awkward_seeds())?;
    ensure_eq!(
        applied(&frame, &prnu, FrameIndex::new(0))?,
        applied(&frame, &prnu, FrameIndex::new(7))?
    );
    let column = column_only(ColumnSigma(Q8_8(600)), awkward_seeds())?;
    ensure_eq!(
        applied(&frame, &column, FrameIndex::new(0))?,
        applied(&frame, &column, FrameIndex::new(7))?
    );
    let read = read_only(ReadSigma(Q8_8(1_280)), awkward_seeds())?;
    ensure_ne!(
        applied(&frame, &read, FrameIndex::new(0))?,
        applied(&frame, &read, FrameIndex::new(7))?
    );
    Ok(())
}

#[test]
fn equal_seeds_keep_pattern_and_temporal_draws_apart() {
    assert_eq!(
        check_equal_seeds_keep_pattern_and_temporal_draws_apart(),
        Ok(())
    );
}

fn check_equal_seeds_keep_pattern_and_temporal_draws_apart() -> Result<(), Fail> {
    // With the pattern seed and the temporal seed equal, a fixed-pattern draw and a temporal draw
    // at the same sensel must still come from different streams, so their draws differ.
    let shared = Seed(0x0123_4567_89AB_CDEF);
    let frame = FrameIndex::new(0);
    // Sensel 0 maps to the base, 2^42, and sensel 5 to 2^42 + 5, both written out by hand.
    ensure_eq!(
        temporal_stream_of(StreamIndex(0)).or_fail()?,
        StreamIndex(0x400_0000_0000)
    );
    ensure_eq!(
        temporal_stream_of(StreamIndex(5)).or_fail()?,
        StreamIndex(0x400_0000_0005)
    );
    for sensel in [0_u64, 1, 2] {
        let pattern_stream = StreamIndex(sensel);
        let temporal_stream = temporal_stream_of(pattern_stream).or_fail()?;
        ensure_ne!(
            stream_seed(shared, frame, pattern_stream),
            stream_seed(shared, frame, temporal_stream),
            "sensel {sensel}"
        );
        ensure_ne!(
            draw_on(shared, frame, pattern_stream)?,
            draw_on(shared, frame, temporal_stream)?,
            "sensel {sensel}"
        );
    }
    Ok(())
}

#[test]
fn column_noise_is_constant_down_each_column() {
    assert_eq!(check_column_noise_is_constant_down_each_column(), Ok(()));
}

fn check_column_noise_is_constant_down_each_column() -> Result<(), Fail> {
    let size = Size {
        columns: Width::new(17).or_fail()?,
        rows: Height::new(31).or_fail()?,
    };
    let frame = flat_frame(size, Channel(128))?;
    let out = applied(
        &frame,
        &column_only(ColumnSigma(Q8_8(600)), awkward_seeds())?,
        FrameIndex::new(4),
    )?;
    let mut tops = Vec::new();
    for column in 0..size.columns.get().get() {
        let top = pixel(&out, PixelCoord::new(PixelColumn(column), PixelRow(0)))?;
        for row in 0..size.rows.get().get() {
            let at = PixelCoord::new(PixelColumn(column), PixelRow(row));
            ensure_eq!(pixel(&out, at)?, top, "column {column} row {row}");
        }
        tops.push(top);
    }
    let first = tops.first().copied().or_fail()?;
    ensure!(
        tops.iter().any(|color| *color != first),
        "column offsets must differ between columns"
    );
    Ok(())
}

#[test]
fn frame_order_does_not_change_any_frame() {
    assert_eq!(check_frame_order_does_not_change_any_frame(), Ok(()));
}

fn check_frame_order_does_not_change_any_frame() -> Result<(), Fail> {
    let frame = flat_frame(
        Size {
            columns: Width::new(17).or_fail()?,
            rows: Height::new(31).or_fail()?,
        },
        Channel(128),
    )?;
    let noise = SensorNoise::new(
        ShotGain(Q8_8(64)),
        ReadSigma(Q8_8(900)),
        PrnuSpread(3_001),
        ColumnSigma(Q8_8(77)),
        awkward_seeds(),
    )
    .or_fail()?;
    let five_first = applied(&frame, &noise, FrameIndex::new(5))?;
    let three_after = applied(&frame, &noise, FrameIndex::new(3))?;
    let three_first = applied(&frame, &noise, FrameIndex::new(3))?;
    let five_after = applied(&frame, &noise, FrameIndex::new(5))?;
    ensure_eq!(five_first, five_after);
    ensure_eq!(three_after, three_first);
    ensure_ne!(five_first, three_after);
    Ok(())
}

#[test]
fn read_noise_has_the_stated_mean_and_spread() {
    assert_eq!(check_read_noise_has_the_stated_mean_and_spread(), Ok(()));
}

fn check_read_noise_has_the_stated_mean_and_spread() -> Result<(), Fail> {
    // Read sigma 1280 raw is 5.0 code values over a flat 128 frame.
    let size = Size {
        columns: Width::new(64).or_fail()?,
        rows: Height::new(64).or_fail()?,
    };
    let frame = flat_frame(size, Channel(128))?;
    let out = applied(
        &frame,
        &read_only(ReadSigma(Q8_8(1_280)), awkward_seeds())?,
        FrameIndex::new(1),
    )?;
    let mut greens = Vec::new();
    for row in 0..size.rows.get().get() {
        for column in 0..size.columns.get().get() {
            let at = PixelCoord::new(PixelColumn(column), PixelRow(row));
            greens.push(pixel(&out, at)?.green.0);
        }
    }
    let count = u64::try_from(greens.len()).or_fail()?;
    let sum: u64 = greens.iter().map(|&green| u64::from(green)).sum();
    let mean = sum.checked_div(count).or_fail()?;
    ensure!(
        (127..=129).contains(&mean),
        "mean green {mean} must lie in 127..=129"
    );
    let squared = greens.iter().try_fold(0_u64, |total, &green| {
        let deviation = u64::from(green.abs_diff(128));
        total.checked_add(deviation.checked_mul(deviation)?)
    });
    let sum_squared = squared.or_fail()?;
    let variance = sum_squared.checked_div(count).or_fail()?;
    let spread = isqrt(IntSqrtInput(variance)).0;
    ensure!(
        (4..=6).contains(&spread),
        "standard deviation {spread} must lie in 4..=6"
    );
    Ok(())
}
