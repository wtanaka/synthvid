use super::*;
use core::num::{NonZeroI64, NonZeroUsize};
use synthvid_scene::{normal_draw, stream_seed, NormalDraw, Rng, StreamIndex};

/// The filtered field of one channel, from the documented stream layout.
///
/// Padded row `r` of a channel with stream base `channel` is drawn from the stream
/// `channel * padded_rows + r`, one normal draw per padded column in column order. Each field
/// value is the sum of the `window * window` draws under it, divided by the window, floored.
fn expected_field(
    seed: Seed,
    frame: FrameIndex,
    channel: StreamIndex,
    dimensions: Dimensions,
    window: GrainWindow,
) -> Result<Vec<Vec<SignedQ8>>, Fail> {
    let width = NonZeroUsize::from(dimensions.width.get()).get();
    let height = NonZeroUsize::from(dimensions.height.get()).get();
    let span = NonZeroUsize::from(window.get()).get();
    let padded_columns = width
        .checked_add(span)
        .and_then(|sum| sum.checked_sub(1))
        .or_fail()?;
    let padded_rows = height
        .checked_add(span)
        .and_then(|sum| sum.checked_sub(1))
        .or_fail()?;
    let row_count = u64::try_from(padded_rows).or_fail()?;
    let base = channel.0.checked_mul(row_count).or_fail()?;

    let mut draws: Vec<Vec<i64>> = Vec::new();
    for padded_row in 0..padded_rows {
        let offset = u64::try_from(padded_row).or_fail()?;
        let stream = base.checked_add(offset).map(StreamIndex).or_fail()?;
        let mut rng = Rng::from_seed(stream_seed(seed, frame, stream));
        let mut row_draws = Vec::new();
        for _ in 0..padded_columns {
            let NormalDraw(raw) = normal_draw(&mut rng).or_fail()?;
            row_draws.push(i64::from(raw));
        }
        draws.push(row_draws);
    }

    let divisor = NonZeroI64::from(window.get()).get();
    let mut field = Vec::new();
    for band in draws.windows(span) {
        let mut line = Vec::new();
        for column in 0..width {
            let mut sum: i64 = 0;
            for row_draws in band {
                let run = row_draws.windows(span).nth(column).or_fail()?;
                for &draw in run {
                    sum = sum.checked_add(draw).or_fail()?;
                }
            }
            let average = sum.checked_div_euclid(divisor).or_fail()?;
            line.push(SignedQ8(average));
        }
        field.push(line);
    }
    Ok(field)
}

/// The blue field of a 5 by 3 frame with a window of three sensels matches the documented stream
/// layout at every sensel. At the asymmetric sensel in column 3 of row 1, it differs from the red
/// field, so reading the wrong channel's streams changes the value.
#[test]
fn blue_grain_field_draws_from_its_padded_row_streams() {
    assert_eq!(
        check_blue_grain_field_draws_from_its_padded_row_streams(),
        Ok(())
    );
}

fn check_blue_grain_field_draws_from_its_padded_row_streams() -> Result<(), Fail> {
    let dimensions = Dimensions::new(Width::new(5).or_fail()?, Height::new(3).or_fail()?);
    let window = GrainWindow(NonZeroU16::new(3).or_fail()?);
    let seed = Seed(0x0BAD_5EED_1234_5677);
    let frame = FrameIndex::new(4);
    let shape = FieldShape {
        seed,
        index: frame,
        plane: ColorPlane::Blue,
        dimensions,
        window,
    };
    let actual = field::filtered_noise(shape).or_fail()?;
    // Blue is the third channel, so its stream base is 2 * 5 padded rows.
    let expected = expected_field(seed, frame, StreamIndex(2), dimensions, window)?;
    ensure_eq!(actual, expected);

    // The red channel reads the streams from base 0.
    let red = expected_field(seed, frame, StreamIndex(0), dimensions, window)?;
    let blue_at = expected.get(1).and_then(|line| line.get(3)).or_fail()?;
    let red_at = red.get(1).and_then(|line| line.get(3)).or_fail()?;
    ensure_ne!(blue_at, red_at);
    Ok(())
}
