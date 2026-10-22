use super::*;
use crate::testutil::Frac;
use synthvid_scene::{
    Background, Camera, CaptureCarrier, Channel, Dimensions, FrameCount, Height, Magnification,
    Motion, Numerator, Point, Ratio, Rgb8, Rotation, Turns, Width, Zoom,
};
use synthvid_testkit::{ensure, ensure_eq, refusal, Fail, OrFail as _};

fn rate(fraction: Frac) -> Result<FrameRate, Fail> {
    FrameRate::new(fraction.ratio()?).or_fail()
}

/// A distinct capture rate carried in the project-specific box.
fn project_box(rate: FrameRate) -> CaptureRate {
    CaptureRate::Distinct(DistinctCapture {
        rate,
        carrier: CaptureCarrier::ProjectBox,
    })
}

/// A two-frame, 16 by 16 grey scene at the given playback rate.
fn scene_at(frame_rate: FrameRate) -> Result<Scene, Fail> {
    let origin = Point {
        across: Ratio::from_integer(Numerator(0)),
        down: Ratio::from_integer(Numerator(0)),
    };
    let camera = Camera::new(
        Motion::Fixed(origin),
        Rotation::Fixed(Turns::new(Ratio::from_integer(Numerator(0)))),
        Zoom::Fixed(Magnification::new(Ratio::from_integer(Numerator(1))).or_fail()?),
    );
    Ok(Scene {
        dimensions: Dimensions::new(Width::new(16).or_fail()?, Height::new(16).or_fail()?),
        frame_rate,
        frame_count: FrameCount::new(2).or_fail()?,
        scale: None,
        background: Background::Solid(Rgb8 {
            red: Channel(128),
            green: Channel(128),
            blue: Channel(128),
        }),
        camera,
        objects: Vec::new(),
    })
}

fn iso(capture: CaptureRate) -> Container {
    Container::Iso {
        rotation: DisplayRotation::Identity,
        capture,
    }
}

/// Builds a raw specification, keeping the refusal as a value for the test to inspect.
fn spec_with(
    playback: FrameRate,
    container: Container,
) -> Result<Result<VideoSpec, VideoSpecError>, Fail> {
    Ok(VideoSpec::new(
        JsonEntryName::new("capture-spec").or_fail()?,
        scene_at(playback)?,
        Encoding::Raw,
        container,
        None,
    ))
}

fn check_distinct_capture_accepted() -> Result<(), Fail> {
    let capture = project_box(rate(Frac(24, 1))?);
    let spec = spec_with(rate(Frac(30000, 1001))?, iso(capture))?.or_fail()?;
    ensure_eq!(spec.container(), iso(capture));
    Ok(())
}

/// An ISO container may declare 24/1 capture beside 30000/1001 playback.
#[test]
fn distinct_capture_is_accepted() {
    assert_eq!(check_distinct_capture_accepted(), Ok(()));
}

fn check_equal_capture_refused() -> Result<(), Fail> {
    let refused = spec_with(rate(Frac(30, 1))?, iso(project_box(rate(Frac(30, 1))?)))?;
    ensure_eq!(
        refusal(refused)?,
        VideoSpecError::FrameRates(FrameRatesError::CaptureEqualsPlayback)
    );
    Ok(())
}

/// A capture rate equal to playback is refused.
#[test]
fn equal_capture_is_refused() {
    assert_eq!(check_equal_capture_refused(), Ok(()));
}

fn check_equal_capture_refused_in_other_spelling() -> Result<(), Fail> {
    // 60/2 reduces to 30/1, so the same rate is refused however it is spelled.
    let refused = spec_with(rate(Frac(30, 1))?, iso(project_box(rate(Frac(60, 2))?)))?;
    ensure!(
        refused.is_err(),
        "a capture rate of 60/2 equals a playback rate of 30/1"
    );
    ensure_eq!(
        refusal(refused)?,
        VideoSpecError::FrameRates(FrameRatesError::CaptureEqualsPlayback)
    );
    Ok(())
}

/// Equality is exact, so a differently spelled equal rate is refused too.
#[test]
fn equal_capture_is_refused_in_other_spelling() {
    assert_eq!(check_equal_capture_refused_in_other_spelling(), Ok(()));
}

fn check_integer_capture_over_fraction_playback_accepted() -> Result<(), Fail> {
    // 1000/1 is an integer rate about thirty-three times 30000/1001.
    let capture = project_box(rate(Frac(1000, 1))?);
    let spec = spec_with(rate(Frac(30000, 1001))?, iso(capture))?.or_fail()?;
    ensure_eq!(spec.container(), iso(capture));
    Ok(())
}

/// An integer capture rate far above a fractional playback rate is accepted.
#[test]
fn integer_capture_over_fraction_playback_is_accepted() {
    assert_eq!(
        check_integer_capture_over_fraction_playback_accepted(),
        Ok(())
    );
}

fn check_slower_capture_accepted() -> Result<(), Fail> {
    // A capture rate below playback is a distinct rate like any other.
    let capture = project_box(rate(Frac(12, 1))?);
    let spec = spec_with(rate(Frac(30, 1))?, iso(capture))?.or_fail()?;
    ensure_eq!(spec.container(), iso(capture));
    Ok(())
}

/// A capture rate slower than playback is accepted.
#[test]
fn slower_capture_is_accepted() {
    assert_eq!(check_slower_capture_accepted(), Ok(()));
}

fn check_equal_capture_refused_for_ntsc_in_other_spelling() -> Result<(), Fail> {
    // 60000/2002 is 30000/1001 spelled with a common factor of 2, so it is refused.
    let refused = spec_with(
        rate(Frac(30000, 1001))?,
        iso(project_box(rate(Frac(60000, 2002))?)),
    )?;
    ensure_eq!(
        refusal(refused)?,
        VideoSpecError::FrameRates(FrameRatesError::CaptureEqualsPlayback)
    );
    Ok(())
}

/// The NTSC rate spelled with a common factor of two is refused as equal to playback.
#[test]
fn equal_ntsc_capture_is_refused_in_other_spelling() {
    assert_eq!(
        check_equal_capture_refused_for_ntsc_in_other_spelling(),
        Ok(())
    );
}
