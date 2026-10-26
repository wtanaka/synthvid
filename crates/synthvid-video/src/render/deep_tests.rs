use super::*;
use crate::camera::{Camera, Timing};
use crate::json_names::JsonEntryName;
use crate::medium::{
    ColumnSigma, PatternSeed, PrnuSpread, ReadSigma, SensorNoise, SensorSeeds, ShotGain,
    TemporalSeed, Q8_8,
};
use crate::optics::{Aperture, FocalLength, Lens, Optics, Vignette, VignetteStrength};
use crate::recorder::Recorder;
use crate::rig::{Pose, Rig};
use crate::testutil::Frac;
use crate::video_spec::Format;
use crate::vignette::apply_vignette;
use core::num::{NonZeroU16, NonZeroU32, NonZeroU8};
use core::ops::Range;
use synthvid_scene::frame::{PixelColumn, PixelCoord, PixelRow};
use synthvid_scene::{
    Channel, Coord, Cutout, Extent, FrameCount, FrameRate, FrameSpan, Height, Layer, Magnification,
    Motion, Numerator, PixelGrid, Point, Position, Ratio, RawCoord, Rgb8, Rotation, Scene, Seed,
    Stage, Track, Turns, View, Width, Zoom,
};
use synthvid_testkit::{ensure, ensure_eq, ensure_ne, Fail, OrFail as _};

/// Eight sensels along each side of the recorder in these tests.
fn eight_by_eight() -> Result<Dimensions, Fail> {
    Ok(Dimensions::new(
        Width::new(8).or_fail()?,
        Height::new(8).or_fail()?,
    ))
}

/// A recorder of eight by eight sensels that records on the given medium, at the given rate,
/// with one picture.
fn recorder_at(medium: Medium, rate: FrameRate) -> Result<Recorder, Fail> {
    let timing = Timing::new(rate, FrameCount::new(1).or_fail()?);
    Ok(Recorder::new(medium, eight_by_eight()?, timing))
}

/// Thirty frames per second.
fn thirty() -> Result<FrameRate, Fail> {
    FrameRate::from_fraction(30, NonZeroU32::MIN).or_fail()
}

/// A pinhole lens with a focal length of one frame height, and the given vignette.
fn lens(vignette: Vignette) -> Result<Lens, Fail> {
    Ok(Lens::new(
        FocalLength(NonZeroU8::new(16).or_fail()?),
        Aperture::Pinhole,
        vignette,
    ))
}

/// A pose at the origin with no roll.
fn upright_pose() -> Result<Pose, Fail> {
    let zero = Coord::new(RawCoord(0)).or_fail()?;
    let origin = Position {
        across: zero,
        down: zero,
        depth: zero,
    };
    Ok(Pose::new(
        Track::Fixed(origin),
        Rotation::Fixed(Turns::new(Ratio::from_integer(Numerator(0)))),
    ))
}

/// Pure white.
fn white() -> Rgb8 {
    Rgb8 {
        red: Channel(255),
        green: Channel(255),
        blue: Channel(255),
    }
}

/// Pure black.
fn black() -> Rgb8 {
    Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    }
}

/// A white square layer of half size one, at depth eight on the optical axis.
fn white_block() -> Result<Layer, Fail> {
    let half = Extent::new(NonZeroU32::MIN).or_fail()?;
    let at_depth_eight = Position {
        across: Coord::new(RawCoord(0)).or_fail()?,
        down: Coord::new(RawCoord(0)).or_fail()?,
        depth: Coord::new(RawCoord(8)).or_fail()?,
    };
    let frames: Range<FrameIndex> = FrameIndex::new(0)..FrameIndex::new(1);
    Ok(Layer {
        cutout: Cutout::Rect {
            half_width: half,
            half_height: half,
        },
        fill: white(),
        track: Track::Fixed(at_depth_eight),
        visible: FrameSpan::new(frames).or_fail()?,
    })
}

/// A deep shot of the white block on a black backdrop, seen through the given lens and recorded
/// on the given medium.
fn block_shot(lens: Lens, medium: Medium) -> Result<DeepShot, Fail> {
    let rig = Rig::new(&upright_pose()?, lens, &recorder_at(medium, thirty()?)?);
    let stage = Stage {
        backdrop: black(),
        layers: vec![white_block()?],
    };
    DeepShot::new(stage, &rig, PixelGrid::One).or_fail()
}

/// A specification of a deep shot, written as raw frames in an AVI container.
fn deep_spec(shot: DeepShot) -> Result<VideoSpec, Fail> {
    VideoSpec::deep(
        JsonEntryName::new("deep-block").or_fail()?,
        shot,
        Format::new(Encoding::Raw, Container::Avi, None),
    )
    .or_fail()
}

/// The plain frame of the white block: the sensels in columns and rows three and four are white,
/// and every other sensel is black.
fn expected_block() -> Result<Frame, Fail> {
    let mut frame = Frame::zeroed(eight_by_eight()?).or_fail()?;
    for column in 3..=4 {
        for row in 3..=4 {
            let coord = PixelCoord::new(PixelColumn(column), PixelRow(row));
            frame.set_pixel(coord, white()).or_fail()?;
        }
    }
    Ok(frame)
}

/// Renders the frames of a deep shot, and checks that there is exactly one.
fn single_frame(shot: &DeepShot) -> Result<Frame, Fail> {
    let frames = render_deep_shot_frames(shot).or_fail()?;
    ensure_eq!(frames.len(), 1);
    frames.into_iter().next().or_fail()
}

/// The color of the sensel at the given column and row.
fn pixel_at(frame: &Frame, column: PixelColumn, row: PixelRow) -> Result<Rgb8, Fail> {
    frame.get_pixel(PixelCoord::new(column, row)).or_fail()
}

/// Sensor noise with every term present and awkward seeds, so no test relies on a convenient
/// value.
fn sensor_noise() -> Result<SensorNoise, Fail> {
    SensorNoise::new(
        ShotGain(Q8_8(0x0020)),
        ReadSigma(Q8_8(0x0100)),
        PrnuSpread(0x0010),
        ColumnSigma(Q8_8(0x0080)),
        SensorSeeds::new(
            TemporalSeed::new(Seed(0x0BAD_5EED_1234_5677)),
            PatternSeed::new(Seed(0xC0FF_EE00_DEAD_BEE3)),
        ),
    )
    .or_fail()
}

/// A radial vignette of raw strength 128, which is one half in Q8.8.
fn half_strength() -> Result<Vignette, Fail> {
    let raw = NonZeroU16::new(128).or_fail()?;
    let strength = VignetteStrength::new(raw).or_fail()?;
    Ok(Vignette::Radial(strength))
}

/// The deep frame samples the block into the expected sensels.
fn check_deep_frame_samples_the_block() -> Result<(), Fail> {
    let shot = block_shot(lens(Vignette::None)?, Medium::Off)?;
    let frame = single_frame(&shot)?;
    ensure_eq!(frame, expected_block()?);
    ensure_eq!(pixel_at(&frame, PixelColumn(3), PixelRow(3))?, white());
    ensure_eq!(pixel_at(&frame, PixelColumn(0), PixelRow(0))?, black());
    Ok(())
}

/// A one-frame deep shot of an eight by eight recorder samples the white block into columns and
/// rows three and four.
#[test]
fn deep_frame_samples_the_block() {
    assert_eq!(check_deep_frame_samples_the_block(), Ok(()));
}

/// The deep frame equals the plain frame passed through the vignette.
fn check_deep_frame_is_vignetted() -> Result<(), Fail> {
    let vignette = half_strength()?;
    let shot = block_shot(lens(vignette)?, Medium::Off)?;
    let frame = single_frame(&shot)?;
    let mut expected = expected_block()?;
    apply_vignette(&mut expected, vignette).or_fail()?;
    ensure_eq!(frame, expected);
    ensure!(
        pixel_at(&frame, PixelColumn(3), PixelRow(3))?.red.0 < 255,
        "the sensel at the center of the block must be darker than white"
    );
    ensure_eq!(pixel_at(&frame, PixelColumn(0), PixelRow(0))?, black());
    Ok(())
}

/// The lens vignette darkens the block and leaves the black sensels black.
#[test]
fn deep_frame_is_vignetted() {
    assert_eq!(check_deep_frame_is_vignetted(), Ok(()));
}

/// The deep frame equals the plain frame passed through the sensor noise.
fn check_deep_frame_takes_sensor_noise() -> Result<(), Fail> {
    let noise = sensor_noise()?;
    let shot = block_shot(lens(Vignette::None)?, Medium::Sensor(noise))?;
    let frame = single_frame(&shot)?;
    let mut expected = expected_block()?;
    apply_sensor_noise(&mut expected, &noise, FrameIndex::new(0)).or_fail()?;
    ensure_ne!(frame, expected_block()?);
    ensure_eq!(frame, expected);
    Ok(())
}

/// The sensor medium of the recorder adds the same noise that the flat pipeline adds at frame
/// index zero.
#[test]
fn deep_frame_takes_sensor_noise() {
    assert_eq!(check_deep_frame_takes_sensor_noise(), Ok(()));
}

/// The vignette and the sensor noise combine in that order.
fn check_vignette_comes_before_medium() -> Result<(), Fail> {
    let noise = sensor_noise()?;
    let vignette = half_strength()?;
    let shot = block_shot(lens(vignette)?, Medium::Sensor(noise))?;
    let frame = single_frame(&shot)?;

    let mut vignette_first = expected_block()?;
    apply_vignette(&mut vignette_first, vignette).or_fail()?;
    apply_sensor_noise(&mut vignette_first, &noise, FrameIndex::new(0)).or_fail()?;
    ensure_eq!(frame, vignette_first);

    let mut medium_first = expected_block()?;
    apply_sensor_noise(&mut medium_first, &noise, FrameIndex::new(0)).or_fail()?;
    apply_vignette(&mut medium_first, vignette).or_fail()?;
    ensure_ne!(frame, medium_first);
    Ok(())
}

/// The lens vignette is applied before the recorder's medium.
#[test]
fn vignette_comes_before_medium() {
    assert_eq!(check_vignette_comes_before_medium(), Ok(()));
}

/// Rendering a deep shot writes the manifest the shot describes, and media that is non-empty and
/// the same on every render.
fn check_render_writes_a_deep_manifest() -> Result<(), Fail> {
    let shot = block_shot(lens(Vignette::None)?, Medium::Off)?;
    let spec = deep_spec(shot.clone())?;
    let name = JsonEntryName::new("deep-block").or_fail()?;
    let expected = deep_manifest_json(&name, &shot).or_fail()?;
    let entry = render(&spec).or_fail()?;
    ensure_eq!(entry.manifest_json(), &expected);
    ensure!(
        !entry.media_bytes().as_bytes().is_empty(),
        "the media of a deep shot is empty"
    );
    let again = render(&spec).or_fail()?;
    ensure_eq!(
        again.media_bytes().as_bytes(),
        entry.media_bytes().as_bytes()
    );
    Ok(())
}

/// A deep shot renders to its manifest and to media, with no refusal.
#[test]
fn render_writes_a_deep_manifest() {
    assert_eq!(check_render_writes_a_deep_manifest(), Ok(()));
}

/// A flat specification of a plain scene and camera, for the refusal tests.
fn flat_spec() -> Result<VideoSpec, Fail> {
    let origin = Point {
        across: Ratio::from_integer(Numerator(0)),
        down: Ratio::from_integer(Numerator(0)),
    };
    let view = View::new(
        Motion::Fixed(origin),
        Rotation::Fixed(Turns::new(Ratio::from_integer(Numerator(0)))),
        Zoom::Fixed(Magnification::new(Ratio::from_integer(Numerator(1))).or_fail()?),
    );
    let recorder = recorder_at(Medium::Off, thirty()?)?;
    let camera = Camera::new(view, Optics::Off, &recorder);
    let scene = Scene {
        scale: None,
        background: Background::Solid(black()),
        objects: Vec::new(),
    };
    VideoSpec::new(
        JsonEntryName::new("flat-block").or_fail()?,
        scene,
        camera,
        Format::new(Encoding::Raw, Container::Avi, None),
    )
    .or_fail()
}

/// The playback rate of a deep specification comes from its rig.
fn check_deep_playback_rate_comes_from_the_rig() -> Result<(), Fail> {
    let playback = FrameRate::new(Frac(30000, 1001).ratio()?).or_fail()?;
    let recorder = recorder_at(Medium::Off, playback)?;
    let rig = Rig::new(&upright_pose()?, lens(Vignette::None)?, &recorder);
    let shot = DeepShot::new(
        Stage {
            backdrop: black(),
            layers: Vec::new(),
        },
        &rig,
        PixelGrid::One,
    )
    .or_fail()?;
    let spec = deep_spec(shot)?;
    ensure_eq!(spec.frame_rates().or_fail()?.playback(), playback);
    Ok(())
}

/// The playback rate of a deep specification is the rate of its rig's recorder, here the NTSC
/// rate 30000/1001.
#[test]
fn deep_playback_rate_comes_from_the_rig() {
    assert_eq!(check_deep_playback_rate_comes_from_the_rig(), Ok(()));
}

/// A deep specification holds its shot, records through the rig, and has no flat parts.
fn check_deep_spec_holds_its_shot_and_recorder() -> Result<(), Fail> {
    let shot = block_shot(lens(Vignette::None)?, Medium::Off)?;
    let recorder = shot.rig().recorder();
    let spec = deep_spec(shot)?;
    match spec.shot() {
        Shot::Deep(_) => {}
        Shot::Flat(_) => {
            return Err(Fail::here(format_args!(
                "a deep specification must hold a deep shot"
            )))
        }
    }
    ensure_eq!(spec.recorder(), recorder);
    ensure_eq!(spec.scene(), None);
    ensure_eq!(spec.camera(), None);
    Ok(())
}

/// A deep specification holds its deep shot, records through the rig's recorder, and has no
/// flat scene or camera.
#[test]
fn deep_spec_holds_its_shot_and_recorder() {
    assert_eq!(check_deep_spec_holds_its_shot_and_recorder(), Ok(()));
}

/// The flat shot used by the refusal test keeps its scene and camera.
fn check_flat_spec_holds_a_flat_shot() -> Result<(), Fail> {
    let spec = flat_spec()?;
    let flat = spec.shot().flat().or_fail()?;
    ensure_eq!(spec.camera(), Some(flat.camera()));
    Ok(())
}

/// A flat specification keeps the flat shot it was built with.
#[test]
fn flat_spec_holds_a_flat_shot() {
    assert_eq!(check_flat_spec_holds_a_flat_shot(), Ok(()));
}
