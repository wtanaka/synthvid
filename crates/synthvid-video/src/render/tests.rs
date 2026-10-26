use super::*;
use crate::camera::{Camera, Timing};
use crate::json_names::JsonEntryName;
use crate::medium::{
    FilmGrain, GrainBlur, GrainDensity, GrainRadius, GrainSeed, GrainSpread, Q8_8,
};
use crate::optics::{Optics, Vignette, VignetteStrength};
use crate::recorder::Recorder;
use crate::testutil::Frac;
use crate::video_spec::{CaptureRate, Format};
use crate::vignette::apply_vignette;
use core::num::NonZeroU16;
use synthvid_scene::frame::{PixelColumn, PixelCoord, PixelRow};
use synthvid_scene::{
    CaptureCarrier, Channel, DistinctCapture, FrameCount, FrameRate, FrameRatesError, Height,
    Magnification, Motion, Numerator, Point, Ratio, Rgb8, Rotation, Scene, Seed, Turns, View,
    Width, Zoom,
};
use synthvid_testkit::iso::{
    child_path, payload, tags_in, u32_at, BigEndianWord, BoxPath, BoxTag, ByteOffset, IsoBytes,
};
use synthvid_testkit::{ensure, ensure_eq, ensure_ne, Fail, OrFail as _};

/// A distinct capture rate carried in the project-specific box.
fn project_box(rate: FrameRate) -> CaptureRate {
    CaptureRate::Distinct(DistinctCapture {
        rate,
        carrier: CaptureCarrier::ProjectBox,
    })
}

/// A two-frame grayscene with no objects.
fn still_gray_scene() -> Scene {
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

/// A two-frame, 16 by 16 graycamera with a still view, recording at the given playback rate.
fn gray_camera_at(frame_rate: FrameRate) -> Result<Camera, Fail> {
    gray_camera_on(frame_rate, Medium::Off)
}

/// A two-frame, 16 by 16 graycamera with a still view, recording on the given medium.
fn gray_camera_on(frame_rate: FrameRate, medium: Medium) -> Result<Camera, Fail> {
    gray_camera_with(frame_rate, Optics::Off, medium)
}

/// A two-frame, 16 by 16 graycamera with a still view, focusing through the given optics and
/// recording on the given medium.
fn gray_camera_with(frame_rate: FrameRate, optics: Optics, medium: Medium) -> Result<Camera, Fail> {
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
        optics,
        &Recorder::new(
            medium,
            Dimensions::new(Width::new(16).or_fail()?, Height::new(16).or_fail()?),
            Timing::new(frame_rate, FrameCount::new(2).or_fail()?),
        ),
    ))
}

/// Renders the still grayscene as raw frames in the given container.
fn render_still_gray(container: Container) -> Result<GeneratedEntry, Fail> {
    let spec = VideoSpec::new(
        JsonEntryName::new("still-grey").or_fail()?,
        still_gray_scene(),
        gray_camera_at(FrameRate::new(Ratio::from_integer(Numerator(30))).or_fail()?)?,
        Format::new(Encoding::Raw, container, None),
    )
    .or_fail()?;
    render(&spec).or_fail()
}

fn check_raw_iso_render() -> Result<(), Fail> {
    let generated = render_still_gray(Container::Iso {
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
    let generated = render_still_gray(Container::Avi)?;
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

/// The still graycamera at the NTSC rate 30000/1001.
fn ntsc_camera() -> Result<Camera, Fail> {
    gray_camera_at(FrameRate::new(Frac(30000, 1001).ratio()?).or_fail()?)
}

fn ntsc_iso_spec(capture: CaptureRate) -> Result<VideoSpec, Fail> {
    VideoSpec::new(
        JsonEntryName::new("ntsc-capture").or_fail()?,
        still_gray_scene(),
        ntsc_camera()?,
        Format::new(
            Encoding::Raw,
            Container::Iso {
                rotation: DisplayRotation::Identity,
                capture,
            },
            None,
        ),
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
            still_gray_scene(),
            ntsc_camera()?,
            Format::new(Encoding::Raw, equal, None),
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
        still_gray_scene(),
        gray_camera_at(FrameRate::new(Ratio::from_integer(Numerator(30))).or_fail()?)?,
        Format::new(
            Encoding::Raw,
            Container::Iso {
                rotation: DisplayRotation::Identity,
                capture: project_box(capture),
            },
            None,
        ),
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

/// The still gray scene recorded on the given medium, as raw frames in an ISO container.
fn render_gray_on(medium: Medium) -> Result<GeneratedEntry, Fail> {
    render_gray_with(Optics::Off, medium)
}

/// The still gray scene focused through the given optics and recorded on the given medium, as
/// raw frames in an ISO container.
fn render_gray_with(optics: Optics, medium: Medium) -> Result<GeneratedEntry, Fail> {
    render(&gray_spec_with(optics, medium)?).or_fail()
}

/// The specification of the still gray scene focused through the given optics and recorded on
/// the given medium, as raw frames in an ISO container.
fn gray_spec_with(optics: Optics, medium: Medium) -> Result<VideoSpec, Fail> {
    let camera = gray_camera_with(
        FrameRate::new(Ratio::from_integer(Numerator(30))).or_fail()?,
        optics,
        medium,
    )?;
    VideoSpec::new(
        JsonEntryName::new("gray-on-medium").or_fail()?,
        still_gray_scene(),
        camera,
        Format::new(
            Encoding::Raw,
            Container::Iso {
                rotation: DisplayRotation::Identity,
                capture: CaptureRate::PlaybackOnly,
            },
            None,
        ),
    )
    .or_fail()
}

/// A camera on film grains its frames, so its media differs from the same camera with no medium,
/// while the camera with no medium renders the same bytes every time.
#[test]
fn render_film_medium_grains_the_frames() {
    assert_eq!(check_render_film_medium_grains_the_frames(), Ok(()));
}

fn check_render_film_medium_grains_the_frames() -> Result<(), Fail> {
    let grain = FilmGrain::new(
        GrainRadius::new(Frac(3, 100).ratio()?).or_fail()?,
        GrainSpread(Q8_8(40)),
        GrainDensity::new(NonZeroU16::new(300).or_fail()?).or_fail()?,
        GrainBlur(Q8_8(12)),
        GrainSeed::new(Seed::new(0xF11A_6A11)),
    );
    let plain = render_gray_on(Medium::Off)?;
    let plain_again = render_gray_on(Medium::Off)?;
    let grained = render_gray_on(Medium::Film(grain))?;
    ensure_eq!(plain.media_bytes(), plain_again.media_bytes());
    ensure_ne!(grained.media_bytes(), plain.media_bytes());
    Ok(())
}

/// A 240/1 capture over 30/1 playback writes the capture rate and a 30 timescale.
#[test]
fn render_240_capture_over_30_playback() {
    assert_eq!(check_render_240_capture_over_30_playback(), Ok(()));
}

fn check_render_off_optics_leaves_frames_unchanged() -> Result<(), Fail> {
    let off = render_gray_with(Optics::Off, Medium::Off)?;
    let plain = render_gray_on(Medium::Off)?;
    ensure_eq!(off.media_bytes(), plain.media_bytes());
    Ok(())
}

/// A camera with ideal optics records the same media as the scene's default camera, on every render.
#[test]
fn render_off_optics_leaves_frames_unchanged() {
    assert_eq!(check_render_off_optics_leaves_frames_unchanged(), Ok(()));
}

fn check_render_vignette_optics_changes_frames() -> Result<(), Fail> {
    let strength = VignetteStrength::new(NonZeroU16::new(128).or_fail()?).or_fail()?;
    let vignetted = render_gray_with(Optics::Vignette(strength), Medium::Off)?;
    let again = render_gray_with(Optics::Vignette(strength), Medium::Off)?;
    let plain = render_gray_on(Medium::Off)?;
    ensure_ne!(vignetted.media_bytes(), plain.media_bytes());
    ensure_eq!(vignetted.media_bytes(), again.media_bytes());
    Ok(())
}

/// A vignette of a flat camera dims its frames, and the same camera renders the same bytes twice.
#[test]
fn render_vignette_optics_changes_frames() {
    assert_eq!(check_render_vignette_optics_changes_frames(), Ok(()));
}

/// The pixel at the given column and row of a frame.
fn pixel_at(frame: &Frame, column: PixelColumn, row: PixelRow) -> Result<Rgb8, Fail> {
    frame.get_pixel(PixelCoord::new(column, row)).or_fail()
}

fn check_flat_vignette_dims_a_two_by_two_frame() -> Result<(), Fail> {
    // Every sensel of a 2 by 2 frame has a doubled offset of magnitude 1 on each axis, so its
    // squared radius fraction is (1 + 1) / (2^2 + 2^2) = 2/8. The loss is 128 * 256 * 2 / 8 = 8192
    // parts per 65536, which leaves a factor of 57344. A channel of 200 becomes
    // floor((200 * 57344 + 32768) / 65536) = floor(175.5) = 175.
    let dimensions = Dimensions::new(Width::new(2).or_fail()?, Height::new(2).or_fail()?);
    let solid = Channel(200);
    let mut frame = Frame::from_color(
        dimensions,
        Rgb8 {
            red: solid,
            green: solid,
            blue: solid,
        },
    )
    .or_fail()?;
    let strength = VignetteStrength::new(NonZeroU16::new(128).or_fail()?).or_fail()?;
    apply_vignette(&mut frame, Vignette::Radial(strength)).or_fail()?;
    let dimmed = Channel(175);
    let sensels = [
        (PixelColumn(0), PixelRow(0)),
        (PixelColumn(1), PixelRow(0)),
        (PixelColumn(0), PixelRow(1)),
        (PixelColumn(1), PixelRow(1)),
    ];
    for (column, row) in sensels {
        let color = pixel_at(&frame, column, row)?;
        ensure_eq!(
            color,
            Rgb8 {
                red: dimmed,
                green: dimmed,
                blue: dimmed,
            }
        );
    }
    Ok(())
}

/// A 2 by 2 solid frame of 200 under a half-strength vignette falls to 175 in every channel.
#[test]
fn flat_vignette_dims_a_two_by_two_frame() {
    assert_eq!(check_flat_vignette_dims_a_two_by_two_frame(), Ok(()));
}
