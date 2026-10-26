use super::*;
use crate::medium::{GrainDensityError, GrainSeed};
use crate::testutil::Frac;
use synthvid_scene::{Seed, Width};
use synthvid_testkit::{ensure, ensure_eq, ensure_ne, Fail, OrFail as _};

mod stream;

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

/// The grain radius that is the given fraction of the frame height.
fn radius(fraction: Frac) -> Result<GrainRadius, Fail> {
    GrainRadius::new(fraction.ratio()?).or_fail()
}

/// Grain with the given radius and density, and neither spread nor blur.
fn plain_grain(radius: GrainRadius, density: GrainDensity, seed: GrainSeed) -> FilmGrain {
    FilmGrain::new(
        radius,
        GrainSpread(Q8_8(0)),
        density,
        GrainBlur(Q8_8(0)),
        seed,
    )
}

/// Grain with a spread, a blur and a density of 300, with the given radius and seed.
fn awkward_grain(radius: GrainRadius, seed: GrainSeed) -> Result<FilmGrain, Fail> {
    Ok(FilmGrain::new(
        radius,
        GrainSpread(Q8_8(40)),
        GrainDensity::new(NonZeroU16::new(300).or_fail()?).or_fail()?,
        GrainBlur(Q8_8(12)),
        seed,
    ))
}

/// A copy of `frame` with the grain applied as the frame at `index`.
fn applied(frame: &Frame, grain: &FilmGrain, index: FrameIndex) -> Result<Frame, Fail> {
    let mut copy = frame.clone();
    apply_film_grain(&mut copy, grain, index).or_fail()?;
    Ok(copy)
}

/// The green value at the given coordinate.
fn green_at(frame: &Frame, coord: PixelCoord) -> Result<Channel, Fail> {
    Ok(frame.get_pixel(coord).or_fail()?.green)
}

/// The green values of a frame, row by row from the top.
fn green_rows(frame: &Frame) -> Result<Vec<Vec<Channel>>, Fail> {
    let dimensions = frame.dimensions();
    let mut rows = Vec::new();
    for row in 0..dimensions.height.get().get() {
        let mut line = Vec::new();
        for column in 0..dimensions.width.get().get() {
            line.push(green_at(
                frame,
                PixelCoord::new(PixelColumn(column), PixelRow(row)),
            )?);
        }
        rows.push(line);
    }
    Ok(rows)
}

/// The number of sensels a statistic is summed over.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Count(pub u64);

/// The sample mean of the green values, floored.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Mean(pub u64);

/// A sum of squared deviations from the mean.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Energy(pub i64);

/// A sum of products of each deviation with the deviation of the sensel to its right.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Covariance(pub i64);

/// The moments of the green channel of a frame, computed with checked arithmetic.
#[derive(Copy, Clone, Debug)]
struct GreenStatistics {
    /// The number of sensels summed.
    count: Count,
    /// The sample mean, floored.
    mean: Mean,
    /// The sum of the squared deviations from the mean.
    energy: Energy,
    /// The lag-one sum of products of neighboring deviations, within each row.
    lag_one: Covariance,
}

/// The green-channel statistics of a frame.
fn green_statistics(frame: &Frame) -> Result<GreenStatistics, Fail> {
    let rows = green_rows(frame)?;
    let count = Count(u64::try_from(rows.iter().flatten().count()).or_fail()?);
    let total = rows
        .iter()
        .flatten()
        .try_fold(0_u64, |sum, green| sum.checked_add(u64::from(green.0)))
        .or_fail()?;
    let mean = total.checked_div(count.0).or_fail()?;
    let center = i64::try_from(mean).or_fail()?;
    let mut energy = 0_i64;
    let mut lag_one = 0_i64;
    for row in &rows {
        let deviations = row
            .iter()
            .map(|green| i64::from(green.0).checked_sub(center).or_fail())
            .collect::<Result<Vec<i64>, Fail>>()?;
        for &deviation in &deviations {
            let square = deviation.checked_mul(deviation).or_fail()?;
            energy = energy.checked_add(square).or_fail()?;
        }
        for (&left, &right) in deviations.iter().zip(deviations.iter().skip(1)) {
            let product = left.checked_mul(right).or_fail()?;
            lag_one = lag_one.checked_add(product).or_fail()?;
        }
    }
    Ok(GreenStatistics {
        count,
        mean: Mean(mean),
        energy: Energy(energy),
        lag_one: Covariance(lag_one),
    })
}

#[test]
fn grain_window_of_a_fine_grain_is_eleven_sensels() {
    assert_eq!(
        check_grain_window_of_a_fine_grain_is_eleven_sensels(),
        Ok(())
    );
}

fn check_grain_window_of_a_fine_grain_is_eleven_sensels() -> Result<(), Fail> {
    // 2 * 1 * 1080 * 256 * 256 + 200 * 32768 = 148111360, over 200 * 65536 = 13107200, is 11.3.
    let window = grain_window(
        radius(Frac(1, 200))?,
        GrainSpread(Q8_8(0)),
        GrainBlur(Q8_8(0)),
        Height::new(1080).or_fail()?,
    )
    .or_fail()?;
    ensure_eq!(window.get().get(), 11);
    Ok(())
}

#[test]
fn grain_window_with_spread_and_blur_is_one_sensel_on_a_short_frame() {
    assert_eq!(
        check_grain_window_with_spread_and_blur_is_one_sensel_on_a_short_frame(),
        Ok(())
    );
}

fn check_grain_window_with_spread_and_blur_is_one_sensel_on_a_short_frame() -> Result<(), Fail> {
    // sqrt_term is isqrt(131072) = 362, the numerator is 48793088 and the denominator 45875200.
    let window = grain_window(
        radius(Frac(3, 700))?,
        GrainSpread(Q8_8(256)),
        GrainBlur(Q8_8(128)),
        Height::new(31).or_fail()?,
    )
    .or_fail()?;
    ensure_eq!(window.get().get(), 1);
    Ok(())
}

#[test]
fn grain_window_grows_with_the_frame_height() {
    assert_eq!(check_grain_window_grows_with_the_frame_height(), Ok(()));
}

fn check_grain_window_grows_with_the_frame_height() -> Result<(), Fail> {
    // The numerator is 856985600 and the denominator 45875200, a quotient of 18.68.
    let window = grain_window(
        radius(Frac(3, 700))?,
        GrainSpread(Q8_8(256)),
        GrainBlur(Q8_8(128)),
        Height::new(1000).or_fail()?,
    )
    .or_fail()?;
    ensure_eq!(window.get().get(), 18);
    Ok(())
}

#[test]
fn grain_window_wider_than_256_sensels_is_too_coarse() {
    assert_eq!(
        check_grain_window_wider_than_256_sensels_is_too_coarse(),
        Ok(())
    );
}

fn check_grain_window_wider_than_256_sensels_is_too_coarse() -> Result<(), Fail> {
    let result = grain_window(
        radius(Frac(1, 2))?,
        GrainSpread(Q8_8(0)),
        GrainBlur(Q8_8(0)),
        Height::new(1080).or_fail()?,
    );
    ensure_eq!(result, Err(GrainError::GrainTooCoarse));
    Ok(())
}

#[test]
fn grain_sigma_is_zero_at_black_and_white() {
    assert_eq!(check_grain_sigma_is_zero_at_black_and_white(), Ok(()));
}

fn check_grain_sigma_is_zero_at_black_and_white() -> Result<(), Fail> {
    let density = GrainDensity::new(NonZeroU16::new(256).or_fail()?).or_fail()?;
    ensure_eq!(grain_sigma(density, Channel(0)).or_fail()?, Q8_8(0));
    ensure_eq!(grain_sigma(density, Channel(255)).or_fail()?, Q8_8(0));
    Ok(())
}

#[test]
fn grain_sigma_at_mid_gray_matches_the_hand_computed_value() {
    assert_eq!(
        check_grain_sigma_at_mid_gray_matches_the_hand_computed_value(),
        Ok(())
    );
}

fn check_grain_sigma_at_mid_gray_matches_the_hand_computed_value() -> Result<(), Fail> {
    // 128 * 127 * 65536 = 1065353216, whose root is 32639; 256 * 32639 / 4096 = 2039.
    let density = GrainDensity::new(NonZeroU16::new(256).or_fail()?).or_fail()?;
    ensure_eq!(grain_sigma(density, Channel(128)).or_fail()?, Q8_8(2039));
    Ok(())
}

#[test]
fn grain_sigma_scales_with_density_and_floors() {
    assert_eq!(check_grain_sigma_scales_with_density_and_floors(), Ok(()));
}

fn check_grain_sigma_scales_with_density_and_floors() -> Result<(), Fail> {
    // 10 * 245 * 65536 = 160563200, whose root is 12671; 300 * 12671 / 4096 = 928.04.
    let density = GrainDensity::new(NonZeroU16::new(300).or_fail()?).or_fail()?;
    ensure_eq!(grain_sigma(density, Channel(10)).or_fail()?, Q8_8(928));
    Ok(())
}

#[test]
fn grain_sigma_at_the_density_limit_fits_q8_8() {
    assert_eq!(check_grain_sigma_at_the_density_limit_fits_q8_8(), Ok(()));
}

fn check_grain_sigma_at_the_density_limit_fits_q8_8() -> Result<(), Fail> {
    // At value 127 the root is isqrt(127 * 128 * 65536) = 32639, since 32639 * 32639 = 1065304321
    // is at most 1065353216 and 32640 * 32640 = 1065369600 is above it. Then
    // 8224 * 32639 / 4096 = 65532 with a remainder of 4064, so the sigma is 65532, which fits.
    // Value 128 has the same product, so it gives the same sigma.
    let limit = GrainDensity::new(NonZeroU16::new(8224).or_fail()?).or_fail()?;
    ensure_eq!(grain_sigma(limit, Channel(127)).or_fail()?, Q8_8(0xFFFC));
    ensure_eq!(grain_sigma(limit, Channel(128)).or_fail()?, Q8_8(0xFFFC));
    Ok(())
}

#[test]
fn grain_density_above_the_limit_is_refused() {
    assert_eq!(check_grain_density_above_the_limit_is_refused(), Ok(()));
}

fn check_grain_density_above_the_limit_is_refused() -> Result<(), Fail> {
    // At 8225 the deviation at value 127 would be floor(8225 * 32639 / 4096) = 65540, which does
    // not fit a Q8.8 value, so the constructor refuses it. The largest raw value is refused too.
    for raw in [8225_u16, 0xFFFF] {
        ensure_eq!(
            GrainDensity::new(NonZeroU16::new(raw).or_fail()?),
            Err(GrainDensityError::AboveLimit),
            "raw density {raw}"
        );
    }
    Ok(())
}

#[test]
fn flat_black_and_white_are_unchanged_by_grain() {
    assert_eq!(check_flat_black_and_white_are_unchanged_by_grain(), Ok(()));
}

fn check_flat_black_and_white_are_unchanged_by_grain() -> Result<(), Fail> {
    let grain = awkward_grain(
        radius(Frac(1, 8))?,
        GrainSeed::new(Seed(0x0BAD_5EED_1234_5677)),
    )?;
    let sizes = [
        Size {
            columns: Width::new(16).or_fail()?,
            rows: Height::new(16).or_fail()?,
        },
        Size {
            columns: Width::new(17).or_fail()?,
            rows: Height::new(31).or_fail()?,
        },
    ];
    for size in sizes {
        for value in [Channel(0), Channel(255)] {
            let frame = flat_frame(size, value)?;
            let out = applied(&frame, &grain, FrameIndex::new(3))?;
            ensure_eq!(out, frame);
        }
    }
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
    let grain = awkward_grain(
        radius(Frac(1, 8))?,
        GrainSeed::new(Seed(0x0BAD_5EED_1234_5677)),
    )?;
    let first = applied(&frame, &grain, FrameIndex::new(4))?;
    let second = applied(&frame, &grain, FrameIndex::new(4))?;
    ensure_eq!(first, second);
    Ok(())
}

#[test]
fn a_different_seed_changes_a_mid_gray_frame() {
    assert_eq!(check_a_different_seed_changes_a_mid_gray_frame(), Ok(()));
}

fn check_a_different_seed_changes_a_mid_gray_frame() -> Result<(), Fail> {
    let frame = flat_frame(
        Size {
            columns: Width::new(16).or_fail()?,
            rows: Height::new(16).or_fail()?,
        },
        Channel(128),
    )?;
    let first = applied(
        &frame,
        &awkward_grain(
            radius(Frac(1, 8))?,
            GrainSeed::new(Seed(0x0BAD_5EED_1234_5677)),
        )?,
        FrameIndex::new(0),
    )?;
    let second = applied(
        &frame,
        &awkward_grain(
            radius(Frac(1, 8))?,
            GrainSeed::new(Seed(0xC0FF_EE00_DEAD_BEE3)),
        )?,
        FrameIndex::new(0),
    )?;
    ensure_ne!(first, second);
    Ok(())
}

#[test]
fn frame_index_zero_and_seven_draw_different_grain() {
    assert_eq!(
        check_frame_index_zero_and_seven_draw_different_grain(),
        Ok(())
    );
}

fn check_frame_index_zero_and_seven_draw_different_grain() -> Result<(), Fail> {
    let frame = flat_frame(
        Size {
            columns: Width::new(16).or_fail()?,
            rows: Height::new(16).or_fail()?,
        },
        Channel(128),
    )?;
    let grain = awkward_grain(
        radius(Frac(1, 8))?,
        GrainSeed::new(Seed(0x0BAD_5EED_1234_5677)),
    )?;
    let zeroth = applied(&frame, &grain, FrameIndex::new(0))?;
    let seventh = applied(&frame, &grain, FrameIndex::new(7))?;
    ensure_ne!(zeroth, seventh);
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
    let grain = awkward_grain(
        radius(Frac(1, 8))?,
        GrainSeed::new(Seed(0x0BAD_5EED_1234_5677)),
    )?;
    let five_first = applied(&frame, &grain, FrameIndex::new(5))?;
    let three_after = applied(&frame, &grain, FrameIndex::new(3))?;
    let three_first = applied(&frame, &grain, FrameIndex::new(3))?;
    let five_after = applied(&frame, &grain, FrameIndex::new(5))?;
    ensure_eq!(five_first, five_after);
    ensure_eq!(three_after, three_first);
    ensure_ne!(five_first, three_after);
    Ok(())
}

#[test]
fn window_one_grain_has_the_stated_mean_and_spread() {
    assert_eq!(
        check_window_one_grain_has_the_stated_mean_and_spread(),
        Ok(())
    );
}

fn check_window_one_grain_has_the_stated_mean_and_spread() -> Result<(), Fail> {
    // A radius of 1/1000 on a 64-row frame gives a window of one sensel, and density 1.0 gives
    // about 8 code values of deviation at mid-gray.
    let frame = flat_frame(
        Size {
            columns: Width::new(64).or_fail()?,
            rows: Height::new(64).or_fail()?,
        },
        Channel(128),
    )?;
    let grain = plain_grain(
        radius(Frac(1, 1000))?,
        GrainDensity::new(NonZeroU16::new(256).or_fail()?).or_fail()?,
        GrainSeed::new(Seed(0xA11C_E5EE_D5EE_D001)),
    );
    let out = applied(&frame, &grain, FrameIndex::new(2))?;
    let stats = green_statistics(&out)?;
    let mean = stats.mean.0;
    ensure!(
        (126..=130).contains(&mean),
        "mean green {mean} must lie in 126..=130"
    );
    let count = stats.count.0;
    let energy = u64::try_from(stats.energy.0).or_fail()?;
    let variance = energy.checked_div(count).or_fail()?;
    let spread = isqrt(IntSqrtInput(variance)).0;
    ensure!(
        (6..=10).contains(&spread),
        "standard deviation {spread} must lie in 6..=10"
    );
    Ok(())
}

#[test]
fn wide_grain_correlates_neighboring_sensels() {
    assert_eq!(check_wide_grain_correlates_neighboring_sensels(), Ok(()));
}

fn check_wide_grain_correlates_neighboring_sensels() -> Result<(), Fail> {
    // A radius of 1/16 on a 64-row frame gives a window of 8 sensels, since 8.5 floors to 8.
    let frame = flat_frame(
        Size {
            columns: Width::new(64).or_fail()?,
            rows: Height::new(64).or_fail()?,
        },
        Channel(128),
    )?;
    let seed = GrainSeed::new(Seed(0xA11C_E5EE_D5EE_D001));
    let unit = GrainDensity::new(NonZeroU16::new(256).or_fail()?).or_fail()?;
    let fine = applied(
        &frame,
        &plain_grain(radius(Frac(1, 1000))?, unit, seed),
        FrameIndex::new(2),
    )?;
    let wide = applied(
        &frame,
        &plain_grain(radius(Frac(1, 16))?, unit, seed),
        FrameIndex::new(2),
    )?;
    let fine_stats = green_statistics(&fine)?;
    let wide_stats = green_statistics(&wide)?;
    // A box of 8 sensels has lag-one correlation 7/8, so twice the covariance exceeds the energy.
    let twice = wide_stats.lag_one.0.checked_mul(2).or_fail()?;
    ensure!(
        twice > wide_stats.energy.0,
        "lag-one covariance {} must exceed half the energy {}",
        wide_stats.lag_one.0,
        wide_stats.energy.0
    );
    ensure!(
        wide_stats.lag_one.0 > fine_stats.lag_one.0,
        "wide grain covariance {} must exceed fine grain covariance {}",
        wide_stats.lag_one.0,
        fine_stats.lag_one.0
    );
    // A window of one sensel has no correlation, so ten times its covariance stays below the energy.
    let scaled = fine_stats.lag_one.0.checked_mul(10).or_fail()?;
    let magnitude = scaled.checked_abs().or_fail()?;
    ensure!(
        magnitude < fine_stats.energy.0,
        "fine grain correlation must stay below 0.1"
    );
    Ok(())
}

#[test]
fn a_grain_too_coarse_for_the_frame_fails_to_apply() {
    assert_eq!(
        check_a_grain_too_coarse_for_the_frame_fails_to_apply(),
        Ok(())
    );
}

fn check_a_grain_too_coarse_for_the_frame_fails_to_apply() -> Result<(), Fail> {
    let mut frame = flat_frame(
        Size {
            columns: Width::new(16).or_fail()?,
            rows: Height::new(1080).or_fail()?,
        },
        Channel(128),
    )?;
    let grain = plain_grain(
        radius(Frac(1, 2))?,
        GrainDensity::new(NonZeroU16::new(256).or_fail()?).or_fail()?,
        GrainSeed::new(Seed(0xA11C_E5EE_D5EE_D001)),
    );
    let result = apply_film_grain(&mut frame, &grain, FrameIndex::new(0));
    ensure_eq!(result, Err(GrainError::GrainTooCoarse));
    Ok(())
}
