use super::*;
use crate::camera::Timing;
use crate::medium::Medium;
use crate::optics::{
    Aperture, ApertureGrid, ApertureShape, FStop, FocalLength, Focus, FocusDistance, FrameHeight,
    Iris, Lens, Vignette,
};
use crate::recorder::Recorder;
use crate::rig::{Pose, Rig};
use core::num::{NonZeroU16, NonZeroU32, NonZeroU8};
use core::ops::Range;
use synthvid_scene::{
    Coord, Cutout, Dimensions, Extent, FrameCount, FrameRate, FrameSpan, Height, Layer, Numerator,
    PerFrame, Position, Ratio, RawCoord, Rotation, Stage, Track, Turns, Velocity, Width,
};
use synthvid_testkit::{ensure_eq, ensure_ne, Fail, OrFail as _};

mod hand_derived;
mod layering;
mod rounding;

/// The number of sensels along each side of the recorder in these tests.
const SIDE: u16 = 8;

/// A sensel, named by its column and row so that the two cannot be transposed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Cell {
    /// The column of the sensel.
    column: PixelColumn,
    /// The row of the sensel.
    row: PixelRow,
}

/// An inclusive run of positions along one axis.
#[derive(Copy, Clone, Debug)]
struct Span<T> {
    /// The first position of the run.
    first: T,
    /// The last position of the run.
    last: T,
}

/// An inclusive block of sensels, given by its columns and its rows.
#[derive(Copy, Clone, Debug)]
struct Block {
    /// The columns that the block holds.
    columns: Span<PixelColumn>,
    /// The rows that the block holds.
    rows: Span<PixelRow>,
}

/// A position in world units, named by its three components so that they cannot be transposed.
#[derive(Copy, Clone, Debug)]
struct Spot {
    /// The component to the right of the optical axis.
    across: RawCoord,
    /// The component below the optical axis.
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
        across: Coord::new(across).or_fail()?,
        down: Coord::new(down).or_fail()?,
        depth: Coord::new(depth).or_fail()?,
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

/// A roll of zero turns, which leaves the camera upright on every frame.
fn upright() -> Rotation {
    Rotation::Fixed(Turns::new(Ratio::from_integer(Numerator(0))))
}

/// An extent from a non-zero length.
fn extent(length: NonZeroU32) -> Result<Extent, Fail> {
    Extent::new(length).or_fail()
}

/// A square of the given half size, in world units.
fn square(half: NonZeroU32) -> Result<Cutout, Fail> {
    Ok(Cutout::Rect {
        half_width: extent(half)?,
        half_height: extent(half)?,
    })
}

/// A layer that is visible on frames zero to nine.
fn layer(cutout: Cutout, fill: Rgb8, track: Track) -> Result<Layer, Fail> {
    let frames: Range<FrameIndex> = FrameIndex::new(0)..FrameIndex::new(10);
    Ok(Layer {
        cutout,
        fill,
        track,
        visible: FrameSpan::new(frames).or_fail()?,
    })
}

/// Pure white.
fn white() -> Rgb8 {
    gray(Channel(255))
}

/// Pure black.
fn black() -> Rgb8 {
    gray(Channel(0))
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

/// A color with all three channels at the given level.
fn gray(level: Channel) -> Rgb8 {
    Rgb8 {
        red: level,
        green: level,
        blue: level,
    }
}

/// The backdrop used by the tests that show it: an awkward color with three distinct channels.
fn backdrop_color() -> Rgb8 {
    Rgb8 {
        red: Channel(10),
        green: Channel(20),
        blue: Channel(30),
    }
}

/// Eight sensels wide and eight tall.
fn eight_by_eight() -> Result<Dimensions, Fail> {
    Ok(Dimensions::new(
        Width::new(SIDE).or_fail()?,
        Height::new(SIDE).or_fail()?,
    ))
}

/// A recorder with no medium, eight by eight sensels, and one picture at 30 frames per second.
fn recorder() -> Result<Recorder, Fail> {
    let timing = Timing::new(
        FrameRate::from_fraction(30, NonZeroU32::MIN).or_fail()?,
        FrameCount::new(1).or_fail()?,
    );
    Ok(Recorder::new(Medium::Off, eight_by_eight()?, timing))
}

/// A lens with a focal length of one frame height, and the given aperture.
fn lens(aperture: Aperture) -> Result<Lens, Fail> {
    Ok(Lens::new(
        FocalLength(NonZeroU8::new(16).or_fail()?),
        aperture,
        Vignette::None,
    ))
}

/// An iris with an f/2 disc outline, a frame height of eight world units, and a two by two grid.
fn f2_iris(focus: Focus) -> Result<Aperture, Fail> {
    Ok(f2_iris_of_height(focus, NonZeroU16::new(8).or_fail()?))
}

/// An iris with an f/2 disc outline, the given frame height in world units, and a two by two grid.
fn f2_iris_of_height(focus: Focus, height: NonZeroU16) -> Aperture {
    Aperture::Iris(Iris::new(
        FStop::F2,
        ApertureShape::Disc,
        focus,
        FrameHeight(height),
        ApertureGrid::Two,
    ))
}

/// A focus on the plane eight scene units from the camera.
fn focus_at_eight() -> Result<Focus, Fail> {
    let distance = FocusDistance::new(NonZeroU32::new(8).or_fail()?).or_fail()?;
    Ok(Focus::At(distance))
}

/// A deep shot whose camera stays at the origin, upright, with the given lens, sample grid,
/// backdrop, and layers.
fn shot(lens: Lens, grid: PixelGrid, backdrop: Rgb8, layers: Vec<Layer>) -> Result<DeepShot, Fail> {
    let pose = Pose::new(fixed(on_axis(RawCoord(0)))?, upright());
    let rig = Rig::new(&pose, lens, &recorder()?);
    DeepShot::new(Stage { backdrop, layers }, &rig, grid).or_fail()
}

/// Renders a shot at one frame index into a fresh eight by eight frame.
fn render_at(shot: &DeepShot, index: FrameIndex) -> Result<Frame, Fail> {
    let mut out = Frame::zeroed(eight_by_eight()?).or_fail()?;
    render_stage_into(shot, index, &mut out).or_fail()?;
    Ok(out)
}

/// The sensels of a block, as a membership test that names both outcomes.
fn block_at(block: Block, cell: Cell) -> Containment {
    let in_columns =
        block.columns.first.0 <= cell.column.0 && cell.column.0 <= block.columns.last.0;
    let in_rows = block.rows.first.0 <= cell.row.0 && cell.row.0 <= block.rows.last.0;
    if in_columns && in_rows {
        Containment::Inside
    } else {
        Containment::Outside
    }
}

/// The block of columns two to five and rows two to five.
fn two_to_five() -> Block {
    Block {
        columns: Span {
            first: PixelColumn(2),
            last: PixelColumn(5),
        },
        rows: Span {
            first: PixelRow(2),
            last: PixelRow(5),
        },
    }
}

/// The block of columns three and four and rows three and four.
fn three_to_four() -> Block {
    Block {
        columns: Span {
            first: PixelColumn(3),
            last: PixelColumn(4),
        },
        rows: Span {
            first: PixelRow(3),
            last: PixelRow(4),
        },
    }
}

/// Checks every sensel of an eight by eight frame against the color expected at that sensel.
fn ensure_every_sensel(frame: &Frame, expected: impl Fn(Cell) -> Rgb8) -> Result<(), Fail> {
    for row in 0..SIDE {
        for column in 0..SIDE {
            let cell = Cell {
                column: PixelColumn(column),
                row: PixelRow(row),
            };
            let found = frame
                .get_pixel(PixelCoord::new(cell.column, cell.row))
                .or_fail()?;
            ensure_eq!(found, expected(cell));
        }
    }
    Ok(())
}

#[test]
fn out_of_focus_iris_spreads_the_rect_over_four_lens_points() {
    assert_eq!(check_out_of_focus_iris_spreads_the_rect(), Ok(()));
}

fn check_out_of_focus_iris_spreads_the_rect() -> Result<(), Fail> {
    let rect = layer(
        square(NonZeroU32::MIN)?,
        white(),
        fixed(on_axis(RawCoord(8)))?,
    )?;
    let scene = shot(
        lens(f2_iris(Focus::Infinity)?)?,
        PixelGrid::One,
        black(),
        vec![rect],
    )?;
    let frame = render_at(&scene, FrameIndex::new(0))?;
    ensure_every_sensel(&frame, |cell| match block_at(two_to_five(), cell) {
        Containment::Inside => gray(Channel(64)),
        Containment::Outside => black(),
    })
}

#[test]
fn in_focus_iris_gives_the_rect_its_sharp_extent() {
    assert_eq!(
        check_in_focus_iris_gives_the_rect_its_sharp_extent(),
        Ok(())
    );
}

fn check_in_focus_iris_gives_the_rect_its_sharp_extent() -> Result<(), Fail> {
    let rect = layer(
        square(NonZeroU32::MIN)?,
        white(),
        fixed(on_axis(RawCoord(8)))?,
    )?;
    // The focus is at the depth of the square, eight scene units, and the iris has a frame height of
    // one scene unit. The rig accepts it because 2 * 8 = 16 = fn * H with fn = 16 and H = 1. The
    // shift of a layer on the plane of focus is zero for any frame height, so the expected frame is
    // unchanged.
    let scene = shot(
        lens(f2_iris_of_height(
            focus_at_eight()?,
            NonZeroU16::new(1).or_fail()?,
        ))?,
        PixelGrid::One,
        black(),
        vec![rect],
    )?;
    let frame = render_at(&scene, FrameIndex::new(0))?;
    ensure_every_sensel(&frame, |cell| match block_at(three_to_four(), cell) {
        Containment::Inside => white(),
        Containment::Outside => black(),
    })
}

#[test]
fn edge_sensels_average_their_samples_half_and_half() {
    assert_eq!(check_edge_sensels_average_their_samples(), Ok(()));
}

fn check_edge_sensels_average_their_samples() -> Result<(), Fail> {
    let rect = layer(
        square(NonZeroU32::MIN)?,
        white(),
        fixed(Spot {
            across: RawCoord(1),
            down: RawCoord(0),
            depth: RawCoord(16),
        })?,
    )?;
    let scene = shot(
        lens(Aperture::Pinhole)?,
        PixelGrid::Two,
        black(),
        vec![rect],
    )?;
    let frame = render_at(&scene, FrameIndex::new(0))?;
    ensure_every_sensel(&frame, |cell| {
        let half_column = PixelColumn(4);
        let upper_edge = PixelRow(3);
        let lower_edge = PixelRow(4);
        if cell.column == half_column && (cell.row == upper_edge || cell.row == lower_edge) {
            gray(Channel(128))
        } else {
            black()
        }
    })
}

#[test]
fn shot_without_layers_shows_the_backdrop_through_a_pinhole() {
    assert_eq!(check_backdrop_through_a_pinhole(), Ok(()));
}

#[test]
fn shot_without_layers_shows_the_backdrop_through_an_iris() {
    assert_eq!(check_backdrop_through_an_iris(), Ok(()));
}

fn check_backdrop_through_a_pinhole() -> Result<(), Fail> {
    backdrop_shows_through(Aperture::Pinhole)
}

fn check_backdrop_through_an_iris() -> Result<(), Fail> {
    backdrop_shows_through(f2_iris(Focus::Infinity)?)
}

/// Checks that a shot with no layers gives the backdrop at every sensel through an aperture.
fn backdrop_shows_through(aperture: Aperture) -> Result<(), Fail> {
    let scene = shot(
        lens(aperture)?,
        PixelGrid::One,
        backdrop_color(),
        Vec::new(),
    )?;
    let frame = render_at(&scene, FrameIndex::new(0))?;
    ensure_every_sensel(&frame, |_| backdrop_color())
}

#[test]
fn frame_of_other_dimensions_is_rejected() {
    assert_eq!(check_frame_of_other_dimensions_is_rejected(), Ok(()));
}

fn check_frame_of_other_dimensions_is_rejected() -> Result<(), Fail> {
    let scene = shot(
        lens(Aperture::Pinhole)?,
        PixelGrid::One,
        black(),
        Vec::new(),
    )?;
    let dimensions = Dimensions::new(Width::new(4).or_fail()?, Height::new(SIDE).or_fail()?);
    let mut out = Frame::zeroed(dimensions).or_fail()?;
    ensure_eq!(
        render_stage_into(&scene, FrameIndex::new(0), &mut out),
        Err(DeepRenderError::FrameSizeMismatch)
    );
    Ok(())
}

#[test]
fn rendering_is_repeatable_and_independent_of_frame_order() {
    assert_eq!(
        check_rendering_is_repeatable_and_independent_of_frame_order(),
        Ok(())
    );
}

fn check_rendering_is_repeatable_and_independent_of_frame_order() -> Result<(), Fail> {
    let walker = layer(
        square(NonZeroU32::MIN)?,
        white(),
        walking_across(on_axis(RawCoord(8)))?,
    )?;
    let scene = shot(
        lens(Aperture::Pinhole)?,
        PixelGrid::One,
        black(),
        vec![walker],
    )?;
    let alone = render_at(&scene, FrameIndex::new(3))?;
    let later = render_at(&scene, FrameIndex::new(5))?;
    ensure_ne!(later, alone);
    let again = render_at(&scene, FrameIndex::new(3))?;
    ensure_eq!(again, alone);
    let first_pass = render_at(&scene, FrameIndex::new(3))?;
    ensure_eq!(first_pass, alone);
    Ok(())
}
