//! The deep shots of the depth singletons: flat layers at several depths, seen by a perspective
//! camera through a lens.
//!
//! A camera whose focal length is one frame height projects a point at offset `X` across and `Y`
//! down from its axis, at depth `z`, to `24 * X / z` and `24 * Y / z` sensels from the frame
//! center, because every depth entry is 24 sensels high. The bokeh shots use a focal length of six
//! frame heights, which projects the same point to `144 * X / z` and `144 * Y / z` sensels. Each
//! constant below is chosen for that 32 by 24 frame, and its comment says what it models.

use crate::scene_builder::{make_ratio, RatioParts, SceneBuildError};
use core::num::{NonZeroU16, NonZeroU32, NonZeroU8};
use synthvid_scene::{
    Channel, Coord, Cutout, Extent, FrameIndex, FrameSpan, Layer, LocalPoint, Numerator, Outline,
    PerFrame, PixelGrid, Position, Ratio, RawCoord, Rgb8, Rotation, Stage, Track, Turns, Velocity,
};
use synthvid_video::{
    Aperture, ApertureGrid, ApertureShape, BladeCount, DeepShot, FStop, FocalLength, Focus,
    FocusDistance, FrameHeight, Iris, Lens, Pose, Recorder, Rig, Vignette,
};

/// Red, for the near layers of the parallax and roll shots.
const RED: Rgb8 = Rgb8 {
    red: Channel(255),
    green: Channel(0),
    blue: Channel(0),
};

/// Green, for the middle layers of the parallax and roll shots.
const GREEN: Rgb8 = Rgb8 {
    red: Channel(0),
    green: Channel(255),
    blue: Channel(0),
};

/// Blue, for the far layers of the parallax and roll shots.
const BLUE: Rgb8 = Rgb8 {
    red: Channel(0),
    green: Channel(0),
    blue: Channel(255),
};

/// White, for the point highlights of the bokeh shots.
const WHITE: Rgb8 = Rgb8 {
    red: Channel(255),
    green: Channel(255),
    blue: Channel(255),
};

/// Mid-gray, the backdrop of the parallax and roll shots, so the colored layers stand out.
const GRAY: Rgb8 = Rgb8 {
    red: Channel(40),
    green: Channel(40),
    blue: Channel(40),
};

/// Near-black, the backdrop of the bokeh shots, so each highlight's blur stands out.
const NEAR_BLACK: Rgb8 = Rgb8 {
    red: Channel(10),
    green: Channel(10),
    blue: Channel(10),
};

/// The focal length of the parallax and roll shots, 16 sixteenths of a frame height: one frame
/// height.
pub(super) const FOCAL_SIXTEENTHS: NonZeroU8 = match NonZeroU8::new(16) {
    Some(value) => value,
    // Unreachable: 16 is a fixed nonzero literal.
    None => NonZeroU8::MIN,
};

/// The focal length of the bokeh shots, 96 sixteenths of a frame height: six frame heights.
pub(super) const BOKEH_FOCAL_SIXTEENTHS: NonZeroU8 = match NonZeroU8::new(96) {
    Some(value) => value,
    // Unreachable: 96 is a fixed nonzero literal.
    None => NonZeroU8::MIN,
};

/// The radius of the far disc of the parallax shot, in world units.
pub(super) const PARALLAX_FAR_RADIUS: NonZeroU32 = match NonZeroU32::new(160) {
    Some(value) => value,
    // Unreachable: 160 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// The half width of the middle rectangle of the parallax shot, in world units.
pub(super) const PARALLAX_MIDDLE_HALF_WIDTH: NonZeroU32 = match NonZeroU32::new(120) {
    Some(value) => value,
    // Unreachable: 120 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// The half height of the middle rectangle of the parallax shot, in world units.
pub(super) const PARALLAX_MIDDLE_HALF_HEIGHT: NonZeroU32 = match NonZeroU32::new(80) {
    Some(value) => value,
    // Unreachable: 80 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// The radius of the near disc of the parallax shot, in world units.
pub(super) const PARALLAX_NEAR_RADIUS: NonZeroU32 = match NonZeroU32::new(60) {
    Some(value) => value,
    // Unreachable: 60 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// The half width of the red rectangle of the roll shot, in world units.
pub(super) const ROLL_HALF_WIDTH: NonZeroU32 = match NonZeroU32::new(60) {
    Some(value) => value,
    // Unreachable: 60 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// The half height of the red rectangle of the roll shot, in world units.
pub(super) const ROLL_HALF_HEIGHT: NonZeroU32 = match NonZeroU32::new(30) {
    Some(value) => value,
    // Unreachable: 30 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// The denominator of the roll shot's turn per frame: one thirty-second of a turn.
pub(super) const ROLL_TURN_DENOMINATOR: NonZeroU16 = match NonZeroU16::new(32) {
    Some(value) => value,
    // Unreachable: 32 is a fixed nonzero literal.
    None => NonZeroU16::MIN,
};

/// The distance the bokeh shots are focused at, in scene units. It is nine focal lengths of the
/// bokeh lens, so it lies beyond the eight focal lengths that a focus must keep.
pub(super) const FOCUS_DISTANCE: NonZeroU32 = match NonZeroU32::new(864) {
    Some(value) => value,
    // Unreachable: 864 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// The frame height of the bokeh shots' lens, in world units. With a focal length of six frame
/// heights the lens has a focal length of 96 world units, and the focus distance of 864 is nine
/// of them.
pub(super) const BOKEH_FRAME_HEIGHT: NonZeroU16 = match NonZeroU16::new(16) {
    Some(value) => value,
    // Unreachable: 16 is a fixed nonzero literal.
    None => NonZeroU16::MIN,
};

/// The number of blades of the hexagonal iris.
pub(super) const HEXAGON_BLADES: NonZeroU8 = match NonZeroU8::new(6) {
    Some(value) => value,
    // Unreachable: 6 is a fixed nonzero literal.
    None => NonZeroU8::MIN,
};

/// The radius of the near highlight, in world units. It projects to 2 sensels at depth 432.
pub(super) const NEAR_HIGHLIGHT_RADIUS: NonZeroU32 = match NonZeroU32::new(6) {
    Some(value) => value,
    // Unreachable: 6 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// The radius of the focused highlight, in world units. It projects to 2 sensels at depth 864.
pub(super) const FOCUSED_HIGHLIGHT_RADIUS: NonZeroU32 = match NonZeroU32::new(12) {
    Some(value) => value,
    // Unreachable: 12 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// The radius of the far highlight, in world units. It projects to 2 sensels at depth 3456.
pub(super) const FAR_HIGHLIGHT_RADIUS: NonZeroU32 = match NonZeroU32::new(48) {
    Some(value) => value,
    // Unreachable: 48 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// A position in world units, named by its three axes so that the coordinates cannot be
/// passed in each other's place.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Spot {
    /// Distance to the right of the camera axis.
    across: RawCoord,
    /// Distance below the camera axis.
    down: RawCoord,
    /// Distance forward along the optical axis.
    depth: RawCoord,
}

/// Checks a world coordinate against the world bound.
fn coord(raw: RawCoord) -> Result<Coord, SceneBuildError> {
    Coord::new(raw).map_err(|error| SceneBuildError::Coord(Box::new(error)))
}

/// The position of a spot in world units.
fn position(spot: Spot) -> Result<Position, SceneBuildError> {
    Ok(Position {
        across: coord(spot.across)?,
        down: coord(spot.down)?,
        depth: coord(spot.depth)?,
    })
}

/// A track that keeps a layer at one spot on every frame.
fn fixed(spot: Spot) -> Result<Track, SceneBuildError> {
    Ok(Track::Fixed(position(spot)?))
}

/// A track that keeps the camera at the origin on every frame.
fn origin() -> Result<Track, SceneBuildError> {
    fixed(Spot {
        across: RawCoord(0),
        down: RawCoord(0),
        depth: RawCoord(0),
    })
}

/// A rotation that holds the camera's roll at zero on every frame.
fn upright() -> Rotation {
    Rotation::Fixed(Turns::new(Ratio::from_integer(Numerator(0))))
}

/// A length in world units, checked against the world bound.
fn extent(length: NonZeroU32) -> Result<Extent, SceneBuildError> {
    Extent::new(length).map_err(|error| SceneBuildError::Extent(Box::new(error)))
}

/// A lens with the given aperture, a focal length of one frame height, and no vignetting.
const fn lens(aperture: Aperture) -> Lens {
    Lens::new(FocalLength(FOCAL_SIXTEENTHS), aperture, Vignette::None)
}

/// A layer of the given shape, color and track, visible on every frame the recorder records.
fn layer(
    recorder: &Recorder,
    cutout: Cutout,
    fill: Rgb8,
    track: Track,
) -> Result<Layer, SceneBuildError> {
    let count = recorder.timing().count().get().get();
    let visible = FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(count))
        .map_err(|error| SceneBuildError::VisibilitySpan(Box::new(error)))?;
    Ok(Layer {
        cutout,
        fill,
        track,
        visible,
    })
}

/// A deep shot of a stage, seen through a rig with the given pose and lens and recorded by the
/// given recorder, sampled on the two-by-two grid of every sensel.
fn shot(
    stage: Stage,
    pose: &Pose,
    lens: Lens,
    recorder: &Recorder,
) -> Result<DeepShot, SceneBuildError> {
    DeepShot::new(stage, &Rig::new(pose, lens, recorder), PixelGrid::Two)
        .map_err(|error| SceneBuildError::DeepShot(Box::new(error)))
}

/// A parallax shot: a red disc near the camera, a green rectangle in the middle distance and a
/// blue disc far away, seen by a pinhole camera that dollies forward and slides to the right.
///
/// The camera moves 20 world units forward and 10 to the right on every frame. Near layers
/// therefore cross the frame faster than far ones, and every layer grows as it nears.
///
/// # Errors
///
/// Returns `SceneBuildError` if a coordinate, extent or span constant is invalid.
pub(super) fn parallax(recorder: &Recorder) -> Result<DeepShot, SceneBuildError> {
    let camera = Track::Linear {
        origin: position(Spot {
            across: RawCoord(0),
            down: RawCoord(0),
            depth: RawCoord(0),
        })?,
        velocity: Velocity {
            across: PerFrame(10),
            down: PerFrame(0),
            depth: PerFrame(20),
        },
    };
    let stage = Stage {
        backdrop: GRAY,
        layers: vec![
            // The far disc has radius 160 at depth 1600, about 2.4 sensels, and sits 200 units to
            // the right, so it shows beside the rectangle.
            layer(
                recorder,
                Cutout::Disc {
                    radius: extent(PARALLAX_FAR_RADIUS)?,
                },
                BLUE,
                fixed(Spot {
                    across: RawCoord(200),
                    down: RawCoord(-40),
                    depth: RawCoord(1600),
                })?,
            )?,
            // The middle rectangle is 240 by 160 units at depth 800, about 7 by 5 sensels, and
            // centered on the axis.
            layer(
                recorder,
                Cutout::Rect {
                    half_width: extent(PARALLAX_MIDDLE_HALF_WIDTH)?,
                    half_height: extent(PARALLAX_MIDDLE_HALF_HEIGHT)?,
                },
                GREEN,
                fixed(Spot {
                    across: RawCoord(0),
                    down: RawCoord(0),
                    depth: RawCoord(800),
                })?,
            )?,
            // The near disc has radius 60 at depth 400, about 3.6 sensels, and sits 40 units left
            // of the axis. It is the nearest layer, so it lies in front of the others.
            layer(
                recorder,
                Cutout::Disc {
                    radius: extent(PARALLAX_NEAR_RADIUS)?,
                },
                RED,
                fixed(Spot {
                    across: RawCoord(-40),
                    down: RawCoord(0),
                    depth: RawCoord(400),
                })?,
            )?,
        ],
    };
    shot(
        stage,
        &Pose::new(camera, upright()),
        lens(Aperture::Pinhole),
        recorder,
    )
}

/// A roll shot: a red rectangle and a green triangle at different depths, seen by a pinhole
/// camera that turns 1/32 of a turn about its axis on every frame.
///
/// The rectangle and the triangle are placed off the axis on opposite sides, so the turn moves
/// each one along its own arc and the frames differ in more than a shift.
///
/// # Errors
///
/// Returns `SceneBuildError` if a coordinate, extent or ratio constant is invalid.
pub(super) fn roll(recorder: &Recorder) -> Result<DeepShot, SceneBuildError> {
    let per_frame = make_ratio(RatioParts {
        numerator: Numerator(1),
        denominator: ROLL_TURN_DENOMINATOR,
    })?;
    let turning = Rotation::Linear {
        start: Turns::new(Ratio::from_integer(Numerator(0))),
        per_frame: Turns::new(per_frame),
    };
    // An asymmetric triangle: its apex is above the center and its base is off to one side, so
    // a rotated copy is easy to tell from an unrotated one.
    let triangle = Outline::new(
        [
            LocalPoint {
                across: coord(RawCoord(0))?,
                down: coord(RawCoord(-120))?,
            },
            LocalPoint {
                across: coord(RawCoord(120))?,
                down: coord(RawCoord(60))?,
            },
            LocalPoint {
                across: coord(RawCoord(-100))?,
                down: coord(RawCoord(60))?,
            },
        ],
        Vec::new(),
    );
    let stage = Stage {
        backdrop: GRAY,
        layers: vec![
            // The rectangle is 120 by 60 units at depth 400, about 7 by 4 sensels, and sits 100
            // units left of the axis.
            layer(
                recorder,
                Cutout::Rect {
                    half_width: extent(ROLL_HALF_WIDTH)?,
                    half_height: extent(ROLL_HALF_HEIGHT)?,
                },
                RED,
                fixed(Spot {
                    across: RawCoord(-100),
                    down: RawCoord(0),
                    depth: RawCoord(400),
                })?,
            )?,
            // The triangle sits at depth 800, twice as far, 150 units right of the axis.
            layer(
                recorder,
                Cutout::Polygon(triangle),
                GREEN,
                fixed(Spot {
                    across: RawCoord(150),
                    down: RawCoord(0),
                    depth: RawCoord(800),
                })?,
            )?,
        ],
    };
    shot(
        stage,
        &Pose::new(origin()?, turning),
        lens(Aperture::Pinhole),
        recorder,
    )
}

/// A white disc of the given radius at the given spot.
///
/// The radius is chosen per highlight, so that each one projects to exactly 2 sensels whatever its
/// depth: at the bokeh shots' focal length, a radius of `depth / 72` world units projects to
/// `144 * (depth / 72) / depth = 2` sensels.
fn highlight(
    recorder: &Recorder,
    radius: NonZeroU32,
    spot: Spot,
) -> Result<Layer, SceneBuildError> {
    layer(
        recorder,
        Cutout::Disc {
            radius: extent(radius)?,
        },
        WHITE,
        fixed(spot)?,
    )
}

/// Three point highlights seen through an F2 iris focused at depth 864, with the given aperture
/// outline.
///
/// The lens has a focal length of six frame heights: 96 sixteenths of a frame height, or 96 world
/// units for a frame height of 16. The focus distance of 864 is nine focal lengths. The iris is
/// that focal length divided by the f-number 2, so the aperture has diameter 48 world units and
/// radius 24 world units.
///
/// A lateral world length `X` at depth `z` projects to `144 * X / z` sensels. A lens point whose
/// unit-aperture coordinate is `g / 8`, with `g` odd and `|g|` at most 7, shifts the image of a
/// point at depth `z` by `-432 * g * (864 - z) / (864 * z)` sensels, which is the lens formula
/// with the frame height 16, the focal length 96, the f-number 2 and the aperture count 8. At the
/// rim of the aperture, `g = 8`, the blur of a highlight has radius `3456 * |1/z - 1/864|` sensels:
/// 4 sensels at depth 432, 3 sensels at depth 3456, and none at the focus distance. Every shift at
/// these three depths is an exact multiple of 1/256 sensel, so no rounding enters the counts below.
///
/// Each highlight projects to exactly 2 sensels. The highlight at depth 864 is in focus, and it
/// is a disc of radius 2 sensels centered on column 16, covering columns 14 to 17. The highlight
/// at depth 432 is centered at -9 sensels, column 7, and its smear covers columns 1 to 12. The
/// highlight at depth 3456 is centered at +9 sensels, column 25, and its smear covers columns 20
/// to 29. Each smear lies inside the frame and clear of its border columns.
///
/// There are 52 aperture points: odd g and k with g^2 + k^2 at most 64. A sensel has 2 by 2
/// samples at offsets of a quarter and three quarters of a sensel, so it has 208 (sample, aperture
/// point) pairs in all. The four sensels that meet at the center of the near smear, columns 6 and
/// 7 of rows 11 and 12, each have 52 pairs whose sample falls inside the highlight and 156 whose
/// sample falls on the backdrop. Their exact average is (52 * 255 + 156 * 10) / 208 = 14820 / 208 =
/// 71.25. The renderer rounds that average half up, as floor((2 * 14820 + 208) / (2 * 208)) =
/// floor(71.75) = 71, which is 61 above the backdrop of 10. The four sensels that meet at the
/// center of the far smear, columns 24 and 25 of rows 11 and 12, each have 94 such pairs and 114
/// on the backdrop. Their exact average is (94 * 255 + 114 * 10) / 208 = 25110 / 208 = 120.72,
/// which rounds half up, as floor((2 * 25110 + 208) / 416) = floor(121.22) = 121, to 111 above the
/// backdrop. The focused highlight is inside its disc for all 208 pairs, so its center is 255.
///
/// # Errors
///
/// Returns `SceneBuildError` if a coordinate, extent, focus, frame height or lens constant is
/// invalid.
fn bokeh(recorder: &Recorder, shape: ApertureShape) -> Result<DeepShot, SceneBuildError> {
    let focus = FocusDistance::new(FOCUS_DISTANCE)
        .map_err(|error| SceneBuildError::FocusDistance(Box::new(error)))?;
    let iris = Iris::new(
        FStop::F2,
        shape,
        Focus::At(focus),
        FrameHeight(BOKEH_FRAME_HEIGHT),
        ApertureGrid::Eight,
    );
    let stage = Stage {
        backdrop: NEAR_BLACK,
        layers: vec![
            // Depth 432, at the left: 27 units left of the axis at depth 432 is 9 sensels left of
            // the center, at column 7.
            highlight(
                recorder,
                NEAR_HIGHLIGHT_RADIUS,
                Spot {
                    across: RawCoord(-27),
                    down: RawCoord(0),
                    depth: RawCoord(432),
                },
            )?,
            // Depth 864, the focus distance, at the center.
            highlight(
                recorder,
                FOCUSED_HIGHLIGHT_RADIUS,
                Spot {
                    across: RawCoord(0),
                    down: RawCoord(0),
                    depth: RawCoord(864),
                },
            )?,
            // Depth 3456, at the right: 216 units right of the axis at depth 3456 is 9 sensels
            // right of the center, at column 25.
            highlight(
                recorder,
                FAR_HIGHLIGHT_RADIUS,
                Spot {
                    across: RawCoord(216),
                    down: RawCoord(0),
                    depth: RawCoord(3456),
                },
            )?,
        ],
    };
    let lens = Lens::new(
        FocalLength(BOKEH_FOCAL_SIXTEENTHS),
        Aperture::Iris(iris),
        Vignette::None,
    );
    shot(stage, &Pose::new(origin()?, upright()), lens, recorder)
}

/// The disc bokeh shot: the highlights through a round iris.
///
/// # Errors
///
/// Returns `SceneBuildError` if the shot's constants are invalid.
pub(super) fn disc_bokeh(recorder: &Recorder) -> Result<DeepShot, SceneBuildError> {
    bokeh(recorder, ApertureShape::Disc)
}

/// The hexagonal bokeh shot: the same highlights through an iris of six blades, so each smear is
/// a hexagon.
///
/// # Errors
///
/// Returns `SceneBuildError` if the shot's constants are invalid.
pub(super) fn hex_bokeh(recorder: &Recorder) -> Result<DeepShot, SceneBuildError> {
    let blades = BladeCount::new(HEXAGON_BLADES)
        .map_err(|error| SceneBuildError::BladeCount(Box::new(error)))?;
    bokeh(recorder, ApertureShape::Polygon(blades))
}
