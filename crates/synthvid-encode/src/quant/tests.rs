use super::*;
use synthvid_testkit::{ensure, ensure_eq, ensure_ne, Fail, OrFail as _};

#[test]
fn scale_by_quality_correctness() {
    assert_eq!(check_scale_by_quality_correctness(), Ok(()));
}

fn check_scale_by_quality_correctness() -> Result<(), Fail> {
    // Verify the lookup table is correct by re-deriving a few entries
    for (quality_index, &table_entry) in SCALE_BY_QUALITY.iter().enumerate().skip(1) {
        let quality = u32::from(u8::try_from(quality_index).or_fail()?);
        let expected_scale = if quality < 50 {
            (5000_u32).div_euclid(quality).clamp(2, 256)
        } else {
            // `quality <= 100`, so `2 * quality <= 200` and neither step can fail.
            let doubled = quality.checked_mul(2).or_fail()?;
            (200_u32).checked_sub(doubled).or_fail()?.clamp(2, 256)
        };
        let table_scale = u32::from(table_entry);
        ensure_eq!(
            table_scale,
            expected_scale,
            "Scale factor mismatch at quality {quality_index}"
        );
    }
    Ok(())
}

#[test]
fn scale_quantization_luma_vs_chroma() {
    assert_eq!(check_scale_quantization_luma_vs_chroma(), Ok(()));
}

fn check_scale_quantization_luma_vs_chroma() -> Result<(), Fail> {
    let quality = Quality::new(75).or_fail()?;
    let luma = scale_table(quality, Component::Luma);
    let chroma = scale_table(quality, Component::Chroma);
    // Tables should differ (they have different base values)
    ensure_ne!(luma.divisors()[0].get(), chroma.divisors()[0].get());
    Ok(())
}

#[test]
fn higher_quality_smaller_quant() {
    assert_eq!(check_higher_quality_smaller_quant(), Ok(()));
}

fn check_higher_quality_smaller_quant() -> Result<(), Fail> {
    let low_quality = Quality::new(50).or_fail()?;
    let high_quality = Quality::new(90).or_fail()?;
    let low_table = scale_table(low_quality, Component::Luma);
    let high_table = scale_table(high_quality, Component::Luma);
    // Higher quality should have smaller quantization values
    ensure!(low_table.divisors()[0].get() > high_table.divisors()[0].get());
    Ok(())
}

#[test]
fn quantization_table_clamping_low_quality() {
    assert_eq!(check_quantization_table_clamping_low_quality(), Ok(()));
}

fn check_quantization_table_clamping_low_quality() -> Result<(), Fail> {
    // Regression test for quantization table truncation bug.
    // Before the fix, at quality settings 1-23, several table entries would
    // produce u32 values > 255 before extracting the low byte, causing silent
    // truncation/wrapping modulo 256 instead of proper JPEG-spec clamping to
    // [1, 255]. This could even produce a quantization divisor of 0 (causing
    // division by zero in dct::quantize) or other wrong values.
    // Example: quality=1, luma base value 100 gave val=256, which truncated to 0.
    // Example: quality=1, luma base value 121 (index 53) gave val=310, which
    // truncated to 54 instead of the correct clamped value 255.

    // Check that all divisors in low-quality tables are in valid range [1, 255]
    for level in 1..=23 {
        let quality = Quality::new(level).or_fail()?;
        let luma = scale_table(quality, Component::Luma);
        let chroma = scale_table(quality, Component::Chroma);

        // All divisors must be at least 1 (no quantization divisor of 0)
        // -- guaranteed by `QuantDivisor`'s `NonZeroU8` storage, not just
        // checked here, but this still pins the invariant against a
        // future regression in that type.
        ensure!(
            luma.divisors()
                .iter()
                .all(|&divisor| divisor.get().get() >= 1),
            "Luma table at quality {level} has divisor < 1"
        );
        ensure!(
            chroma
                .divisors()
                .iter()
                .all(|&divisor| divisor.get().get() >= 1),
            "Chroma table at quality {level} has divisor < 1"
        );
    }

    // Specific regression check: at quality=1, the luma base value 121 (at
    // index 53 in STANDARD_LUMA_QUANTIZATION) previously gave val=(121*256+50)/100=310,
    // which truncated to byte 54. After the fix, it should clamp to 255.
    let quality_1 = Quality::new(1).or_fail()?;
    let luma_q1 = scale_table(quality_1, Component::Luma);
    let divisors = luma_q1.divisors();
    ensure_eq!(
        divisors[53].get().get(),
        255,
        "Luma table at quality 1, index 53 (base value 121) should clamp to 255, not wrap to 54"
    );
    Ok(())
}

#[test]
fn quant_divisor_invariants() {
    // Test that QuantDivisor::new always clamps to the valid range [1, 255].
    // QuantDivisor(0) should be impossible to construct.

    // Zero should clamp to 1
    let zero = QuantDivisor::new(0);
    assert_eq!(
        zero.get().get(),
        1,
        "QuantDivisor::new(0) should clamp to 1, never produce 0"
    );

    // Values below 1 should all clamp to 1
    for val in [0, 1, 2, 3] {
        let div = QuantDivisor::new(val);
        assert!(
            div.get().get() >= 1,
            "QuantDivisor::new({val}) produced {}, must be >= 1",
            div.get()
        );
    }

    // Values above 255 should clamp to 255
    let large = QuantDivisor::new(1000);
    assert_eq!(
        large.get().get(),
        255,
        "QuantDivisor::new(1000) should clamp to 255, not wrap or truncate"
    );

    // Boundary values should be preserved
    assert_eq!(QuantDivisor::new(1).get().get(), 1);
    assert_eq!(QuantDivisor::new(128).get().get(), 128);
    assert_eq!(QuantDivisor::new(255).get().get(), 255);
}
