use super::*;
use crate::medium::{
    ColumnSigma, PatternSeed, PrnuSpread, ReadSigma, SensorNoise, SensorSeeds, ShotGain,
    TemporalSeed, Q8_8,
};
use crate::optics::VignetteStrength;
use crate::testutil::Frac;
use core::num::{NonZeroU16, NonZeroU32};
use synthvid_scene::{Height, Magnification, Motion, Point, Rotation, Seed, Turns, Width, Zoom};
use synthvid_testkit::{ensure_eq, ensure_ne, Fail, OrFail as _};

/// A magnification built from an exact fraction.
fn magnification(fraction: Frac) -> Result<Magnification, Fail> {
    Magnification::new(fraction.ratio()?).or_fail()
}

/// A view with a fixed off-origin position, a fixed turn, and a fixed zoom, built from awkward
/// fractions.
fn base_view() -> Result<View, Fail> {
    Ok(View::new(
        Motion::Fixed(Point {
            across: Frac(5, 3).ratio()?,
            down: Frac(-7, 4).ratio()?,
        }),
        Rotation::Fixed(Turns::new(Frac(1, 7).ratio()?)),
        Zoom::Fixed(magnification(Frac(3, 2))?),
    ))
}

/// A flat camera's vignette with a raw strength of 64.
fn base_optics() -> Result<Optics, Fail> {
    let strength = VignetteStrength::new(NonZeroU16::new(64).or_fail()?).or_fail()?;
    Ok(Optics::Vignette(strength))
}

/// Sensor noise with every term present.
fn base_medium() -> Result<Medium, Fail> {
    Ok(Medium::Sensor(
        SensorNoise::new(
            ShotGain(Q8_8(37)),
            ReadSigma(Q8_8(91)),
            PrnuSpread(1_237),
            ColumnSigma(Q8_8(14)),
            SensorSeeds::new(
                TemporalSeed::new(Seed::new(11)),
                PatternSeed::new(Seed::new(13)),
            ),
        )
        .or_fail()?,
    ))
}

/// Full high-definition dimensions.
fn base_dimensions() -> Result<Dimensions, Fail> {
    Ok(Dimensions::new(
        Width::new(1920).or_fail()?,
        Height::new(1080).or_fail()?,
    ))
}

/// The NTSC film rate of 30000/1001 pictures per second.
fn ntsc_rate() -> Result<FrameRate, Fail> {
    FrameRate::from_fraction(30_000, NonZeroU32::new(1_001).or_fail()?).or_fail()
}

/// The timing of 20000 pictures at 30000/1001 pictures per second.
fn base_timing() -> Result<Timing, Fail> {
    Ok(Timing::new(
        ntsc_rate()?,
        FrameCount::new(20_000).or_fail()?,
    ))
}

/// The recorder every base camera records with: sensor noise, full high-definition dimensions,
/// and the NTSC timing.
fn base_recorder() -> Result<Recorder, Fail> {
    Ok(Recorder::new(
        base_medium()?,
        base_dimensions()?,
        base_timing()?,
    ))
}

/// The camera every single-part variant is derived from.
fn base_camera() -> Result<Camera, Fail> {
    Ok(Camera::new(base_view()?, base_optics()?, &base_recorder()?))
}

/// Six cameras that each differ from the base camera in exactly one part.
///
/// The order is view, optics, medium, dimensions, frame rate, and frame count.
fn single_part_variants() -> Result<Vec<Camera>, Fail> {
    let view = View {
        zoom: Zoom::Fixed(magnification(Frac(2, 1))?),
        ..base_view()?
    };
    let rate = FrameRate::from_fraction(24_000, NonZeroU32::new(1_001).or_fail()?).or_fail()?;
    let count = FrameCount::new(7).or_fail()?;
    let small = Dimensions::new(Width::new(17).or_fail()?, Height::new(31).or_fail()?);
    Ok(vec![
        Camera::new(view, base_optics()?, &base_recorder()?),
        Camera::new(base_view()?, Optics::Off, &base_recorder()?),
        Camera::new(
            base_view()?,
            base_optics()?,
            &Recorder::new(Medium::Off, base_dimensions()?, base_timing()?),
        ),
        Camera::new(
            base_view()?,
            base_optics()?,
            &Recorder::new(base_medium()?, small, base_timing()?),
        ),
        Camera::new(
            base_view()?,
            base_optics()?,
            &Recorder::new(
                base_medium()?,
                base_dimensions()?,
                Timing::new(rate, FrameCount::new(20_000).or_fail()?),
            ),
        ),
        Camera::new(
            base_view()?,
            base_optics()?,
            &Recorder::new(
                base_medium()?,
                base_dimensions()?,
                Timing::new(ntsc_rate()?, count),
            ),
        ),
    ])
}

#[test]
fn timing_round_trips_rate_and_count() {
    assert_eq!(check_timing_round_trips_rate_and_count(), Ok(()));
}

fn check_timing_round_trips_rate_and_count() -> Result<(), Fail> {
    let timing = base_timing()?;
    ensure_eq!(timing.rate(), ntsc_rate()?);
    ensure_eq!(timing.count(), FrameCount::new(20_000).or_fail()?);
    Ok(())
}

#[test]
fn camera_round_trips_each_part() {
    assert_eq!(check_camera_round_trips_each_part(), Ok(()));
}

fn check_camera_round_trips_each_part() -> Result<(), Fail> {
    let camera = base_camera()?;
    ensure_eq!(camera.view(), &base_view()?);
    ensure_eq!(camera.optics(), base_optics()?);
    ensure_eq!(camera.medium(), base_medium()?);
    ensure_eq!(camera.dimensions(), base_dimensions()?);
    ensure_eq!(camera.timing(), base_timing()?);
    Ok(())
}

#[test]
fn camera_differing_in_one_part_is_unequal() {
    assert_eq!(check_camera_differing_in_one_part_is_unequal(), Ok(()));
}

fn check_camera_differing_in_one_part_is_unequal() -> Result<(), Fail> {
    let base = base_camera()?;
    let variants = single_part_variants()?;
    ensure_eq!(variants.len(), 6);
    for variant in variants {
        ensure_ne!(base, variant);
    }
    Ok(())
}

#[test]
fn camera_returns_the_recorder_it_was_built_with() {
    assert_eq!(
        check_camera_returns_the_recorder_it_was_built_with(),
        Ok(())
    );
}

fn check_camera_returns_the_recorder_it_was_built_with() -> Result<(), Fail> {
    let recorder = base_recorder()?;
    let camera = Camera::new(base_view()?, base_optics()?, &recorder);
    ensure_eq!(camera.recorder(), recorder);
    ensure_eq!(camera.medium(), recorder.medium());
    ensure_eq!(camera.dimensions(), recorder.dimensions());
    ensure_eq!(camera.timing(), recorder.timing());
    ensure_eq!(camera.medium(), base_medium()?);
    ensure_eq!(camera.dimensions(), base_dimensions()?);
    ensure_eq!(camera.timing(), base_timing()?);
    Ok(())
}

#[test]
fn camera_timing_count_is_the_recorder_frame_count() {
    assert_eq!(
        check_camera_timing_count_is_the_recorder_frame_count(),
        Ok(())
    );
}

fn check_camera_timing_count_is_the_recorder_frame_count() -> Result<(), Fail> {
    let recorder = Recorder::new(
        Medium::Off,
        Dimensions::new(Width::new(64).or_fail()?, Height::new(48).or_fail()?),
        Timing::new(
            FrameRate::from_fraction(30, NonZeroU32::new(1).or_fail()?).or_fail()?,
            FrameCount::new(8).or_fail()?,
        ),
    );
    let camera = Camera::new(base_view()?, Optics::Off, &recorder);
    ensure_eq!(camera.timing().count(), FrameCount::new(8).or_fail()?);
    ensure_eq!(
        camera.dimensions(),
        Dimensions::new(Width::new(64).or_fail()?, Height::new(48).or_fail()?)
    );
    ensure_eq!(camera.medium(), Medium::Off);
    Ok(())
}

#[test]
fn cameras_differing_only_in_medium_are_unequal() {
    assert_eq!(check_cameras_differing_only_in_medium_are_unequal(), Ok(()));
}

fn check_cameras_differing_only_in_medium_are_unequal() -> Result<(), Fail> {
    let sensor = Camera::new(base_view()?, base_optics()?, &base_recorder()?);
    let unrecorded = Camera::new(
        base_view()?,
        base_optics()?,
        &Recorder::new(Medium::Off, base_dimensions()?, base_timing()?),
    );
    ensure_ne!(sensor, unrecorded);
    ensure_eq!(sensor.dimensions(), unrecorded.dimensions());
    ensure_eq!(sensor.timing(), unrecorded.timing());
    Ok(())
}
