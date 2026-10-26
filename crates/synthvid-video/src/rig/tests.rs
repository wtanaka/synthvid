use super::*;
use crate::camera::Timing;
use crate::medium::Medium;
use crate::optics::{
    ApertureGrid, ApertureShape, BladeCount, FStop, FocusDistance, Iris, Vignette,
};
use core::num::{NonZeroU16, NonZeroU32, NonZeroU8};
use synthvid_scene::{
    Channel, Coord, Dimensions, FrameCount, FrameRate, Height, Numerator, Position, Ratio,
    RawCoord, Rgb8, Turns, Width,
};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// The origin of the scene, with every component zero.
fn origin() -> Result<Position, Fail> {
    let zero = Coord::new(RawCoord(0)).or_fail()?;
    Ok(Position {
        across: zero,
        down: zero,
        depth: zero,
    })
}

/// A camera that stays at the origin.
fn fixed_track() -> Result<Track, Fail> {
    Ok(Track::Fixed(origin()?))
}

/// A roll of zero turns, which leaves the camera upright on every frame.
fn upright() -> Rotation {
    Rotation::Fixed(Turns::new(Ratio::from_integer(Numerator(0))))
}

/// A pose at the origin with no roll.
fn upright_pose() -> Result<Pose, Fail> {
    Ok(Pose::new(fixed_track()?, upright()))
}

/// A recorder with 8 by 8 sensels, no medium, and one picture at 30 frames per second.
fn recorder() -> Result<Recorder, Fail> {
    let dimensions = Dimensions::new(Width::new(8).or_fail()?, Height::new(8).or_fail()?);
    let timing = Timing::new(
        FrameRate::from_fraction(30, NonZeroU32::MIN).or_fail()?,
        FrameCount::new(1).or_fail()?,
    );
    Ok(Recorder::new(Medium::Off, dimensions, timing))
}

/// A pinhole lens with a focal length of 16 sixteenths of a frame height.
fn pinhole_lens() -> Result<Lens, Fail> {
    Ok(Lens::new(
        FocalLength(NonZeroU8::new(16).or_fail()?),
        Aperture::Pinhole,
        Vignette::None,
    ))
}

/// A rig with the upright pose, a pinhole lens, and the 8 by 8 recorder.
fn rig() -> Result<Rig, Fail> {
    Ok(Rig::new(&upright_pose()?, pinhole_lens()?, &recorder()?))
}

/// A stage with a white backdrop and no layers.
fn empty_stage() -> Stage {
    Stage {
        backdrop: Rgb8 {
            red: Channel(255),
            green: Channel(255),
            blue: Channel(255),
        },
        layers: Vec::new(),
    }
}

#[test]
fn pose_keeps_its_track() {
    assert_eq!(check_pose_keeps_its_track(), Ok(()));
}

fn check_pose_keeps_its_track() -> Result<(), Fail> {
    let track = fixed_track()?;
    let pose = Pose::new(track, upright());
    ensure_eq!(pose.track(), track);
    Ok(())
}

#[test]
fn pose_keeps_its_roll() {
    assert_eq!(check_pose_keeps_its_roll(), Ok(()));
}

fn check_pose_keeps_its_roll() -> Result<(), Fail> {
    let pose = upright_pose()?;
    ensure_eq!(pose.roll(), upright());
    Ok(())
}

#[test]
fn rig_keeps_its_pose_lens_and_recorder() {
    assert_eq!(check_rig_keeps_its_pose_lens_and_recorder(), Ok(()));
}

fn check_rig_keeps_its_pose_lens_and_recorder() -> Result<(), Fail> {
    let pose = upright_pose()?;
    let lens = pinhole_lens()?;
    let recorder = recorder()?;
    let rig = Rig::new(&pose, lens, &recorder);
    ensure_eq!(rig.pose(), pose);
    ensure_eq!(rig.lens(), lens);
    ensure_eq!(rig.recorder(), recorder);
    Ok(())
}

#[test]
fn shot_keeps_its_stage_rig_and_pixel_grid() {
    assert_eq!(check_shot_keeps_its_stage_rig_and_pixel_grid(), Ok(()));
}

fn check_shot_keeps_its_stage_rig_and_pixel_grid() -> Result<(), Fail> {
    let stage = empty_stage();
    let camera = rig()?;
    let shot = DeepShot::new(stage.clone(), &camera, PixelGrid::Four).or_fail()?;
    ensure_eq!(shot.stage(), &stage);
    ensure_eq!(shot.rig(), camera);
    ensure_eq!(shot.pixels(), PixelGrid::Four);
    Ok(())
}

#[test]
fn rig_dimensions_are_the_recorder_dimensions() {
    assert_eq!(check_rig_dimensions_are_the_recorder_dimensions(), Ok(()));
}

fn check_rig_dimensions_are_the_recorder_dimensions() -> Result<(), Fail> {
    let camera = rig()?;
    let expected = Dimensions::new(
        Width(NonZeroU16::new(8).or_fail()?),
        Height(NonZeroU16::new(8).or_fail()?),
    );
    ensure_eq!(camera.recorder().dimensions(), expected);
    Ok(())
}

/// A lens with an f/2 disc or polygon iris of the given focus and frame height, the given focal
/// length in sixteenths, and the two by two sampling grid.
fn iris_lens(
    focal: NonZeroU8,
    shape: ApertureShape,
    focus: Focus,
    frame_height: NonZeroU16,
) -> Lens {
    let iris = Iris::new(
        FStop::F2,
        shape,
        focus,
        FrameHeight(frame_height),
        ApertureGrid::Two,
    );
    Lens::new(FocalLength(focal), Aperture::Iris(iris), Vignette::None)
}

/// A focus at the given number of scene units.
fn focus_at(units: NonZeroU32) -> Result<Focus, Fail> {
    Ok(Focus::At(FocusDistance::new(units).or_fail()?))
}

/// A rig with the upright pose, the given lens, and the 8 by 8 recorder.
fn rig_with(lens: Lens) -> Result<Rig, Fail> {
    Ok(Rig::new(&upright_pose()?, lens, &recorder()?))
}

#[test]
fn focus_closer_than_eight_focal_lengths_is_refused_exactly() {
    assert_eq!(
        check_focus_closer_than_eight_focal_lengths_is_refused_exactly(),
        Ok(())
    );
}

fn check_focus_closer_than_eight_focal_lengths_is_refused_exactly() -> Result<(), Fail> {
    // The focal length is fn * H / 16 = 5 * 1080 / 16 = 337.5 scene units, so eight focal lengths
    // is 2700 scene units, and the rule is 2 * s >= fn * H = 5400. A focus at 2699 gives
    // 2 * 2699 = 5398 < 5400 and is refused. A focus at 2700 gives 5400 and is accepted.
    let focal = NonZeroU8::new(5).or_fail()?;
    let frame_height = NonZeroU16::new(1080).or_fail()?;
    let too_close = iris_lens(
        focal,
        ApertureShape::Disc,
        focus_at(NonZeroU32::new(2699).or_fail()?)?,
        frame_height,
    );
    ensure_eq!(
        DeepShot::new(empty_stage(), &rig_with(too_close)?, PixelGrid::One),
        Err(DeepShotError::FocusTooClose)
    );
    let exactly_eight = iris_lens(
        focal,
        ApertureShape::Disc,
        focus_at(NonZeroU32::new(2700).or_fail()?)?,
        frame_height,
    );
    ensure!(
        DeepShot::new(empty_stage(), &rig_with(exactly_eight)?, PixelGrid::One).is_ok(),
        "a focus of eight focal lengths is accepted"
    );
    Ok(())
}

#[test]
fn focus_rule_at_an_odd_product_is_exact() {
    assert_eq!(check_focus_rule_at_an_odd_product_is_exact(), Ok(()));
}

fn check_focus_rule_at_an_odd_product_is_exact() -> Result<(), Fail> {
    // The focal length is fn * H / 16 = 5 * 1081 / 16 scene units, and the rule is
    // 2 * s >= fn * H = 5 * 1081 = 5405. A focus at 2702 gives 2 * 2702 = 5404 < 5405 and is
    // refused. A focus at 2703 gives 5406 >= 5405 and is accepted.
    let focal = NonZeroU8::new(5).or_fail()?;
    let frame_height = NonZeroU16::new(1081).or_fail()?;
    let too_close = iris_lens(
        focal,
        ApertureShape::Disc,
        focus_at(NonZeroU32::new(2702).or_fail()?)?,
        frame_height,
    );
    ensure_eq!(
        DeepShot::new(empty_stage(), &rig_with(too_close)?, PixelGrid::One),
        Err(DeepShotError::FocusTooClose)
    );
    let accepted = iris_lens(
        focal,
        ApertureShape::Disc,
        focus_at(NonZeroU32::new(2703).or_fail()?)?,
        frame_height,
    );
    ensure!(
        DeepShot::new(empty_stage(), &rig_with(accepted)?, PixelGrid::One).is_ok(),
        "a focus with 2 * s just above fn * H is accepted"
    );
    Ok(())
}

#[test]
fn infinite_focus_is_accepted_at_any_focal_length() {
    assert_eq!(
        check_infinite_focus_is_accepted_at_any_focal_length(),
        Ok(())
    );
}

fn check_infinite_focus_is_accepted_at_any_focal_length() -> Result<(), Fail> {
    let lens = iris_lens(
        NonZeroU8::MAX,
        ApertureShape::Disc,
        Focus::Infinity,
        NonZeroU16::MAX,
    );
    ensure!(DeepShot::new(empty_stage(), &rig_with(lens)?, PixelGrid::One).is_ok());
    Ok(())
}

#[test]
fn aperture_refusal_is_wrapped_by_the_shot_error() {
    assert_eq!(
        check_aperture_refusal_is_wrapped_by_the_shot_error(),
        Ok(())
    );
}

fn check_aperture_refusal_is_wrapped_by_the_shot_error() -> Result<(), Fail> {
    // Three blades on the two by two grid keep no symmetric pair of samples.
    let blades = BladeCount::new(NonZeroU8::new(3).or_fail()?).or_fail()?;
    let lens = iris_lens(
        NonZeroU8::new(16).or_fail()?,
        ApertureShape::Polygon(blades),
        Focus::Infinity,
        NonZeroU16::new(8).or_fail()?,
    );
    ensure_eq!(
        DeepShot::new(empty_stage(), &rig_with(lens)?, PixelGrid::One),
        Err(DeepShotError::Aperture(Box::new(
            ApertureError::NoSymmetricPoints
        )))
    );
    Ok(())
}
