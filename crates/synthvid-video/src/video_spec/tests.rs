use super::*;
use crate::camera::Timing;
use crate::medium::Medium;
use crate::optics::Optics;
use crate::testutil::Frac;
use synthvid_scene::{
    Background, CaptureCarrier, Channel, Dimensions, FrameCount, Height, Magnification, Motion,
    Numerator, Point, Ratio, Rgb8, Rotation, Turns, View, Width, Zoom,
};
use synthvid_testkit::{ensure_eq, refusal, Fail, OrFail as _};

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

/// A two-frame gray scene with no objects.
fn scene() -> Scene {
    Scene {
        scale: None,
        background: Background::Solid(Rgb8 {
            red: Channel(128),
            green: Channel(128),
            blue: Channel(128),
        }),
        objects: Vec::new(),
    }
}

/// A still camera of 16 by 16 sensels that records two frames at the given playback rate.
fn camera_at(frame_rate: FrameRate) -> Result<Camera, Fail> {
    let origin = Point {
        across: Ratio::from_integer(Numerator(0)),
        down: Ratio::from_integer(Numerator(0)),
    };
    let view = View::new(
        Motion::Fixed(origin),
        Rotation::Fixed(Turns::new(Ratio::from_integer(Numerator(0)))),
        Zoom::Fixed(Magnification::new(Ratio::from_integer(Numerator(1))).or_fail()?),
    );
    Ok(Camera::new(
        view,
        Optics::Off,
        &Recorder::new(
            Medium::Off,
            Dimensions::new(Width::new(16).or_fail()?, Height::new(16).or_fail()?),
            Timing::new(frame_rate, FrameCount::new(2).or_fail()?),
        ),
    ))
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
        scene(),
        camera_at(playback)?,
        Format::new(Encoding::Raw, container, None),
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

fn check_playback_rate_comes_from_camera() -> Result<(), Fail> {
    let playback = rate(Frac(30000, 1001))?;
    let spec = spec_with(playback, Container::Avi)?.or_fail()?;
    ensure_eq!(spec.recorder().timing().rate(), playback);
    ensure_eq!(spec.frame_rates().or_fail()?.playback(), playback);
    Ok(())
}

/// The playback rate of a specification is the rate its camera's timing declares, here the
/// awkward NTSC rate 30000/1001.
#[test]
fn playback_rate_comes_from_camera() {
    assert_eq!(check_playback_rate_comes_from_camera(), Ok(()));
}

fn check_camera_round_trips() -> Result<(), Fail> {
    let camera = camera_at(rate(Frac(30000, 1001))?)?;
    let spec = VideoSpec::new(
        JsonEntryName::new("camera-spec").or_fail()?,
        scene(),
        camera.clone(),
        Format::new(Encoding::Raw, Container::Avi, None),
    )
    .or_fail()?;
    ensure_eq!(spec.camera(), Some(&camera));
    Ok(())
}

/// A specification returns the camera it was built with.
#[test]
fn camera_round_trips() {
    assert_eq!(check_camera_round_trips(), Ok(()));
}

/// A specification built from a scene and camera holds a flat shot of them.
fn check_built_spec_holds_flat_shot() -> Result<(), Fail> {
    let spec = spec_with(rate(Frac(30000, 1001))?, Container::Avi)?.or_fail()?;
    match spec.shot() {
        Shot::Flat(flat) => {
            ensure_eq!(flat.scene(), &scene());
            ensure_eq!(spec.recorder(), flat.camera().recorder());
            Ok(())
        }
        Shot::Deep(_) => Err(Fail::here(format_args!(
            "a specification built from a scene and camera must hold a flat shot"
        ))),
    }
}

/// A specification built from a scene and a camera holds a flat shot of them.
#[test]
fn built_spec_holds_flat_shot() {
    assert_eq!(check_built_spec_holds_flat_shot(), Ok(()));
}
