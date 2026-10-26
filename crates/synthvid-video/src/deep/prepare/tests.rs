use super::*;
use crate::camera::Timing;
use crate::medium::Medium;
use crate::optics::{Aperture, Iris};
use crate::optics::{ApertureGrid, ApertureShape, BladeCount, FStop, Focus, FocusDistance};
use crate::optics::{FrameHeight, Lens, Vignette};
use crate::recorder::Recorder;
use crate::rig::{Pose, Rig};
use core::num::{NonZeroU16, NonZeroU8};
use core::ops::Range;
use synthvid_scene::scene::FrameSpan;
use synthvid_scene::{
    covers, Channel, Containment, Coord, FrameCount, FrameRate, Height, Numerator, PerFrame,
    PixelGrid, Ratio, RawCoord, Rotation, Stage, Track, Turns, Velocity, Width,
};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

mod geometry;

/// A coordinate from its raw world value.
fn coord(raw: RawCoord) -> Result<Coord, Fail> {
    Coord::new(raw).or_fail()
}

/// A position named by its three components, so that the across, down, and depth values cannot be
/// transposed.
#[derive(Copy, Clone, Debug)]
struct Spot {
    /// The component to the right of the origin.
    across: RawCoord,
    /// The component below the origin.
    down: RawCoord,
    /// The component forward along the optical axis.
    depth: RawCoord,
}

/// A spot on the optical axis at the given depth.
fn on_axis(depth: RawCoord) -> Spot {
    Spot {
        across: RawCoord(0),
        down: RawCoord(0),
        depth,
    }
}

/// The world position of a spot.
fn position(spot: Spot) -> Result<Position, Fail> {
    let Spot {
        across,
        down,
        depth,
    } = spot;
    Ok(Position {
        across: coord(across)?,
        down: coord(down)?,
        depth: coord(depth)?,
    })
}

/// A track that stays at the given spot on every frame.
fn fixed(spot: Spot) -> Result<Track, Fail> {
    Ok(Track::Fixed(position(spot)?))
}

/// A track that starts at the given spot and moves one world unit to the right per frame.
fn walking_across(spot: Spot) -> Result<Track, Fail> {
    Ok(Track::Linear {
        origin: position(spot)?,
        velocity: Velocity {
            across: PerFrame(1),
            down: PerFrame(0),
            depth: PerFrame(0),
        },
    })
}

/// A span of frames, checked by the span's own constructor.
fn span(frames: Range<FrameIndex>) -> Result<FrameSpan, Fail> {
    FrameSpan::new(frames).or_fail()
}

/// An extent from a non-zero length.
fn extent(length: NonZeroU32) -> Result<Extent, Fail> {
    Extent::new(length).or_fail()
}

/// A rectangle with half-width and half-height of one world unit.
fn minimal_rect() -> Result<Cutout, Fail> {
    Ok(Cutout::Rect {
        half_width: extent(NonZeroU32::MIN)?,
        half_height: extent(NonZeroU32::MIN)?,
    })
}

/// A rectangle two world units wide and one world unit tall, in half-sizes.
fn wide_rect() -> Result<Cutout, Fail> {
    Ok(Cutout::Rect {
        half_width: extent(NonZeroU32::new(2).or_fail()?)?,
        half_height: extent(NonZeroU32::MIN)?,
    })
}

/// A layer that is visible on frames zero to nine.
fn layer(cutout: Cutout, fill: Rgb8, track: Track) -> Result<Layer, Fail> {
    Ok(Layer {
        cutout,
        fill,
        track,
        visible: span(FrameIndex::new(0)..FrameIndex::new(10))?,
    })
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

/// Pure red.
fn red() -> Rgb8 {
    Rgb8 {
        red: Channel(255),
        green: Channel(0),
        blue: Channel(0),
    }
}

/// Pure blue.
fn blue() -> Rgb8 {
    Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(255),
    }
}

/// A roll of zero turns, which leaves the camera upright on every frame.
fn upright() -> Rotation {
    Rotation::Fixed(Turns::new(Ratio::from_integer(Numerator(0))))
}

/// A roll of a quarter turn, which turns the camera's image by a right angle.
fn quarter_turn() -> Result<Rotation, Fail> {
    let quarter = Ratio::new(Numerator(1), NonZeroI64::new(4).or_fail()?).or_fail()?;
    Ok(Rotation::Fixed(Turns::new(quarter)))
}

/// Eight sensels wide and eight tall.
fn eight_by_eight() -> Result<Dimensions, Fail> {
    Ok(Dimensions::new(
        Width::new(8).or_fail()?,
        Height::new(8).or_fail()?,
    ))
}

/// Sixty-four sensels wide and forty-eight tall.
fn wide_short() -> Result<Dimensions, Fail> {
    Ok(Dimensions::new(
        Width::new(64).or_fail()?,
        Height::new(48).or_fail()?,
    ))
}

/// A recorder with no medium, the given dimensions, and one picture at 30 frames per second.
fn recorder(dimensions: Dimensions) -> Result<Recorder, Fail> {
    let timing = Timing::new(
        FrameRate::from_fraction(30, NonZeroU32::MIN).or_fail()?,
        FrameCount::new(1).or_fail()?,
    );
    Ok(Recorder::new(Medium::Off, dimensions, timing))
}

/// A focal length of the given count of sixteenths of a frame height.
fn focal(sixteenths: NonZeroU8) -> FocalLength {
    FocalLength(sixteenths)
}

/// A lens with a focal length of 16 sixteenths and the given aperture.
fn lens(aperture: Aperture) -> Result<Lens, Fail> {
    Ok(Lens::new(
        focal(NonZeroU8::new(16).or_fail()?),
        aperture,
        Vignette::None,
    ))
}

/// An iris with an f/2 disc outline, a frame height of eight world units, and a two by two grid.
fn f2_disc_iris(focus: Focus) -> Result<Aperture, Fail> {
    Ok(Aperture::Iris(Iris::new(
        FStop::F2,
        ApertureShape::Disc,
        focus,
        FrameHeight(NonZeroU16::new(8).or_fail()?),
        ApertureGrid::Two,
    )))
}

/// An iris with a three-blade outline, a frame height of eight world units, and a two by two grid.
fn triangle_iris() -> Result<Aperture, Fail> {
    let blades = BladeCount::new(NonZeroU8::new(3).or_fail()?).or_fail()?;
    Ok(Aperture::Iris(Iris::new(
        FStop::F2,
        ApertureShape::Polygon(blades),
        Focus::Infinity,
        FrameHeight(NonZeroU16::new(8).or_fail()?),
        ApertureGrid::Two,
    )))
}

/// A focus on the plane 64 scene units from the camera. The iris has a frame height of eight scene
/// units and the lens a focal length of 16 sixteenths, so the focal length is 8 scene units and the
/// focus is eight focal lengths away, the closest the rig accepts.
fn focus_at_sixty_four() -> Result<Focus, Fail> {
    let distance = FocusDistance::new(NonZeroU32::new(64).or_fail()?).or_fail()?;
    Ok(Focus::At(distance))
}

/// A deep shot with a black backdrop, a pose built from the camera track and roll, and the
/// given lens, recorder, and layers.
fn shot(
    camera: Track,
    roll: Rotation,
    lens: Lens,
    recorder: &Recorder,
    layers: Vec<Layer>,
) -> Result<DeepShot, Fail> {
    let rig = Rig::new(&Pose::new(camera, roll), lens, recorder);
    DeepShot::new(
        Stage {
            backdrop: black(),
            layers,
        },
        &rig,
        PixelGrid::One,
    )
    .or_fail()
}

/// The first layer of a prepared frame, which must exist.
fn first_layer<'frame>(prepared: &'frame PreparedFrame<'_>) -> Result<&'frame PreparedLayer, Fail> {
    prepared.layers().first().or_fail()
}

/// The fill of every prepared layer, from front to back.
fn fills(prepared: &PreparedFrame<'_>) -> Vec<Rgb8> {
    prepared.layers().iter().map(PreparedLayer::fill).collect()
}

/// The shift of the nearest visible layer through each point of the lens, in the order of the
/// points. Every row has a shift for each visible layer, so the nearest layer's shift is the first
/// entry of each row, which must exist.
fn nearest_shifts(prepared: &PreparedFrame<'_>) -> Result<Vec<LatticeShift>, Fail> {
    prepared
        .lens_rows()
        .iter()
        .map(|row| row.first().copied().or_fail())
        .collect()
}

/// The lattice bounds of a minimal rectangle at a relative depth of eight with an eight by eight
/// recorder: the corners sit 256 lattice units from the frame center at 1024.
fn minimal_rect_bounds() -> LatticeBounds {
    LatticeBounds {
        low: LatticePoint {
            across: SubSensel(768),
            down: SubSensel(768),
        },
        high: LatticePoint {
            across: SubSensel(1280),
            down: SubSensel(1280),
        },
    }
}

/// The shift of a pinhole, or of a layer exactly on the plane of focus.
fn zero_shift() -> LatticeShift {
    LatticeShift {
        across: SubSensel(0),
        down: SubSensel(0),
    }
}

#[test]
fn minimal_rect_projects_to_the_expected_bounds_through_a_pinhole() {
    assert_eq!(
        check_minimal_rect_projects_to_the_expected_bounds_through_a_pinhole(),
        Ok(())
    );
}

fn check_minimal_rect_projects_to_the_expected_bounds_through_a_pinhole() -> Result<(), Fail> {
    let camera = shot(
        fixed(on_axis(RawCoord(0)))?,
        upright(),
        lens(Aperture::Pinhole)?,
        &recorder(eight_by_eight()?)?,
        vec![layer(
            minimal_rect()?,
            white(),
            fixed(on_axis(RawCoord(8)))?,
        )?],
    )?;
    let prepared = prepare_frame(&camera, FrameIndex::new(0)).or_fail()?;
    let found = first_layer(&prepared)?;
    ensure_eq!(found.bounds(), minimal_rect_bounds());
    ensure_eq!(nearest_shifts(&prepared)?, vec![zero_shift()]);
    ensure_eq!(prepared.lens_points(), LensPointCount::new(NonZeroU32::MIN));
    Ok(())
}

#[test]
fn minimal_rect_through_an_iris_gives_one_shift_per_aperture_point() {
    assert_eq!(
        check_minimal_rect_through_an_iris_gives_one_shift_per_aperture_point(),
        Ok(())
    );
}

fn check_minimal_rect_through_an_iris_gives_one_shift_per_aperture_point() -> Result<(), Fail> {
    let camera = shot(
        fixed(on_axis(RawCoord(0)))?,
        upright(),
        lens(f2_disc_iris(Focus::Infinity)?)?,
        &recorder(eight_by_eight()?)?,
        vec![layer(
            minimal_rect()?,
            white(),
            fixed(on_axis(RawCoord(8)))?,
        )?],
    )?;
    let prepared = prepare_frame(&camera, FrameIndex::new(0)).or_fail()?;
    ensure_eq!(
        prepared.lens_points(),
        LensPointCount::new(NonZeroU32::new(4).or_fail()?)
    );
    ensure_eq!(
        nearest_shifts(&prepared)?,
        vec![
            LatticeShift {
                across: SubSensel(256),
                down: SubSensel(256),
            },
            LatticeShift {
                across: SubSensel(-256),
                down: SubSensel(256),
            },
            LatticeShift {
                across: SubSensel(256),
                down: SubSensel(-256),
            },
            LatticeShift {
                across: SubSensel(-256),
                down: SubSensel(-256),
            },
        ]
    );
    Ok(())
}

#[test]
fn layer_on_the_plane_of_focus_has_zero_iris_shifts() {
    assert_eq!(
        check_layer_on_the_plane_of_focus_has_zero_iris_shifts(),
        Ok(())
    );
}

fn check_layer_on_the_plane_of_focus_has_zero_iris_shifts() -> Result<(), Fail> {
    let camera = shot(
        fixed(on_axis(RawCoord(0)))?,
        upright(),
        lens(f2_disc_iris(focus_at_sixty_four()?)?)?,
        &recorder(eight_by_eight()?)?,
        vec![layer(
            minimal_rect()?,
            white(),
            fixed(on_axis(RawCoord(64)))?,
        )?],
    )?;
    let prepared = prepare_frame(&camera, FrameIndex::new(0)).or_fail()?;
    ensure_eq!(
        prepared.lens_points(),
        LensPointCount::new(NonZeroU32::new(4).or_fail()?)
    );
    ensure_eq!(nearest_shifts(&prepared)?, vec![zero_shift(); 4]);
    Ok(())
}

#[test]
fn nearer_layer_comes_first() {
    assert_eq!(check_nearer_layer_comes_first(), Ok(()));
}

fn check_nearer_layer_comes_first() -> Result<(), Fail> {
    let camera = shot(
        fixed(on_axis(RawCoord(0)))?,
        upright(),
        lens(Aperture::Pinhole)?,
        &recorder(eight_by_eight()?)?,
        vec![
            layer(minimal_rect()?, blue(), fixed(on_axis(RawCoord(16)))?)?,
            layer(minimal_rect()?, red(), fixed(on_axis(RawCoord(8)))?)?,
        ],
    )?;
    let prepared = prepare_frame(&camera, FrameIndex::new(0)).or_fail()?;
    ensure_eq!(fills(&prepared), vec![red(), blue()]);
    Ok(())
}

#[test]
fn later_declared_layer_comes_first_at_equal_depth() {
    assert_eq!(
        check_later_declared_layer_comes_first_at_equal_depth(),
        Ok(())
    );
}

fn check_later_declared_layer_comes_first_at_equal_depth() -> Result<(), Fail> {
    let camera = shot(
        fixed(on_axis(RawCoord(0)))?,
        upright(),
        lens(Aperture::Pinhole)?,
        &recorder(eight_by_eight()?)?,
        vec![
            layer(minimal_rect()?, red(), fixed(on_axis(RawCoord(8)))?)?,
            layer(minimal_rect()?, blue(), fixed(on_axis(RawCoord(8)))?)?,
        ],
    )?;
    let prepared = prepare_frame(&camera, FrameIndex::new(0)).or_fail()?;
    ensure_eq!(fills(&prepared), vec![blue(), red()]);
    Ok(())
}

#[test]
fn layers_at_or_behind_the_camera_and_hidden_layers_are_absent() {
    assert_eq!(
        check_layers_at_or_behind_the_camera_and_hidden_layers_are_absent(),
        Ok(())
    );
}

fn check_layers_at_or_behind_the_camera_and_hidden_layers_are_absent() -> Result<(), Fail> {
    let at_origin = shot(
        fixed(on_axis(RawCoord(0)))?,
        upright(),
        lens(Aperture::Pinhole)?,
        &recorder(eight_by_eight()?)?,
        vec![layer(
            minimal_rect()?,
            white(),
            fixed(on_axis(RawCoord(0)))?,
        )?],
    )?;
    ensure!(prepare_frame(&at_origin, FrameIndex::new(0))
        .or_fail()?
        .layers()
        .is_empty());

    let behind = shot(
        fixed(on_axis(RawCoord(10)))?,
        upright(),
        lens(Aperture::Pinhole)?,
        &recorder(eight_by_eight()?)?,
        vec![layer(
            minimal_rect()?,
            white(),
            fixed(on_axis(RawCoord(8)))?,
        )?],
    )?;
    ensure!(prepare_frame(&behind, FrameIndex::new(0))
        .or_fail()?
        .layers()
        .is_empty());

    let hidden = shot(
        fixed(on_axis(RawCoord(0)))?,
        upright(),
        lens(Aperture::Pinhole)?,
        &recorder(eight_by_eight()?)?,
        vec![Layer {
            cutout: minimal_rect()?,
            fill: white(),
            track: fixed(on_axis(RawCoord(8)))?,
            visible: span(FrameIndex::new(1)..FrameIndex::new(5))?,
        }],
    )?;
    ensure!(prepare_frame(&hidden, FrameIndex::new(0))
        .or_fail()?
        .layers()
        .is_empty());
    Ok(())
}

#[test]
fn depth_is_measured_from_the_camera() {
    assert_eq!(check_depth_is_measured_from_the_camera(), Ok(()));
}

fn check_depth_is_measured_from_the_camera() -> Result<(), Fail> {
    let camera = shot(
        fixed(on_axis(RawCoord(10)))?,
        upright(),
        lens(Aperture::Pinhole)?,
        &recorder(eight_by_eight()?)?,
        vec![layer(
            minimal_rect()?,
            white(),
            fixed(on_axis(RawCoord(18)))?,
        )?],
    )?;
    let prepared = prepare_frame(&camera, FrameIndex::new(0)).or_fail()?;
    ensure_eq!(first_layer(&prepared)?.bounds(), minimal_rect_bounds());
    Ok(())
}

#[test]
fn moving_camera_and_layer_together_leaves_the_bounds_unchanged() {
    assert_eq!(
        check_moving_camera_and_layer_together_leaves_the_bounds_unchanged(),
        Ok(())
    );
}

fn check_moving_camera_and_layer_together_leaves_the_bounds_unchanged() -> Result<(), Fail> {
    let camera = shot(
        fixed(Spot {
            across: RawCoord(1),
            down: RawCoord(0),
            depth: RawCoord(0),
        })?,
        upright(),
        lens(Aperture::Pinhole)?,
        &recorder(eight_by_eight()?)?,
        vec![layer(
            minimal_rect()?,
            white(),
            fixed(Spot {
                across: RawCoord(1),
                down: RawCoord(0),
                depth: RawCoord(8),
            })?,
        )?],
    )?;
    let prepared = prepare_frame(&camera, FrameIndex::new(0)).or_fail()?;
    ensure_eq!(first_layer(&prepared)?.bounds(), minimal_rect_bounds());
    Ok(())
}
