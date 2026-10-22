use super::*;

use core::num::NonZeroU32;
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

#[test]
fn playback_only_has_no_capture() {
    assert_eq!(check_playback_only_has_no_capture(), Ok(()));
}

fn check_playback_only_has_no_capture() -> Result<(), Fail> {
    let playback = FrameRate::from_fps(30).or_fail()?;
    let rates = FrameRates::playback_only(playback);
    ensure_eq!(rates.playback(), playback);
    ensure!(rates.capture().is_none(), "playback-only has no capture");
    Ok(())
}

#[test]
fn distinct_rates_are_both_recoverable() {
    assert_eq!(check_distinct_rates_are_both_recoverable(), Ok(()));
}

fn check_distinct_rates_are_both_recoverable() -> Result<(), Fail> {
    let playback = FrameRate::from_fraction(30000, NonZeroU32::new(1001).or_fail()?).or_fail()?;
    let capture = FrameRate::from_fps(24).or_fail()?;
    let rates = FrameRates::distinct_in(RatePair { playback, capture }, CaptureCarrier::ProjectBox)
        .or_fail()?;
    ensure_eq!(rates.playback(), playback);
    ensure_eq!(rates.capture(), Some(capture));
    Ok(())
}

#[test]
fn equal_rates_are_rejected() {
    assert_eq!(check_equal_rates_are_rejected(), Ok(()));
}

fn check_equal_rates_are_rejected() -> Result<(), Fail> {
    let twenty_four = FrameRate::from_fps(24).or_fail()?;
    ensure_eq!(
        FrameRates::distinct_in(
            RatePair {
                playback: twenty_four,
                capture: twenty_four,
            },
            CaptureCarrier::ProjectBox,
        ),
        Err(FrameRatesError::CaptureEqualsPlayback)
    );
    Ok(())
}

#[test]
fn equal_rates_in_different_spellings_are_rejected() {
    assert_eq!(
        check_equal_rates_in_different_spellings_are_rejected(),
        Ok(())
    );
}

fn check_equal_rates_in_different_spellings_are_rejected() -> Result<(), Fail> {
    // 48/2 normalises to 24/1, so the two spellings are the same rate.
    let spelled_plain = FrameRate::from_fps(24).or_fail()?;
    let spelled_halved = FrameRate::from_fraction(48, NonZeroU32::new(2).or_fail()?).or_fail()?;
    ensure_eq!(
        FrameRates::distinct_in(
            RatePair {
                playback: spelled_plain,
                capture: spelled_halved,
            },
            CaptureCarrier::ProjectBox,
        ),
        Err(FrameRatesError::CaptureEqualsPlayback)
    );
    Ok(())
}

#[test]
fn rebuild_refuses_equal_pair() {
    assert_eq!(check_rebuild_refuses_equal_pair(), Ok(()));
}

fn check_rebuild_refuses_equal_pair() -> Result<(), Fail> {
    // 25 fps playback with a capture part that names the same 25 fps rate.
    let playback = FrameRate::from_fps(25).or_fail()?;
    let equal = DistinctCapture {
        rate: playback,
        carrier: CaptureCarrier::QuickTimeMeta,
    };
    ensure_eq!(
        FrameRates::from_playback_and_capture(playback, Some(equal)),
        Err(FrameRatesError::CaptureEqualsPlayback)
    );
    Ok(())
}

#[test]
fn rebuild_accepts_distinct_pair() {
    assert_eq!(check_rebuild_accepts_distinct_pair(), Ok(()));
}

fn check_rebuild_accepts_distinct_pair() -> Result<(), Fail> {
    // 25 fps playback with a 23 fps capture part: the two rates stay recoverable.
    let playback = FrameRate::from_fps(25).or_fail()?;
    let capture = FrameRate::from_fps(23).or_fail()?;
    let distinct = DistinctCapture {
        rate: capture,
        carrier: CaptureCarrier::ProjectBox,
    };
    let rates = FrameRates::from_playback_and_capture(playback, Some(distinct)).or_fail()?;
    ensure_eq!(rates.playback(), playback);
    ensure_eq!(rates.capture(), Some(capture));
    ensure_eq!(rates.distinct_capture(), Some(distinct));
    Ok(())
}
