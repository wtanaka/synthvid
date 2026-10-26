use super::*;
use core::num::NonZeroU32;
use synthvid_scene::{FrameCount, FrameRate, Height, Width};
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

/// Sixty-four sensels wide and forty-eight tall.
fn small_dimensions() -> Result<Dimensions, Fail> {
    Ok(Dimensions::new(
        Width::new(64).or_fail()?,
        Height::new(48).or_fail()?,
    ))
}

/// Seventeen sensels wide and thirty-one tall.
fn awkward_dimensions() -> Result<Dimensions, Fail> {
    Ok(Dimensions::new(
        Width::new(17).or_fail()?,
        Height::new(31).or_fail()?,
    ))
}

/// Eight pictures at 30 per second.
fn small_timing() -> Result<Timing, Fail> {
    Ok(Timing::new(
        FrameRate::from_fraction(30, NonZeroU32::new(1).or_fail()?).or_fail()?,
        FrameCount::new(8).or_fail()?,
    ))
}

/// A recorder that records nothing, outputs 64 by 48 sensels, and takes eight pictures at 30 per
/// second.
fn small_recorder() -> Result<Recorder, Fail> {
    Ok(Recorder::new(
        Medium::Off,
        small_dimensions()?,
        small_timing()?,
    ))
}

#[test]
fn dimensions_are_the_given_dimensions() {
    assert_eq!(check_dimensions_are_the_given_dimensions(), Ok(()));
}

fn check_dimensions_are_the_given_dimensions() -> Result<(), Fail> {
    let recorder = small_recorder()?;
    ensure_eq!(recorder.dimensions(), small_dimensions()?);
    Ok(())
}

#[test]
fn timing_count_is_the_frame_count() {
    assert_eq!(check_timing_count_is_the_frame_count(), Ok(()));
}

fn check_timing_count_is_the_frame_count() -> Result<(), Fail> {
    let recorder = small_recorder()?;
    ensure_eq!(recorder.timing().count(), FrameCount::new(8).or_fail()?);
    Ok(())
}

#[test]
fn awkward_values_round_trip() {
    assert_eq!(check_awkward_values_round_trip(), Ok(()));
}

fn check_awkward_values_round_trip() -> Result<(), Fail> {
    let timing = Timing::new(
        FrameRate::from_fraction(30_000, NonZeroU32::new(1_001).or_fail()?).or_fail()?,
        FrameCount::new(20_000).or_fail()?,
    );
    let recorder = Recorder::new(Medium::Off, awkward_dimensions()?, timing);
    ensure_eq!(recorder.medium(), Medium::Off);
    ensure_eq!(recorder.dimensions(), awkward_dimensions()?);
    ensure_eq!(recorder.timing(), timing);
    ensure_eq!(
        recorder.timing().count(),
        FrameCount::new(20_000).or_fail()?
    );
    Ok(())
}
