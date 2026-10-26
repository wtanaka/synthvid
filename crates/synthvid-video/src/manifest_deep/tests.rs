use core::fmt;
use core::num::{NonZeroU16, NonZeroU32, NonZeroU8};
use core::ops::Range;

use synthvid_scene::{
    Affine, Channel, Coord, Cutout, Dimensions, Extent, FrameCount, FrameIndex, FrameRate,
    FrameRates, FrameSpan, Height, Layer, LocalPoint, Numerator, Outline, PerFrame, PixelGrid,
    Position, Ratio, RawCoord, Rgb8, Rotation, Seed, Stage, Track, Turns, Velocity, Width,
};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

use super::deep_manifest_json;
use crate::camera::Timing;
use crate::json_names::JsonEntryName;
use crate::manifest::{Manifest, ManifestAffine, ManifestCamera, ManifestFrame, ManifestHeader};
use crate::medium::{
    ColumnSigma, Medium, PatternSeed, PrnuSpread, ReadSigma, SensorNoise, SensorSeeds, ShotGain,
    TemporalSeed, Q8_8,
};
use crate::medium_json::medium_object;
use crate::names::BackgroundName;
use crate::optics::{
    Aperture, ApertureGrid, ApertureShape, BladeCount, FStop, FocalLength, Focus, FocusDistance,
    FrameHeight, Iris, Lens, Optics, Vignette, VignetteStrength,
};
use crate::recorder::Recorder;
use crate::rig::{DeepShot, Pose, Rig};

/// Rgb8 pure white.
fn white() -> Rgb8 {
    Rgb8 {
        red: Channel(255),
        green: Channel(255),
        blue: Channel(255),
    }
}

/// Rgb8 pure black.
fn black() -> Rgb8 {
    Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    }
}

/// A frame span over the given frames.
fn span(frames: Range<FrameIndex>) -> Result<FrameSpan, Fail> {
    FrameSpan::new(frames).or_fail()
}

/// A world coordinate at the given raw value.
fn coord(raw: RawCoord) -> Result<Coord, Fail> {
    Coord::new(raw).or_fail()
}

/// The raw coordinates of a position on the three axes, each named so that none can be
/// transposed with another.
#[derive(Copy, Clone)]
struct Offsets {
    across: RawCoord,
    down: RawCoord,
    depth: RawCoord,
}

/// A position at the given coordinates on the three axes.
fn at(offsets: Offsets) -> Result<Position, Fail> {
    Ok(Position {
        across: coord(offsets.across)?,
        down: coord(offsets.down)?,
        depth: coord(offsets.depth)?,
    })
}

/// A stationary camera at the origin with no roll.
fn still_pose() -> Result<Pose, Fail> {
    Ok(Pose::new(
        Track::Fixed(at(Offsets {
            across: RawCoord(0),
            down: RawCoord(0),
            depth: RawCoord(0),
        })?),
        Rotation::Fixed(Turns::new(Ratio::from_integer(Numerator(0)))),
    ))
}

/// A recorder of eight by eight sensels on the given medium, at thirty frames per second, taking
/// the given number of pictures.
fn recorder(medium: Medium, count: FrameCount) -> Result<Recorder, Fail> {
    let dimensions = Dimensions::new(Width::new(8).or_fail()?, Height::new(8).or_fail()?);
    let rate = FrameRate::from_fps(30).or_fail()?;
    Ok(Recorder::new(medium, dimensions, Timing::new(rate, count)))
}

/// A layer of the given extent as a disc, in white, on the given track and visible span.
fn disc_layer(radius: Extent, track: Track, visible: FrameSpan) -> Layer {
    Layer {
        cutout: Cutout::Disc { radius },
        fill: white(),
        track,
        visible,
    }
}

/// A one-frame deep shot: a disc of radius three at (10, -5, 400) on a black backdrop, seen
/// through the given lens and recorded on the given medium.
fn one_frame_shot(lens: Lens, medium: Medium) -> Result<DeepShot, Fail> {
    let rig = Rig::new(
        &still_pose()?,
        lens,
        &recorder(medium, FrameCount::new(1).or_fail()?)?,
    );
    let disc = disc_layer(
        Extent::new(NonZeroU32::new(3).or_fail()?).or_fail()?,
        Track::Fixed(at(Offsets {
            across: RawCoord(10),
            down: RawCoord(-5),
            depth: RawCoord(400),
        })?),
        span(FrameIndex::new(0)..FrameIndex::new(1))?,
    );
    let stage = Stage {
        backdrop: black(),
        layers: vec![disc],
    };
    DeepShot::new(stage, &rig, PixelGrid::One).or_fail()
}

/// A pinhole lens of the given focal length, with no vignetting.
fn pinhole(focal: FocalLength) -> Lens {
    Lens::new(focal, Aperture::Pinhole, Vignette::None)
}

/// The serialized text of a manifest.
#[derive(Debug, Eq, PartialEq)]
struct ManifestText(pub String);

impl fmt::Display for ManifestText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The canonical text of a deep shot's manifest under the entry name `deep-one`.
fn manifest_text(shot: &DeepShot) -> Result<ManifestText, Fail> {
    let name = JsonEntryName::new("deep-one").or_fail()?;
    Ok(ManifestText(
        deep_manifest_json(&name, shot)
            .or_fail()?
            .as_str()
            .to_owned(),
    ))
}

/// The one-frame manifest, written in full.
const ONE_FRAME_TEXT: &str = r#"{"backdrop":{"blue":0,"green":0,"red":0},"dimensions":{"height":8,"width":8},"frame_count":1,"frame_rate":{"den":1,"num":30},"frames":[{"camera":{"roll":{"den":1,"num":0},"x":0,"y":0,"z":0},"index":0,"objects":[{"index":0,"x":10,"y":-5,"z":400}]}],"name":"deep-one","objects":[{"fill":{"blue":255,"green":255,"red":255},"index":0,"radius":3,"shape":"disc","visible":{"end":1,"start":0}}],"optics":{"aperture":{"kind":"pinhole"},"focal_length":{"den":1,"num":1},"kind":"lens","vignette":{"kind":"none"}},"pixel_samples":1,"projection":"perspective","schema_version":2}"#;

/// The full text of a one-frame deep shot is exactly as the manifest format describes.
#[test]
fn one_frame_deep_manifest_is_written_in_full() {
    assert_eq!(check_one_frame_deep_manifest_is_written_in_full(), Ok(()));
}

/// Writes the one-frame shot and compares the whole text.
fn check_one_frame_deep_manifest_is_written_in_full() -> Result<(), Fail> {
    let focal = FocalLength(NonZeroU8::new(16).or_fail()?);
    let shot = one_frame_shot(pinhole(focal), Medium::Off)?;
    ensure_eq!(
        manifest_text(&shot)?,
        ManifestText(ONE_FRAME_TEXT.to_owned())
    );
    Ok(())
}

/// The optics text of an iris lens with a polygonal outline and a focus distance.
const IRIS_OPTICS_TEXT: &str = r#""optics":{"aperture":{"focus":288,"frame_height":24,"kind":"iris","samples":4,"shape":{"blades":6,"kind":"polygon"},"stop":"f2"},"focal_length":{"den":2,"num":3},"kind":"lens","vignette":{"kind":"vignette","strength":64}},"pixel_samples""#;

/// The optics text of the same iris lens focused at infinity with a round outline.
const INFINITY_OPTICS_TEXT: &str = r#""optics":{"aperture":{"focus":"infinity","frame_height":24,"kind":"iris","samples":4,"shape":{"kind":"disc"},"stop":"f2"},"focal_length":{"den":2,"num":3},"kind":"lens","vignette":{"kind":"vignette","strength":64}},"pixel_samples""#;

/// An iris of stop f/2, a frame height of 24, four samples per axis, and a radial vignette of raw
/// strength 64, on a lens of focal length 24 sixteenths.
fn iris_lens(focus: Focus, shape: ApertureShape) -> Result<Lens, Fail> {
    let iris = Iris::new(
        FStop::F2,
        shape,
        focus,
        FrameHeight(NonZeroU16::new(24).or_fail()?),
        ApertureGrid::Four,
    );
    Ok(Lens::new(
        FocalLength(NonZeroU8::new(24).or_fail()?),
        Aperture::Iris(iris),
        Vignette::Radial(VignetteStrength::new(NonZeroU16::new(64).or_fail()?).or_fail()?),
    ))
}

/// The optics of an iris with a focus distance and a polygonal outline are written in full.
#[test]
fn iris_optics_are_written_in_full() {
    assert_eq!(check_iris_optics_are_written_in_full(), Ok(()));
}

/// Writes the polygonal iris focused at 288 scene units, and checks its optics text.
fn check_iris_optics_are_written_in_full() -> Result<(), Fail> {
    let blades = BladeCount::new(NonZeroU8::new(6).or_fail()?).or_fail()?;
    let focus = Focus::At(FocusDistance::new(NonZeroU32::new(288).or_fail()?).or_fail()?);
    let shot = one_frame_shot(
        iris_lens(focus, ApertureShape::Polygon(blades))?,
        Medium::Off,
    )?;
    let text = manifest_text(&shot)?;
    ensure!(text.0.contains(IRIS_OPTICS_TEXT), "optics: {text}");
    Ok(())
}

/// An iris focused at infinity with a round outline writes `infinity` and a disc kind.
#[test]
fn infinite_iris_optics_are_written_in_full() {
    assert_eq!(check_infinite_iris_optics_are_written_in_full(), Ok(()));
}

/// Writes the round iris focused at infinity, and checks its optics text.
fn check_infinite_iris_optics_are_written_in_full() -> Result<(), Fail> {
    let shot = one_frame_shot(
        iris_lens(Focus::Infinity, ApertureShape::Disc)?,
        Medium::Off,
    )?;
    let text = manifest_text(&shot)?;
    ensure!(text.0.contains(INFINITY_OPTICS_TEXT), "optics: {text}");
    Ok(())
}

/// The declarations of the three layers of the six-frame shot, in declaration order.
const SIX_FRAME_OBJECTS_TEXT: &str = r#""objects":[{"fill":{"blue":30,"green":20,"red":10},"half_height":2,"half_width":5,"index":0,"shape":"rect","visible":{"end":4,"start":2}},{"fill":{"blue":60,"green":50,"red":40},"index":1,"shape":"polygon","vertices":[{"x":0,"y":-3},{"x":3,"y":2},{"x":-3,"y":2}],"visible":{"end":6,"start":0}},{"fill":{"blue":3,"green":2,"red":1},"index":2,"radius":3,"shape":"disc","visible":{"end":4,"start":0}}]"#;

/// The six frames of the three-layer shot: the rect is visible in frames 2 and 3 only, and the
/// disc, which lies behind the camera, is visible in frames 0 to 3.
const SIX_FRAME_FRAMES_TEXT: &str = r#""frames":[{"camera":{"roll":{"den":1,"num":0},"x":0,"y":0,"z":0},"index":0,"objects":[{"index":1,"x":-10,"y":0,"z":500},{"index":2,"x":0,"y":0,"z":-50}]},{"camera":{"roll":{"den":1,"num":0},"x":0,"y":0,"z":0},"index":1,"objects":[{"index":1,"x":-10,"y":0,"z":500},{"index":2,"x":0,"y":0,"z":-50}]},{"camera":{"roll":{"den":1,"num":0},"x":0,"y":0,"z":0},"index":2,"objects":[{"index":0,"x":6,"y":-2,"z":1014},{"index":1,"x":-10,"y":0,"z":500},{"index":2,"x":0,"y":0,"z":-50}]},{"camera":{"roll":{"den":1,"num":0},"x":0,"y":0,"z":0},"index":3,"objects":[{"index":0,"x":7,"y":-2,"z":1021},{"index":1,"x":-10,"y":0,"z":500},{"index":2,"x":0,"y":0,"z":-50}]},{"camera":{"roll":{"den":1,"num":0},"x":0,"y":0,"z":0},"index":4,"objects":[{"index":1,"x":-10,"y":0,"z":500}]},{"camera":{"roll":{"den":1,"num":0},"x":0,"y":0,"z":0},"index":5,"objects":[{"index":1,"x":-10,"y":0,"z":500}]}]"#;

/// A six-frame shot with a rect on a linear track, a polygon, and a disc behind the camera
/// writes its layer declarations and every frame's visible layers in full.
#[test]
fn six_frame_shot_lists_only_the_visible_layers() {
    assert_eq!(check_six_frame_shot_lists_only_the_visible_layers(), Ok(()));
}

/// The rect, on a linear track that moves in depth, visible in frames 2 and 3.
fn rect_layer() -> Result<Layer, Fail> {
    Ok(Layer {
        cutout: Cutout::Rect {
            half_width: Extent::new(NonZeroU32::new(5).or_fail()?).or_fail()?,
            half_height: Extent::new(NonZeroU32::new(2).or_fail()?).or_fail()?,
        },
        fill: Rgb8 {
            red: Channel(10),
            green: Channel(20),
            blue: Channel(30),
        },
        track: Track::Linear {
            origin: at(Offsets {
                across: RawCoord(4),
                down: RawCoord(-2),
                depth: RawCoord(1000),
            })?,
            velocity: Velocity {
                across: PerFrame(1),
                down: PerFrame(0),
                depth: PerFrame(7),
            },
        },
        visible: span(FrameIndex::new(2)..FrameIndex::new(4))?,
    })
}

/// The triangle at depth 500, visible in every frame.
fn polygon_layer() -> Result<Layer, Fail> {
    let outline = Outline::new(
        [
            LocalPoint {
                across: coord(RawCoord(0))?,
                down: coord(RawCoord(-3))?,
            },
            LocalPoint {
                across: coord(RawCoord(3))?,
                down: coord(RawCoord(2))?,
            },
            LocalPoint {
                across: coord(RawCoord(-3))?,
                down: coord(RawCoord(2))?,
            },
        ],
        Vec::new(),
    );
    Ok(Layer {
        cutout: Cutout::Polygon(outline),
        fill: Rgb8 {
            red: Channel(40),
            green: Channel(50),
            blue: Channel(60),
        },
        track: Track::Fixed(at(Offsets {
            across: RawCoord(-10),
            down: RawCoord(0),
            depth: RawCoord(500),
        })?),
        visible: span(FrameIndex::new(0)..FrameIndex::new(6))?,
    })
}

/// The disc behind the camera, visible in frames 0 to 3.
fn behind_camera_disc() -> Result<Layer, Fail> {
    Ok(Layer {
        cutout: Cutout::Disc {
            radius: Extent::new(NonZeroU32::new(3).or_fail()?).or_fail()?,
        },
        fill: Rgb8 {
            red: Channel(1),
            green: Channel(2),
            blue: Channel(3),
        },
        track: Track::Fixed(at(Offsets {
            across: RawCoord(0),
            down: RawCoord(0),
            depth: RawCoord(-50),
        })?),
        visible: span(FrameIndex::new(0)..FrameIndex::new(4))?,
    })
}

/// The three-layer shot, of six frames, seen by a stationary camera.
fn six_frame_shot() -> Result<DeepShot, Fail> {
    let stage = Stage {
        backdrop: black(),
        layers: vec![rect_layer()?, polygon_layer()?, behind_camera_disc()?],
    };
    let rig = Rig::new(
        &still_pose()?,
        pinhole(FocalLength(NonZeroU8::new(16).or_fail()?)),
        &recorder(Medium::Off, FrameCount::new(6).or_fail()?)?,
    );
    DeepShot::new(stage, &rig, PixelGrid::One).or_fail()
}

/// Writes the six-frame shot and checks its declarations and its frame arrays.
fn check_six_frame_shot_lists_only_the_visible_layers() -> Result<(), Fail> {
    let text = manifest_text(&six_frame_shot()?)?;
    ensure!(text.0.contains(SIX_FRAME_OBJECTS_TEXT), "objects: {text}");
    ensure!(text.0.contains(SIX_FRAME_FRAMES_TEXT), "frames: {text}");
    Ok(())
}

/// A sensor's medium is written as the shared medium object, in the deep manifest and in the flat
/// manifest alike.
#[test]
fn medium_is_written_as_the_flat_manifest_writes_it() {
    assert_eq!(
        check_medium_is_written_as_the_flat_manifest_writes_it(),
        Ok(())
    );
}

/// Sensor noise with raw terms and seeds of the same values the flat manifest tests use.
fn sensor() -> Result<Medium, Fail> {
    let seeds = SensorSeeds::new(
        TemporalSeed::new(Seed::new(18_446_744_073_709_551_557)),
        PatternSeed::new(Seed::new(7)),
    );
    Ok(Medium::Sensor(
        SensorNoise::new(
            ShotGain(Q8_8(77)),
            ReadSigma(Q8_8(1237)),
            PrnuSpread(655),
            ColumnSigma(Q8_8(384)),
            seeds,
        )
        .or_fail()?,
    ))
}

/// A one-frame flat manifest of the given medium with ideal optics, for comparison.
fn flat_text(medium: Medium) -> Result<ManifestText, Fail> {
    let dimensions = Dimensions::new(Width::new(8).or_fail()?, Height::new(8).or_fail()?);
    let camera = ManifestCamera::new(dimensions, medium, Optics::Off);
    let header = ManifestHeader::new(
        JsonEntryName::new("flat-medium").or_fail()?,
        &camera,
        FrameRates::playback_only(FrameRate::from_fps(30).or_fail()?),
        BackgroundName::Solid,
        None,
    );
    let one = Ratio::from_integer(Numerator(1));
    let zero = Ratio::from_integer(Numerator(0));
    let view = ManifestAffine::from_affine(&Affine {
        xx: one,
        xy: zero,
        tx: zero,
        yx: zero,
        yy: one,
        ty: zero,
    });
    let frame = ManifestFrame::new(&view, Vec::new());
    let manifest = Manifest::new(header, Vec::new(), vec![frame]).or_fail()?;
    Ok(ManifestText(
        manifest.to_canonical_json().as_str().to_owned(),
    ))
}

/// Writes a sensor's medium in both manifests and checks that each contains the shared text.
fn check_medium_is_written_as_the_flat_manifest_writes_it() -> Result<(), Fail> {
    let medium = sensor()?;
    let shared = medium_object(medium).or_fail()?.to_string();
    let member = format!("\"medium\":{shared}");

    let shot = one_frame_shot(pinhole(FocalLength(NonZeroU8::new(16).or_fail()?)), medium)?;
    let deep = manifest_text(&shot)?;
    ensure!(deep.0.contains(&member), "deep manifest: {deep}");

    let flat = flat_text(medium)?;
    ensure!(flat.0.contains(&member), "flat manifest: {flat}");
    Ok(())
}

/// A deep shot with no medium writes no medium key.
#[test]
fn ideal_medium_writes_no_medium_key() {
    assert_eq!(check_ideal_medium_writes_no_medium_key(), Ok(()));
}

/// Writes the one-frame shot on an ideal medium and checks that no medium key appears.
fn check_ideal_medium_writes_no_medium_key() -> Result<(), Fail> {
    let shot = one_frame_shot(
        pinhole(FocalLength(NonZeroU8::new(16).or_fail()?)),
        Medium::Off,
    )?;
    let text = manifest_text(&shot)?;
    ensure!(!text.0.contains("\"medium\""), "medium key: {text}");
    Ok(())
}
