use super::*;

#[test]
fn shot_sigma_is_ten_code_values_at_unit_slope() {
    assert_eq!(check_shot_sigma_is_ten_code_values_at_unit_slope(), Ok(()));
}

fn check_shot_sigma_is_ten_code_values_at_unit_slope() -> Result<(), Fail> {
    // Variance 256 * 100 * 256 = 6553600 = 2560 * 2560, so sigma is 2560 raw, which is 10 code
    // values, the root of the variance 1.0 * 100 in code values squared.
    ensure_eq!(
        shot_sigma(ShotGain(Q8_8(256)), Light(100)).or_fail()?,
        Amplitude(2560)
    );
    Ok(())
}

#[test]
fn shot_sigma_of_an_awkward_slope_floors_the_root() {
    assert_eq!(
        check_shot_sigma_of_an_awkward_slope_floors_the_root(),
        Ok(())
    );
}

fn check_shot_sigma_of_an_awkward_slope_floors_the_root() -> Result<(), Fail> {
    // Variance 1000 * 77 * 256 = 19712000. 4439 * 4439 = 19704721 is at most that, and
    // 4440 * 4440 = 19713600 is above it, so the root floors to 4439.
    ensure_eq!(
        shot_sigma(ShotGain(Q8_8(1000)), Light(77)).or_fail()?,
        Amplitude(4439)
    );
    Ok(())
}

#[test]
fn shot_sigma_of_black_is_zero_for_any_slope() {
    assert_eq!(check_shot_sigma_of_black_is_zero_for_any_slope(), Ok(()));
}

fn check_shot_sigma_of_black_is_zero_for_any_slope() -> Result<(), Fail> {
    for slope in [0_u16, 1, 256, 0xFFFF] {
        ensure_eq!(
            shot_sigma(ShotGain(Q8_8(slope)), Light(0)).or_fail()?,
            Amplitude(0),
            "slope {slope}"
        );
    }
    ensure_eq!(
        shot_sigma(ShotGain(Q8_8(0)), Light(77)).or_fail()?,
        Amplitude(0)
    );
    Ok(())
}

#[test]
fn shot_sigma_at_the_largest_slope_and_full_value_fits_q8_8() {
    assert_eq!(
        check_shot_sigma_at_the_largest_slope_and_full_value_fits_q8_8(),
        Ok(())
    );
}

fn check_shot_sigma_at_the_largest_slope_and_full_value_fits_q8_8() -> Result<(), Fail> {
    // Variance 65535 * 255 * 256 = 4278124800. 65407 * 65407 = 4278075649 is at most that, and
    // 65408 * 65408 = 4278206464 is above it, so the root floors to 65407, which is 255.5 code
    // values and fits a Q8.8 value.
    ensure_eq!(
        shot_sigma(ShotGain(Q8_8(0xFFFF)), Light(255)).or_fail()?,
        Amplitude(65407)
    );
    Ok(())
}

#[test]
fn shot_noise_pulls_some_white_sensels_below_full_scale() {
    assert_eq!(
        check_shot_noise_pulls_some_white_sensels_below_full_scale(),
        Ok(())
    );
}

fn check_shot_noise_pulls_some_white_sensels_below_full_scale() -> Result<(), Fail> {
    // At slope 256 and value 255 the sigma is isqrt(256 * 255 * 256) = 4088 raw, about 16 code
    // values, so roughly half of the sensels draw a negative shot term and fall below 255.
    let size = Size {
        columns: Width::new(16).or_fail()?,
        rows: Height::new(16).or_fail()?,
    };
    let frame = flat_frame(size, Channel(255))?;
    let out = applied(
        &frame,
        &shot_only(ShotGain(Q8_8(256)), awkward_seeds())?,
        FrameIndex::new(6),
    )?;
    let mut below_full = 0_u32;
    for row in 0..size.rows.get().get() {
        for column in 0..size.columns.get().get() {
            let at = PixelCoord::new(PixelColumn(column), PixelRow(row));
            if pixel(&out, at)?.green.0 < 255 {
                below_full = below_full.checked_add(1).or_fail()?;
            }
        }
    }
    ensure!(
        below_full > 0,
        "large shot noise must pull some white sensels below full scale"
    );
    Ok(())
}

#[test]
fn shot_noise_keeps_a_black_frame_at_zero() {
    assert_eq!(check_shot_noise_keeps_a_black_frame_at_zero(), Ok(()));
}

fn check_shot_noise_keeps_a_black_frame_at_zero() -> Result<(), Fail> {
    let frame = flat_frame(
        Size {
            columns: Width::new(16).or_fail()?,
            rows: Height::new(16).or_fail()?,
        },
        Channel(0),
    )?;
    ensure_eq!(
        applied(
            &frame,
            &shot_only(ShotGain(Q8_8(256)), awkward_seeds())?,
            FrameIndex::new(6)
        )?,
        frame
    );
    Ok(())
}

#[test]
fn largest_shot_term_on_one_white_sensel_is_applied() {
    assert_eq!(
        check_largest_shot_term_on_one_white_sensel_is_applied(),
        Ok(())
    );
}

fn check_largest_shot_term_on_one_white_sensel_is_applied() -> Result<(), Fail> {
    // The largest slope on a single full-value sensel has sigma 65407 raw, which fits Q8.8, so the
    // frame is recorded rather than refused.
    let mut frame = flat_frame(
        Size {
            columns: Width::new(1).or_fail()?,
            rows: Height::new(1).or_fail()?,
        },
        Channel(255),
    )?;
    let noise = shot_only(ShotGain(Q8_8(0xFFFF)), awkward_seeds())?;
    ensure_eq!(
        apply_sensor_noise(&mut frame, &noise, FrameIndex::new(0)),
        Ok(())
    );
    Ok(())
}

#[test]
fn shot_noise_follows_light_above_full_scale() {
    assert_eq!(check_shot_noise_follows_light_above_full_scale(), Ok(()));
}

fn check_shot_noise_follows_light_above_full_scale() -> Result<(), Fail> {
    // Value 230 at gain 81920 / 65536 = 1.25 is 230 * 1.25 = 287.5 code values, so the level is
    // 230 * 256 * 81920 / 65536 = 73600 raw. Rounding (73600 + 128) / 256 floors to 288 code
    // values, which is above full scale and is not clamped before the noise is drawn.
    let level = level_of(Channel(230), Gain(81_920)).or_fail()?;
    ensure_eq!(level, Level(73_600));
    let light = light_of(level).or_fail()?;
    ensure_eq!(light, Light(288));
    // Slope 512 (2.0) gives variance 512 * 288 * 256 = 37748736 = 6144 * 6144, so the sigma is
    // 6144 raw, which is 24 code values.
    ensure_eq!(
        shot_sigma(ShotGain(Q8_8(512)), light).or_fail()?,
        Amplitude(6_144)
    );
    // Clamped to 255 before the noise, the variance would be 512 * 255 * 256 = 33423360. Its
    // root floors to 5781 raw, which differs from the unclamped sigma of 6144.
    ensure_eq!(
        shot_sigma(ShotGain(Q8_8(512)), Light(255)).or_fail()?,
        Amplitude(5_781)
    );
    Ok(())
}

#[test]
fn negative_shot_draw_on_light_above_full_scale_records_code_240() {
    assert_eq!(
        check_negative_shot_draw_on_light_above_full_scale_records_code_240(),
        Ok(())
    );
}

fn check_negative_shot_draw_on_light_above_full_scale_records_code_240() -> Result<(), Fail> {
    // Value 230 at gain 1.25 has level 73600 raw and light 288, as in the test above. The sigma at
    // slope 512 is 6144 raw. A shot draw of -512 raw, which is -2.0 in Q8.8, gives the shot term
    // floor(6144 * -512 / 256) = -12288 raw. The column offset and the read term are zero, so the
    // total is 73600 - 12288 = 61312 raw, and the code is floor((61312 + 128) / 256) = 240.
    let level = level_of(Channel(230), Gain(81_920)).or_fail()?;
    let light = light_of(level).or_fail()?;
    let sigma = shot_sigma(ShotGain(Q8_8(512)), light).or_fail()?;
    let shot = scaled_draw(sigma, NormalDraw(-512)).or_fail()?;
    ensure_eq!(shot, SignedQ8(-12_288));
    ensure_eq!(
        combine_level(level, ColumnOffset(0), ShotNoise(shot.0), ReadNoise(0)).or_fail()?,
        Channel(240)
    );
    // With the light clamped to 255 first, the sigma is 5781 raw, the shot term is
    // floor(5781 * -512 / 256) = -11562 raw, and the total 62038 raw gives the code
    // floor((62038 + 128) / 256) = 242, so the clamp would change the recorded value.
    let clamped = shot_sigma(ShotGain(Q8_8(512)), Light(255)).or_fail()?;
    let clamped_shot = scaled_draw(clamped, NormalDraw(-512)).or_fail()?;
    ensure_eq!(clamped_shot, SignedQ8(-11_562));
    ensure_eq!(
        combine_level(
            level,
            ColumnOffset(0),
            ShotNoise(clamped_shot.0),
            ReadNoise(0)
        )
        .or_fail()?,
        Channel(242)
    );
    Ok(())
}

#[test]
fn shot_sigma_of_the_largest_gained_light_is_not_overflowed() {
    assert_eq!(
        check_shot_sigma_of_the_largest_gained_light_is_not_overflowed(),
        Ok(())
    );
}

fn check_shot_sigma_of_the_largest_gained_light_is_not_overflowed() -> Result<(), Fail> {
    // The largest gain is 65536 + floor(65535 * 1530 / 256) = 457210, so the largest light is
    // floor((floor(255 * 256 * 457210 / 65536) + 128) / 256) = 1779 code values. The variance is
    // 65535 * 1779 * 256 = 29846211840, and 172760 * 172760 = 29846017600 is at most that while
    // 172761 * 172761 = 29846363121 is above it, so the sigma is 172760 raw.
    ensure_eq!(
        shot_sigma(ShotGain(Q8_8(0xFFFF)), Light(1_779)).or_fail()?,
        Amplitude(172_760)
    );
    Ok(())
}
