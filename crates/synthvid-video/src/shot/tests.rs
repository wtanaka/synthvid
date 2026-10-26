use super::*;
use crate::camera::Timing;
use crate::medium::Medium;
use crate::optics::{Aperture, FocalLength, Lens, Optics, Vignette};
use crate::rig::{Pose, Rig};
use core::num::{NonZeroU32, NonZeroU8};
use synthvid_scene::{
    Background, Channel, Coord, Dimensions, FrameCount, FrameRate, Height, Magnification, Motion,
    Numerator, PixelGrid, Point, Position, Ratio, RawCoord, Rgb8, Rotation, Stage, Track, Turns,
    View, Width, Zoom,
};
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

/// A recorder with 8 by 8 sensels, no medium, and one picture at 30 frames per second.
fn recorder() -> Result<Recorder, Fail> {
    let dimensions = Dimensions::new(Width::new(8).or_fail()?, Height::new(8).or_fail()?);
    let timing = Timing::new(
        FrameRate::from_fraction(30, NonZeroU32::MIN).or_fail()?,
        FrameCount::new(1).or_fail()?,
    );
    Ok(Recorder::new(Medium::Off, dimensions, timing))
}

/// A camera at the origin with no roll and a magnification of one, recording with ideal optics.
fn camera() -> Result<Camera, Fail> {
    let origin = Point {
        across: Ratio::from_integer(Numerator(0)),
        down: Ratio::from_integer(Numerator(0)),
    };
    let view = View::new(
        Motion::Fixed(origin),
        Rotation::Fixed(Turns::new(Ratio::from_integer(Numerator(0)))),
        Zoom::Fixed(Magnification::new(Ratio::from_integer(Numerator(1))).or_fail()?),
    );
    Ok(Camera::new(view, Optics::Off, &recorder()?))
}

/// A scene with a solid gray background and no objects.
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

/// A deep shot whose rig stays at the origin, with a pinhole lens and no backdrop layers.
fn deep_shot() -> Result<DeepShot, Fail> {
    let zero = Coord::new(RawCoord(0)).or_fail()?;
    let position = Position {
        across: zero,
        down: zero,
        depth: zero,
    };
    let pose = Pose::new(
        Track::Fixed(position),
        Rotation::Fixed(Turns::new(Ratio::from_integer(Numerator(0)))),
    );
    let lens = Lens::new(
        FocalLength(NonZeroU8::new(16).or_fail()?),
        Aperture::Pinhole,
        Vignette::None,
    );
    let rig = Rig::new(&pose, lens, &recorder()?);
    let stage = Stage {
        backdrop: Rgb8 {
            red: Channel(0),
            green: Channel(0),
            blue: Channel(0),
        },
        layers: Vec::new(),
    };
    DeepShot::new(stage, &rig, PixelGrid::One).or_fail()
}

/// The flat shot returns the scene and camera it was built with.
fn check_flat_shot_holds_its_parts() -> Result<(), Fail> {
    let scene = scene();
    let camera = camera()?;
    let flat = FlatShot::new(scene.clone(), &camera);
    ensure_eq!(flat.scene(), &scene);
    ensure_eq!(flat.camera(), &camera);
    Ok(())
}

/// A flat shot returns the scene and the camera it was built with.
#[test]
fn flat_shot_holds_its_parts() {
    assert_eq!(check_flat_shot_holds_its_parts(), Ok(()));
}

/// Each shot records through the recorder of its source.
fn check_shot_recorder_comes_from_its_source() -> Result<(), Fail> {
    let camera = camera()?;
    let flat = Shot::Flat(Box::new(FlatShot::new(scene(), &camera)));
    ensure_eq!(flat.recorder(), camera.recorder());
    let deep = deep_shot()?;
    let expected = deep.rig().recorder();
    ensure_eq!(Shot::Deep(Box::new(deep)).recorder(), expected);
    Ok(())
}

/// A flat shot records through its camera's recorder, and a deep shot through its rig's.
#[test]
fn shot_recorder_comes_from_its_source() {
    assert_eq!(check_shot_recorder_comes_from_its_source(), Ok(()));
}
