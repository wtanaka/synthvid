use super::*;
use core::num::NonZeroU64;

/// The probe frame is five sensels across and three down, so it is not square.
fn probe_size() -> Result<Size, Fail> {
    Ok(Size {
        columns: Width::new(5).or_fail()?,
        rows: Height::new(3).or_fail()?,
    })
}

/// The clean colors of the probe frame, row by row, each as red, green and blue. Every channel
/// differs from its neighbors, so a sensel that reads another stream records a different value.
const PROBE: [[[Channel; 3]; 5]; 3] = [
    [
        [Channel(60), Channel(140), Channel(90)],
        [Channel(150), Channel(45), Channel(175)],
        [Channel(75), Channel(120), Channel(160)],
        [Channel(110), Channel(70), Channel(55)],
        [Channel(95), Channel(160), Channel(130)],
    ],
    [
        [Channel(130), Channel(85), Channel(40)],
        [Channel(55), Channel(165), Channel(110)],
        [Channel(170), Channel(95), Channel(120)],
        [Channel(48), Channel(135), Channel(150)],
        [Channel(140), Channel(60), Channel(80)],
    ],
    [
        [Channel(100), Channel(30), Channel(145)],
        [Channel(65), Channel(115), Channel(70)],
        [Channel(125), Channel(175), Channel(105)],
        [Channel(88), Channel(52), Channel(165)],
        [Channel(158), Channel(100), Channel(62)],
    ],
];

/// The probe frame, with the clean colors of `PROBE`.
fn probe_frame() -> Result<Frame, Fail> {
    let size = probe_size()?;
    let mut frame = Frame::zeroed(dims(size)).or_fail()?;
    for row in 0..size.rows.get().get() {
        for column in 0..size.columns.get().get() {
            let colors = PROBE
                .get(usize::from(row))
                .and_then(|line| line.get(usize::from(column)))
                .or_fail()?;
            let [red, green, blue] = *colors;
            let coord = PixelCoord::new(PixelColumn(column), PixelRow(row));
            frame
                .set_pixel(coord, Rgb8 { red, green, blue })
                .or_fail()?;
        }
    }
    Ok(frame)
}

/// Which light the shot noise of a channel is driven by.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ShotBasis {
    /// The channel's light after its sensel's gain, rounded to a whole code value, floored at zero
    /// and not clamped above, so it can exceed 255.
    GainedLight,
    /// The clean channel value, before the gain.
    CleanValue,
}

/// What the expected values of a frame are computed under.
#[derive(Copy, Clone, Debug)]
struct Setup<'noise> {
    /// The noise the sensor adds.
    noise: &'noise SensorNoise,
    /// The frame being recorded.
    frame: FrameIndex,
    /// The light the shot noise is driven by.
    basis: ShotBasis,
    /// The number of sensels across the frame.
    width: Width,
}

/// The fixed-pattern stream and the temporal stream of one channel of one sensel.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Streams {
    /// The fixed-pattern stream, `(row * width + column) * 3 + plane`.
    fixed: StreamIndex,
    /// The temporal stream, which is the fixed-pattern stream offset by 2^42.
    temporal: StreamIndex,
}

/// The streams of one channel of the sensel at the given column and row of a frame that is
/// `width` sensels wide. The planes are numbered red 0, green 1 and blue 2.
fn streams_of(
    column: PixelColumn,
    row: PixelRow,
    width: Width,
    plane: ColorPlane,
) -> Result<Streams, Fail> {
    let PixelColumn(column_index) = column;
    let PixelRow(row_index) = row;
    let plane_number = match plane {
        ColorPlane::Red => 0_u64,
        ColorPlane::Green => 1,
        ColorPlane::Blue => 2,
    };
    let fixed = u64::from(row_index)
        .checked_mul(NonZeroU64::from(width.get()).get())
        .and_then(|offset| offset.checked_add(u64::from(column_index)))
        .and_then(|sensel| sensel.checked_mul(3))
        .and_then(|base| base.checked_add(plane_number))
        .or_fail()?;
    let temporal = 0x400_0000_0000_u64.checked_add(fixed).or_fail()?;
    Ok(Streams {
        fixed: StreamIndex(fixed),
        temporal: StreamIndex(temporal),
    })
}

/// The gain of a sensel from the fixed pattern: `65536 + spread * g / 256`, floored, where `g` is
/// the first normal draw of the fixed-pattern stream at frame 0.
fn expected_gain(prnu: PrnuSpread, pattern: Seed, fixed: StreamIndex) -> Result<Gain, Fail> {
    let PrnuSpread(spread) = prnu;
    let NormalDraw(draw) = draw_on(pattern, FrameIndex::new(0), fixed)?;
    let deviation = i64::from(spread)
        .checked_mul(i64::from(draw))
        .and_then(|product| product.checked_div_euclid(256))
        .or_fail()?;
    let gain = 0x1_0000_i64.checked_add(deviation).or_fail()?;
    Ok(Gain(gain))
}

/// The light at a sensel: `value * 256 * gain / 65536` in Q8.8, floored.
fn expected_level(value: Channel, gain: Gain) -> Result<Level, Fail> {
    let Channel(code) = value;
    let Gain(gain_raw) = gain;
    let level = i64::from(code)
        .checked_mul(256)
        .and_then(|scaled| scaled.checked_mul(gain_raw))
        .and_then(|product| product.checked_div_euclid(0x1_0000))
        .or_fail()?;
    Ok(Level(level))
}

/// The shot standard deviation for a light, in raw Q8.8: the integer square root of
/// `slope * light * 256`.
fn expected_sigma(slope: ShotGain, light: Light) -> Result<Amplitude, Fail> {
    let ShotGain(Q8_8(slope_raw)) = slope;
    let Light(codes) = light;
    let variance = u64::from(slope_raw)
        .checked_mul(codes)
        .and_then(|product| product.checked_mul(256))
        .or_fail()?;
    Ok(Amplitude(isqrt(IntSqrtInput(variance)).0))
}

/// An amplitude in raw Q8.8 times a normal draw, floored: `amplitude * draw / 256`.
fn expected_scaled(amplitude: Amplitude, draw: NormalDraw) -> Result<SignedQ8, Fail> {
    let Amplitude(raw) = amplitude;
    let NormalDraw(draw_raw) = draw;
    i64::try_from(raw)
        .or_fail()?
        .checked_mul(i64::from(draw_raw))
        .and_then(|product| product.checked_div_euclid(256))
        .map(SignedQ8)
        .or_fail()
}

/// The shot draw and the read draw of a temporal stream. The shot draw is taken first.
fn temporal_draws(
    seed: Seed,
    frame: FrameIndex,
    stream: StreamIndex,
) -> Result<(NormalDraw, NormalDraw), Fail> {
    let mut rng = Rng::from_seed(stream_seed(seed, frame, stream));
    let shot = normal_draw(&mut rng).or_fail()?;
    let read = normal_draw(&mut rng).or_fail()?;
    Ok((shot, read))
}

/// The unclamped light of a sensel after its gain: the level rounded to a whole code value, ties
/// toward positive infinity, floored at zero, and not clamped above.
fn expected_light(total: SignedQ8) -> Result<Light, Fail> {
    let SignedQ8(raw) = total;
    let rounded = raw
        .checked_add(128)
        .and_then(|sum| sum.checked_div_euclid(256))
        .or_fail()?;
    let codes = u64::try_from(rounded.max(0)).or_fail()?;
    Ok(Light(codes))
}

/// Rounds a Q8.8 total to a code value, ties toward positive infinity, and clamps it to 0 to 255.
fn expected_code(total: SignedQ8) -> Result<Channel, Fail> {
    let SignedQ8(raw) = total;
    let rounded = raw
        .checked_add(128)
        .and_then(|sum| sum.checked_div_euclid(256))
        .or_fail()?;
    let byte = u8::try_from(rounded.clamp(0, 255)).or_fail()?;
    Ok(Channel(byte))
}

/// The recorded value of one channel, from the documented formulas and the draws of its streams.
///
/// The shot noise is driven by the light of the setup's basis, and the sum of the level and the
/// three noise terms is saturated to 0 to 255 once, at the end.
fn expected_channel(value: Channel, streams: Streams, setup: Setup<'_>) -> Result<Channel, Fail> {
    let noise = setup.noise;
    let gain = expected_gain(noise.prnu(), noise.pattern_seed().get(), streams.fixed)?;
    let level = expected_level(value, gain)?;
    let Level(level_raw) = level;
    let Channel(clean) = value;
    let light = match setup.basis {
        ShotBasis::GainedLight => expected_light(SignedQ8(level_raw))?,
        ShotBasis::CleanValue => Light(u64::from(clean)),
    };
    let sigma = expected_sigma(noise.shot(), light)?;
    let (shot_draw, read_draw) =
        temporal_draws(noise.temporal_seed().get(), setup.frame, streams.temporal)?;
    let shot = expected_scaled(sigma, shot_draw)?;
    let ReadSigma(Q8_8(read_deviation)) = noise.read();
    let read = expected_scaled(Amplitude(u64::from(read_deviation)), read_draw)?;
    let total = level_raw
        .checked_add(shot.0)
        .and_then(|sum| sum.checked_add(read.0))
        .or_fail()?;
    expected_code(SignedQ8(total))
}

/// The frame the sensor should record from `source`, sensel by sensel.
fn expected_frame(source: &Frame, setup: Setup<'_>) -> Result<Frame, Fail> {
    let dimensions = source.dimensions();
    let mut expected = Frame::zeroed(dimensions).or_fail()?;
    for row in 0..dimensions.height.get().get() {
        for column in 0..dimensions.width.get().get() {
            let coord = PixelCoord::new(PixelColumn(column), PixelRow(row));
            let clean = pixel(source, coord)?;
            let at_column = PixelColumn(column);
            let at_row = PixelRow(row);
            let recorded = Rgb8 {
                red: expected_channel(
                    clean.red,
                    streams_of(at_column, at_row, setup.width, ColorPlane::Red)?,
                    setup,
                )?,
                green: expected_channel(
                    clean.green,
                    streams_of(at_column, at_row, setup.width, ColorPlane::Green)?,
                    setup,
                )?,
                blue: expected_channel(
                    clean.blue,
                    streams_of(at_column, at_row, setup.width, ColorPlane::Blue)?,
                    setup,
                )?,
            };
            expected.set_pixel(coord, recorded).or_fail()?;
        }
    }
    Ok(expected)
}

/// Noise with a fixed-pattern gain and a shot slope, and no read noise or column offsets.
fn gain_and_shot(slope: ShotGain) -> Result<SensorNoise, Fail> {
    SensorNoise::new(
        slope,
        ReadSigma(Q8_8(0)),
        PrnuSpread(40_000),
        ColumnSigma(Q8_8(0)),
        awkward_seeds(),
    )
    .or_fail()
}

/// The light after the gain is the basis of the shot noise: the hand-computed sigma of a gained
/// sensel differs from the sigma of its clean value, and the sensor records the former.
#[test]
fn shot_noise_follows_the_light_after_the_gain() {
    assert_eq!(check_shot_noise_follows_the_light_after_the_gain(), Ok(()));
}

fn check_shot_noise_follows_the_light_after_the_gain() -> Result<(), Fail> {
    // Clean value 77 with gain 52000 is level 15640 raw, which is 61.09 code values, so the light
    // is 61. The variance is 1000 * 61 * 256 = 15616000, and 3951 * 3951 = 15610401 is at most that
    // while 3952 * 3952 = 15618304 is above it, so the sigma is 3951 raw. The clean value gives
    // 1000 * 77 * 256 = 19712000, whose root is 4439.
    let gain = Gain(52_000);
    let level = level_of(Channel(77), gain).or_fail()?;
    ensure_eq!(level, Level(15_640));
    let light = round_to_channel(SignedQ8(15_640)).or_fail()?;
    ensure_eq!(light, Channel(61));
    ensure_eq!(
        shot_sigma(ShotGain(Q8_8(1_000)), Light(61)).or_fail()?,
        Amplitude(3_951)
    );
    ensure_eq!(
        shot_sigma(ShotGain(Q8_8(1_000)), Light(77)).or_fail()?,
        Amplitude(4_439)
    );

    let source = probe_frame()?;
    let noise = gain_and_shot(ShotGain(Q8_8(256)))?;
    let frame = FrameIndex::new(3);
    let width = Width::new(5).or_fail()?;
    let gained = Setup {
        noise: &noise,
        frame,
        basis: ShotBasis::GainedLight,
        width,
    };
    let clean = Setup {
        basis: ShotBasis::CleanValue,
        ..gained
    };
    let expected = expected_frame(&source, gained)?;
    let old = expected_frame(&source, clean)?;
    ensure_ne!(expected, old);
    ensure_eq!(applied(&source, &noise, frame)?, expected);
    Ok(())
}

/// Each channel of each sensel of a 5 by 3 frame draws its gain and its temporal terms from its
/// own streams, so the whole frame matches the documented stream numbering.
#[test]
fn every_sensel_draws_from_its_own_streams() {
    assert_eq!(check_every_sensel_draws_from_its_own_streams(), Ok(()));
}

fn check_every_sensel_draws_from_its_own_streams() -> Result<(), Fail> {
    let source = probe_frame()?;
    let noise = SensorNoise::new(
        ShotGain(Q8_8(0)),
        ReadSigma(Q8_8(1_280)),
        PrnuSpread(40_000),
        ColumnSigma(Q8_8(0)),
        awkward_seeds(),
    )
    .or_fail()?;
    let frame = FrameIndex::new(2);
    let width = Width::new(5).or_fail()?;
    let setup = Setup {
        noise: &noise,
        frame,
        basis: ShotBasis::GainedLight,
        width,
    };
    ensure_eq!(
        applied(&source, &noise, frame)?,
        expected_frame(&source, setup)?
    );

    // The blue channel of the sensel at column 3, row 1 uses the streams 26 and 2^42 + 26.
    let blue = streams_of(PixelColumn(3), PixelRow(1), width, ColorPlane::Blue)?;
    ensure_eq!(blue.fixed, StreamIndex(26));
    ensure_eq!(blue.temporal, StreamIndex(0x400_0000_001A));
    let blue_value = PROBE[1][3][2];
    let correct = expected_channel(blue_value, blue, setup)?;
    // Column and row transposed: (3 * 5 + 1) * 3 + 2 = 50.
    let transposed = streams_of(PixelColumn(1), PixelRow(3), width, ColorPlane::Blue)?;
    ensure_ne!(correct, expected_channel(blue_value, transposed, setup)?);
    // The red offset in place of the blue one: (1 * 5 + 3) * 3 + 0 = 24.
    let red_offset = streams_of(PixelColumn(3), PixelRow(1), width, ColorPlane::Red)?;
    ensure_ne!(correct, expected_channel(blue_value, red_offset, setup)?);
    let rendered = applied(&source, &noise, frame)?;
    ensure_eq!(
        pixel(&rendered, PixelCoord::new(PixelColumn(3), PixelRow(1)))?.blue,
        correct
    );
    Ok(())
}

/// The sensel index is row-major, and a column or row outside the frame has no index.
#[test]
fn sensel_index_is_row_major_inside_the_frame() {
    assert_eq!(check_sensel_index_is_row_major_inside_the_frame(), Ok(()));
}

fn check_sensel_index_is_row_major_inside_the_frame() -> Result<(), Fail> {
    let dimensions = dims(probe_size()?);
    ensure_eq!(
        SenselIndex::of_sensel(PixelColumn(3), PixelRow(1), dimensions),
        Ok(SenselIndex(8))
    );
    ensure_eq!(
        SenselIndex::of_sensel(PixelColumn(4), PixelRow(2), dimensions),
        Ok(SenselIndex(14))
    );
    ensure_eq!(
        SenselIndex::of_sensel(PixelColumn(5), PixelRow(0), dimensions),
        Err(SensorError::PixelOutOfBounds)
    );
    ensure_eq!(
        SenselIndex::of_sensel(PixelColumn(0), PixelRow(3), dimensions),
        Err(SensorError::PixelOutOfBounds)
    );
    Ok(())
}
