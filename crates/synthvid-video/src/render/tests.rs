use super::*;
use crate::json_names::JsonEntryName;
use crate::testutil::Frac;
use crate::video_spec::CaptureRate;
use synthvid_scene::{
    CaptureCarrier, Channel, DistinctCapture, FrameCount, FrameRate, FrameRatesError, Height,
    Magnification, Motion, Numerator, Point, Ratio, Rgb8, Rotation, Turns, Width, Zoom,
};
use synthvid_testkit::iso::{
    child_path, payload, tags_in, u32_at, BigEndianWord, BoxPath, BoxTag, ByteOffset, IsoBytes,
};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// A distinct capture rate carried in the project-specific box.
fn project_box(rate: FrameRate) -> CaptureRate {
    CaptureRate::Distinct(DistinctCapture {
        rate,
        carrier: CaptureCarrier::ProjectBox,
    })
}

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
        capture: CaptureRate::PlaybackOnly,
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

/// The still grey scene at the NTSC rate 30000/1001.
fn ntsc_scene() -> Result<Scene, Fail> {
    let mut scene = still_grey_scene()?;
    scene.frame_rate = FrameRate::new(Frac(30000, 1001).ratio()?).or_fail()?;
    Ok(scene)
}

fn ntsc_iso_spec(capture: CaptureRate) -> Result<VideoSpec, Fail> {
    VideoSpec::new(
        JsonEntryName::new("ntsc-capture").or_fail()?,
        ntsc_scene()?,
        Encoding::Raw,
        Container::Iso {
            rotation: DisplayRotation::Identity,
            capture,
        },
        None,
    )
    .or_fail()
}

fn check_capture_render() -> Result<(), Fail> {
    let capture = FrameRate::new(Frac(24, 1).ratio()?).or_fail()?;
    let spec = ntsc_iso_spec(project_box(capture))?;
    let generated = render(&spec).or_fail()?;
    let media = generated.media_bytes().as_bytes();
    // Media timing follows the playback rate 30000/1001, so the mdhd timescale is 30000.
    // The timescale is 20 bytes into the mdhd box: 8 header, 4 version and flags,
    // 4 creation time, 4 modification time.
    let mdhd = child_path(
        IsoBytes(media),
        BoxPath(&[
            BoxTag(*b"moov"),
            BoxTag(*b"trak"),
            BoxTag(*b"mdia"),
            BoxTag(*b"mdhd"),
        ]),
    )?;
    ensure_eq!(u32_at(mdhd.0, ByteOffset(20))?, BigEndianWord(30000));
    let trak = payload(child_path(
        IsoBytes(media),
        BoxPath(&[BoxTag(*b"moov"), BoxTag(*b"trak")]),
    )?)?;
    ensure!(
        !tags_in(trak)?.contains(&BoxTag(*b"edts")),
        "a distinct capture rate must not write an edts box"
    );
    // The capture rate 24/1 is the cfps payload: version and flags (4 bytes),
    // numerator 24 (0x18), denominator 1.
    let cfps = child_path(
        IsoBytes(media),
        BoxPath(&[BoxTag(*b"moov"), BoxTag(*b"udta"), BoxTag(*b"cfps")]),
    )?;
    ensure_eq!(
        payload(cfps)?,
        IsoBytes(&[0, 0, 0, 0, 0, 0, 0, 0x18, 0, 0, 0, 1])
    );
    let manifest = generated.manifest_json().as_str();
    ensure!(
        manifest.contains(r#""frame_rate":{"den":1001,"num":30000}"#),
        "manifest must state the playback rate: {manifest}"
    );
    ensure!(
        manifest.contains(r#""capture_rate":{"den":1,"num":24}"#),
        "manifest must state the capture rate: {manifest}"
    );
    let again = render(&spec).or_fail()?;
    ensure_eq!(again.media_bytes().as_bytes(), media);
    ensure_eq!(again.manifest_json().as_str(), manifest);
    Ok(())
}

/// An ISO file with a distinct capture rate carries it in a cfps box, times media at the
/// playback rate, and its manifest
/// states both rates; rendering twice is byte-identical.
#[test]
fn capture_rate_render() {
    assert_eq!(check_capture_render(), Ok(()));
}

fn check_equal_capture_is_refused_at_construction() -> Result<(), Fail> {
    let playback = FrameRate::new(Frac(30000, 1001).ratio()?).or_fail()?;
    let equal = Container::Iso {
        rotation: DisplayRotation::Identity,
        capture: project_box(playback),
    };
    let refused = Err(VideoSpecError::FrameRates(
        FrameRatesError::CaptureEqualsPlayback,
    ));
    let spec = ntsc_iso_spec(CaptureRate::PlaybackOnly)?;
    ensure_eq!(spec.with_container(equal).map(|_spec| ()), refused);
    ensure_eq!(
        VideoSpec::new(
            JsonEntryName::new("equal-capture").or_fail()?,
            ntsc_scene()?,
            Encoding::Raw,
            equal,
            None,
        )
        .map(|_spec| ()),
        refused
    );
    Ok(())
}

/// A capture rate equal to playback is refused when the specification is built, whether by
/// `new` or `with_container`, so no render can meet one.
#[test]
fn equal_capture_is_refused_at_construction() {
    assert_eq!(check_equal_capture_is_refused_at_construction(), Ok(()));
}

fn check_render_240_capture_over_30_playback() -> Result<(), Fail> {
    let capture = FrameRate::new(Frac(240, 1).ratio()?).or_fail()?;
    let spec = VideoSpec::new(
        JsonEntryName::new("fast-capture").or_fail()?,
        still_grey_scene()?,
        Encoding::Raw,
        Container::Iso {
            rotation: DisplayRotation::Identity,
            capture: project_box(capture),
        },
        None,
    )
    .or_fail()?;
    let generated = render(&spec).or_fail()?;
    let media = generated.media_bytes().as_bytes();
    // Playback is 30/1, so the mdhd timescale is 30.
    let mdhd = child_path(
        IsoBytes(media),
        BoxPath(&[
            BoxTag(*b"moov"),
            BoxTag(*b"trak"),
            BoxTag(*b"mdia"),
            BoxTag(*b"mdhd"),
        ]),
    )?;
    ensure_eq!(u32_at(mdhd.0, ByteOffset(20))?, BigEndianWord(30));
    // The capture rate 240/1: numerator 240 (0xF0), denominator 1.
    let cfps = child_path(
        IsoBytes(media),
        BoxPath(&[BoxTag(*b"moov"), BoxTag(*b"udta"), BoxTag(*b"cfps")]),
    )?;
    ensure_eq!(
        payload(cfps)?,
        IsoBytes(&[0, 0, 0, 0, 0, 0, 0, 0xF0, 0, 0, 0, 1])
    );
    let manifest = generated.manifest_json().as_str();
    ensure!(
        manifest.contains(r#""frame_rate":{"den":1,"num":30}"#),
        "manifest must state the playback rate: {manifest}"
    );
    ensure!(
        manifest.contains(r#""capture_rate":{"den":1,"num":240}"#),
        "manifest must state the capture rate: {manifest}"
    );
    Ok(())
}

/// A 240/1 capture over 30/1 playback writes the capture rate and a 30 timescale.
#[test]
fn render_240_capture_over_30_playback() {
    assert_eq!(check_render_240_capture_over_30_playback(), Ok(()));
}
