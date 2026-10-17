use super::*;
use crate::json_names::JsonEntryName;
use synthvid_scene::{
    Channel, FrameCount, FrameRate, Height, Magnification, Motion, Numerator, Point, Ratio, Rgb8,
    Rotation, Turns, Width, Zoom,
};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// A two-frame, 16 by 16 grey scene with a still camera and no objects.
fn still_grey_scene() -> Result<Scene, Fail> {
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
        frame_rate: FrameRate::new(Ratio::from_integer(Numerator(30))).or_fail()?,
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

/// Renders the still grey scene as raw frames in the given container.
fn render_still_grey(container: Container) -> Result<GeneratedEntry, Fail> {
    let spec = VideoSpec::new(
        JsonEntryName::new("still-grey").or_fail()?,
        still_grey_scene()?,
        Encoding::Raw,
        container,
        None,
    )
    .or_fail()?;
    render(&spec).or_fail()
}

fn check_raw_iso_render() -> Result<(), Fail> {
    let generated = render_still_grey(Container::Iso {
        rotation: DisplayRotation::Identity,
    })?;
    let media = generated.media_bytes().as_bytes();
    // An ISO file opens with a box whose type is `ftyp`.
    ensure_eq!(media.get(4..8).or_fail()?, b"ftyp".as_slice());
    let manifest = generated.manifest_json().as_str();
    ensure!(
        manifest.contains(r#""frame_count":2"#),
        "manifest must record both frames: {manifest}"
    );
    ensure!(
        manifest.contains(r#""name":"still-grey""#),
        "manifest must record the specification's name: {manifest}"
    );
    Ok(())
}

/// A raw ISO rendering opens with `ftyp` and its manifest records the scene.
#[test]
fn raw_iso_render() {
    assert_eq!(check_raw_iso_render(), Ok(()));
}

fn check_raw_avi_render() -> Result<(), Fail> {
    let generated = render_still_grey(Container::Avi)?;
    let media = generated.media_bytes().as_bytes();
    // An AVI file is a RIFF file of form `AVI `.
    ensure_eq!(media.get(..4).or_fail()?, b"RIFF".as_slice());
    ensure_eq!(media.get(8..12).or_fail()?, b"AVI ".as_slice());
    Ok(())
}

/// A raw AVI rendering is a RIFF file of form `AVI `.
#[test]
fn raw_avi_render() {
    assert_eq!(check_raw_avi_render(), Ok(()));
}
