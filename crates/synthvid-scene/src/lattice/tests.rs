use super::*;
use crate::color::{Channel, Rgb8};
use crate::frame::{Frame, PixelColumn, PixelCoord, PixelRow};
use crate::geom::Point;
use crate::raster::fill_polygon;
use crate::ratio::{Numerator, Ratio};
use crate::units::{Dimensions, Height, Width};
use synthvid_testkit::{ensure, ensure_eq, refusal, Fail, OrFail as _};

/// Full red, the fill of the raster cross-checks.
const RED: Rgb8 = Rgb8 {
    red: Channel(255),
    green: Channel(0),
    blue: Channel(0),
};

/// Builds a lattice point from its across and down coordinates.
macro_rules! point {
    ($across:expr_2021, $down:expr_2021) => {
        LatticePoint {
            across: SubSensel($across),
            down: SubSensel($down),
        }
    };
}

/// The indices of a sample slot, across and down, bundled so that the two
/// same-typed indices cannot be passed in the wrong order.
struct SlotAt {
    /// The index across the sensel.
    across: SampleIndex,
    /// The index down the sensel.
    down: SampleIndex,
}

/// Returns the slot of `grid` at the indices in `at`, failing if the grid has no such slot.
fn slot(grid: PixelGrid, at: &SlotAt) -> Result<SampleSlot, Fail> {
    grid.slots()
        .find(|candidate| candidate.across() == at.across && candidate.down() == at.down)
        .ok_or_else(|| Fail::here(format_args!("the grid has no slot at the given indices")))
}

/// Samples sensel `(column, row)` with `grid`, at the slot with the indices `(across, down)`.
///
/// Expands to the lattice point of that sample, as a `Result` that fails if the grid has no such slot.
macro_rules! sampled {
    ($column:expr_2021, $row:expr_2021, $grid:expr_2021, $across:expr_2021, $down:expr_2021) => {
        slot(
            $grid,
            &SlotAt {
                across: SampleIndex($across),
                down: SampleIndex($down),
            },
        )
        .map(|found| sample_point(PixelColumn($column), PixelRow($row), found))
    };
}

/// The square with vertices (256, 256), (768, 256), (768, 768) and (256, 768).
fn square() -> LatticeShape {
    LatticeShape::polygon(
        [point!(256, 256), point!(768, 256), point!(768, 768)],
        vec![point!(256, 768)],
    )
}

/// The rectangle with vertices (128, 0), (640, 0), (640, 256) and (128, 256).
fn rectangle() -> LatticeShape {
    LatticeShape::polygon(
        [point!(128, 0), point!(640, 0), point!(640, 256)],
        vec![point!(128, 256)],
    )
}

/// The triangle with vertices (0, 0), (1024, 0) and (0, 1024).
fn triangle() -> LatticeShape {
    LatticeShape::polygon([point!(0, 0), point!(1024, 0), point!(0, 1024)], Vec::new())
}

/// The L shape with vertices (0, 0), (512, 0), (512, 256), (256, 256), (256, 512)
/// and (0, 512).
fn l_shape() -> LatticeShape {
    LatticeShape::polygon(
        [point!(0, 0), point!(512, 0), point!(512, 256)],
        vec![point!(256, 256), point!(256, 512), point!(0, 512)],
    )
}

/// The triangle with vertices (-37, 11), (301, -5) and (97, 259), in lattice units.
fn skew_triangle() -> LatticeShape {
    LatticeShape::polygon(
        [point!(-37, 11), point!(301, -5), point!(97, 259)],
        Vec::new(),
    )
}

/// Returns whether `point` lies in `shape`, failing if exact arithmetic overflows.
fn contained(shape: &LatticeShape, point: LatticePoint) -> Result<Containment, Fail> {
    covers(shape, point).or_fail()
}

/// The ratio `numerator / 256`, the sensel coordinate of a lattice coordinate of `numerator`.
fn over_256(numerator: Numerator) -> Result<Ratio, Fail> {
    Ratio::from_integer(numerator)
        .checked_div(Ratio::from_integer(Numerator(256)))
        .or_fail()
}

#[test]
fn square_covers_interior_edges_and_vertices() {
    assert_eq!(check_square_covers_interior_edges_and_vertices(), Ok(()));
}

fn check_square_covers_interior_edges_and_vertices() -> Result<(), Fail> {
    let shape = square();
    ensure_eq!(contained(&shape, point!(384, 384))?, Containment::Inside);
    ensure_eq!(contained(&shape, point!(896, 384))?, Containment::Outside);
    ensure_eq!(contained(&shape, point!(128, 128))?, Containment::Outside);
    ensure_eq!(contained(&shape, point!(256, 256))?, Containment::Inside);
    ensure_eq!(contained(&shape, point!(512, 768))?, Containment::Inside);
    Ok(())
}

#[test]
fn rectangle_sensel_columns_follow_its_edges() {
    assert_eq!(check_rectangle_sensel_columns_follow_its_edges(), Ok(()));
}

fn check_rectangle_sensel_columns_follow_its_edges() -> Result<(), Fail> {
    let shape = rectangle();
    let columns = [
        (0_u16, 128_i64, Containment::Inside),
        (1, 384, Containment::Inside),
        (2, 640, Containment::Inside),
        (3, 896, Containment::Outside),
    ];
    for (column, across, expected) in columns {
        let sample = sampled!(column, 0, PixelGrid::One, 0, 0)?;
        ensure_eq!(sample, point!(across, 128));
        ensure_eq!(contained(&shape, sample)?, expected);
    }
    Ok(())
}

#[test]
fn disc_samples_follow_squared_distance() {
    assert_eq!(check_disc_samples_follow_squared_distance(), Ok(()));
}

fn check_disc_samples_follow_squared_distance() -> Result<(), Fail> {
    // Radius 369 squared is 136161. The first sample is 32768 away squared and
    // the second is 163840 away squared.
    let disc = LatticeShape::disc(point!(8192, 6144), DiscRadius(369));
    let inside = sampled!(32, 24, PixelGrid::One, 0, 0)?;
    ensure_eq!(inside, point!(8320, 6272));
    ensure_eq!(contained(&disc, inside)?, Containment::Inside);
    let outside = sampled!(33, 24, PixelGrid::One, 0, 0)?;
    ensure_eq!(outside, point!(8576, 6272));
    ensure_eq!(contained(&disc, outside)?, Containment::Outside);
    Ok(())
}

#[test]
fn disc_boundary_is_inside() {
    assert_eq!(check_disc_boundary_is_inside(), Ok(()));
}

fn check_disc_boundary_is_inside() -> Result<(), Fail> {
    // 384 squared plus 512 squared is 409600, which is 640 squared exactly.
    let disc = LatticeShape::disc(point!(0, 128), DiscRadius(640));
    let boundary = sampled!(1, 2, PixelGrid::One, 0, 0)?;
    ensure_eq!(boundary, point!(384, 640));
    ensure_eq!(contained(&disc, boundary)?, Containment::Inside);
    ensure_eq!(contained(&disc, point!(385, 640))?, Containment::Outside);
    Ok(())
}

#[test]
fn sample_offsets_divide_the_sensel_evenly() {
    assert_eq!(check_sample_offsets_divide_the_sensel_evenly(), Ok(()));
}

fn check_sample_offsets_divide_the_sensel_evenly() -> Result<(), Fail> {
    ensure_eq!(sampled!(4, 0, PixelGrid::Two, 0, 0)?, point!(1088, 64));
    ensure_eq!(sampled!(4, 0, PixelGrid::Two, 1, 0)?, point!(1216, 64));
    ensure_eq!(sampled!(0, 0, PixelGrid::Four, 3, 0)?, point!(224, 32));
    ensure_eq!(sampled!(1, 0, PixelGrid::Eight, 7, 0)?, point!(496, 16));
    ensure_eq!(sampled!(0, 0, PixelGrid::Eight, 7, 7)?, point!(240, 240));
    Ok(())
}

#[test]
fn grid_slots_are_exactly_the_samples_of_the_grid() {
    assert_eq!(
        check_grid_slots_are_exactly_the_samples_of_the_grid(),
        Ok(())
    );
}

fn check_grid_slots_are_exactly_the_samples_of_the_grid() -> Result<(), Fail> {
    // Each grid has one slot per pair of indices in 0..G, so G squared slots.
    ensure_eq!(PixelGrid::One.slots().count(), 1_usize);
    ensure_eq!(PixelGrid::Two.slots().count(), 4_usize);
    ensure_eq!(PixelGrid::Four.slots().count(), 16_usize);
    ensure_eq!(PixelGrid::Eight.slots().count(), 64_usize);
    // Index 2 does not exist in a grid of two samples per axis.
    ensure!(slot(
        PixelGrid::Two,
        &SlotAt {
            across: SampleIndex(2),
            down: SampleIndex(0),
        }
    )
    .is_err());
    Ok(())
}

#[test]
fn per_axis_counts_are_two_four_and_eight() {
    assert_eq!(check_per_axis_counts_are_two_four_and_eight(), Ok(()));
}

fn check_per_axis_counts_are_two_four_and_eight() -> Result<(), Fail> {
    ensure_eq!(PixelGrid::One.per_axis().get(), 1_u32);
    ensure_eq!(PixelGrid::Two.per_axis().get(), 2_u32);
    ensure_eq!(PixelGrid::Four.per_axis().get(), 4_u32);
    ensure_eq!(PixelGrid::Eight.per_axis().get(), 8_u32);
    Ok(())
}

#[test]
fn zero_radius_disc_covers_nothing() {
    assert_eq!(check_zero_radius_disc_covers_nothing(), Ok(()));
}

fn check_zero_radius_disc_covers_nothing() -> Result<(), Fail> {
    let disc = LatticeShape::disc(point!(0, 0), DiscRadius(0));
    ensure_eq!(contained(&disc, point!(0, 0))?, Containment::Outside);
    Ok(())
}

#[test]
fn disc_radius_beyond_the_i64_range_still_covers_exactly() {
    assert_eq!(
        check_disc_radius_beyond_the_i64_range_still_covers_exactly(),
        Ok(())
    );
}

fn check_disc_radius_beyond_the_i64_range_still_covers_exactly() -> Result<(), Fail> {
    // The radius 2^63 is one more than i64::MAX, so the bounding box cannot be
    // written in i64. Its square, 2^126, still fits in i128, so containment is
    // exact. The point (1, 1) is 2 away squared, which is within it.
    let disc = LatticeShape::disc(point!(0, 0), DiscRadius(0x8000_0000_0000_0000));
    ensure_eq!(contained(&disc, point!(1, 1))?, Containment::Inside);
    ensure_eq!(refusal(disc.bounds())?, LatticeError::Overflow);
    Ok(())
}

#[test]
fn triangle_covers_its_hypotenuse() {
    assert_eq!(check_triangle_covers_its_hypotenuse(), Ok(()));
}

fn check_triangle_covers_its_hypotenuse() -> Result<(), Fail> {
    let shape = triangle();
    ensure_eq!(contained(&shape, point!(256, 256))?, Containment::Inside);
    ensure_eq!(contained(&shape, point!(768, 768))?, Containment::Outside);
    ensure_eq!(contained(&shape, point!(512, 512))?, Containment::Inside);
    Ok(())
}

#[test]
fn concave_l_shape_leaves_its_notch_outside() {
    assert_eq!(check_concave_l_shape_leaves_its_notch_outside(), Ok(()));
}

fn check_concave_l_shape_leaves_its_notch_outside() -> Result<(), Fail> {
    let shape = l_shape();
    ensure_eq!(contained(&shape, point!(384, 384))?, Containment::Outside);
    ensure_eq!(contained(&shape, point!(128, 128))?, Containment::Inside);
    ensure_eq!(contained(&shape, point!(128, 384))?, Containment::Inside);
    Ok(())
}

#[test]
fn l_shape_ray_through_a_vertex_follows_even_odd() {
    assert_eq!(
        check_l_shape_ray_through_a_vertex_follows_even_odd(),
        Ok(())
    );
}

fn check_l_shape_ray_through_a_vertex_follows_even_odd() -> Result<(), Fail> {
    // The L is the union of [0, 512] x [0, 256] and [0, 256] x [256, 512].
    // The point (128, 256) lies inside it: (128, 255) and (128, 257) are both
    // inside, so it is interior.
    //
    // Its rightward ray runs along y = 256 and passes the reflex vertex
    // (256, 256), then runs along the edge from (256, 256) to (512, 256).
    // Counting edges with the half-open rule (an endpoint is below when its
    // down coordinate is at most 256): the edge (512, 256) to (256, 256) has
    // both endpoints at or below, so it does not count, and the edge
    // (256, 256) to (256, 512) has its start at or below and its end strictly
    // below, so it counts once and crosses the ray at x = 256 to the right.
    // The edge (0, 512) to (0, 0) crosses y = 256 at x = 0, to the left, so it
    // does not flip the parity. One crossing is odd, so the point is inside.
    let shape = l_shape();
    ensure_eq!(contained(&shape, point!(128, 256))?, Containment::Inside);
    // (300, 256) lies on the top edge from (256, 256) to (512, 256).
    ensure_eq!(contained(&shape, point!(300, 256))?, Containment::Inside);
    // (300, 255) is in the top block, and (300, 257) is in the notch below it.
    ensure_eq!(contained(&shape, point!(300, 255))?, Containment::Inside);
    ensure_eq!(contained(&shape, point!(300, 257))?, Containment::Outside);
    // (200, 300) is in the lower block, and (400, 300) is right of it.
    ensure_eq!(contained(&shape, point!(200, 300))?, Containment::Inside);
    ensure_eq!(contained(&shape, point!(400, 300))?, Containment::Outside);
    Ok(())
}

#[test]
fn skew_triangle_matches_hand_checked_points() {
    assert_eq!(check_skew_triangle_matches_hand_checked_points(), Ok(()));
}

fn check_skew_triangle_matches_hand_checked_points() -> Result<(), Fail> {
    // Edge AB from (-37, 11) to (301, -5) has y = 11 - 16 (x + 37) / 338, which
    // is 4.5148... at x = 100. Edge CA from (-37, 11) to (97, 259) has
    // x = -37 + 134 (y - 11) / 248, which is 11.0887... at y = 100. Edge BC
    // from (301, -5) to (97, 259) has x = 301 - 204 (y + 5) / 264, which is
    // 219.8636... at y = 100, and on y = 11 it crosses at x = 3175 / 11 = 288.6...
    let shape = skew_triangle();
    // Inside, strictly between the edges.
    ensure_eq!(contained(&shape, point!(100, 5))?, Containment::Inside);
    ensure_eq!(contained(&shape, point!(12, 100))?, Containment::Inside);
    ensure_eq!(contained(&shape, point!(0, 11))?, Containment::Inside);
    // Vertex A is on two edges, so it is inside.
    ensure_eq!(contained(&shape, point!(-37, 11))?, Containment::Inside);
    // Outside, one on each side of an edge.
    ensure_eq!(contained(&shape, point!(100, 4))?, Containment::Outside);
    ensure_eq!(contained(&shape, point!(11, 100))?, Containment::Outside);
    ensure_eq!(contained(&shape, point!(220, 100))?, Containment::Outside);
    // On the horizontal line through vertex A, beyond the vertex and beyond edge BC.
    ensure_eq!(contained(&shape, point!(-38, 11))?, Containment::Outside);
    ensure_eq!(contained(&shape, point!(289, 11))?, Containment::Outside);
    Ok(())
}

#[test]
fn raster_coverage_of_a_triangle_agrees_at_pixel_centers() {
    assert_eq!(
        check_raster_coverage_of_a_triangle_agrees_at_pixel_centers(),
        Ok(())
    );
}

fn check_raster_coverage_of_a_triangle_agrees_at_pixel_centers() -> Result<(), Fail> {
    let vertices = [
        Point {
            across: over_256(Numerator(-37))?,
            down: over_256(Numerator(11))?,
        },
        Point {
            across: over_256(Numerator(301))?,
            down: over_256(Numerator(-5))?,
        },
        Point {
            across: over_256(Numerator(97))?,
            down: over_256(Numerator(259))?,
        },
    ];
    let mut frame = Frame::zeroed(Dimensions::new(
        Width::new(2).or_fail()?,
        Height::new(2).or_fail()?,
    ))
    .or_fail()?;
    fill_polygon(&mut frame, &vertices, RED).or_fail()?;
    // The centers of the four pixels are at lattice (128, 128), (384, 128),
    // (128, 384) and (384, 384). On y = 128 the triangle spans x from 26.2 to
    // 198.2, so only the first center lies in it. The triangle reaches no
    // further down than y = 259, so the two centers at y = 384 are below it.
    let centers = [
        (PixelColumn(0), PixelRow(0), point!(128, 128), true),
        (PixelColumn(1), PixelRow(0), point!(384, 128), false),
        (PixelColumn(0), PixelRow(1), point!(128, 384), false),
        (PixelColumn(1), PixelRow(1), point!(384, 384), false),
    ];
    let shape = skew_triangle();
    for (column, row, center, expected) in centers {
        let painted = frame.pixel(PixelCoord::new(column, row)) == Some(RED);
        let covered = contained(&shape, center)? == Containment::Inside;
        ensure_eq!(
            painted,
            expected,
            "raster paints the pixel only when expected"
        );
        ensure_eq!(
            covered,
            expected,
            "lattice covers the center only when expected"
        );
    }
    Ok(())
}

#[test]
fn raster_coverage_of_an_l_shape_agrees_at_pixel_centers() {
    assert_eq!(
        check_raster_coverage_of_an_l_shape_agrees_at_pixel_centers(),
        Ok(())
    );
}

fn check_raster_coverage_of_an_l_shape_agrees_at_pixel_centers() -> Result<(), Fail> {
    let vertices = [
        Point {
            across: Ratio::from_integer(Numerator(0)),
            down: Ratio::from_integer(Numerator(0)),
        },
        Point {
            across: Ratio::from_integer(Numerator(2)),
            down: Ratio::from_integer(Numerator(0)),
        },
        Point {
            across: Ratio::from_integer(Numerator(2)),
            down: Ratio::from_integer(Numerator(1)),
        },
        Point {
            across: Ratio::from_integer(Numerator(1)),
            down: Ratio::from_integer(Numerator(1)),
        },
        Point {
            across: Ratio::from_integer(Numerator(1)),
            down: Ratio::from_integer(Numerator(2)),
        },
        Point {
            across: Ratio::from_integer(Numerator(0)),
            down: Ratio::from_integer(Numerator(2)),
        },
    ];
    let mut frame = Frame::zeroed(Dimensions::new(
        Width::new(2).or_fail()?,
        Height::new(2).or_fail()?,
    ))
    .or_fail()?;
    fill_polygon(&mut frame, &vertices, RED).or_fail()?;
    // The notch is the pixel at (1, 1), whose center lies right of the vertical
    // edge at x = 256 and below the top block.
    let centers = [
        (PixelColumn(0), PixelRow(0), point!(128, 128), true),
        (PixelColumn(1), PixelRow(0), point!(384, 128), true),
        (PixelColumn(0), PixelRow(1), point!(128, 384), true),
        (PixelColumn(1), PixelRow(1), point!(384, 384), false),
    ];
    let shape = l_shape();
    for (column, row, center, expected) in centers {
        let painted = frame.pixel(PixelCoord::new(column, row)) == Some(RED);
        let covered = contained(&shape, center)? == Containment::Inside;
        ensure_eq!(
            painted,
            expected,
            "raster paints the pixel only when expected"
        );
        ensure_eq!(
            covered,
            expected,
            "lattice covers the center only when expected"
        );
    }
    Ok(())
}

#[test]
fn raster_coverage_on_edges_and_vertices_agrees_on_a_three_by_three_frame() {
    assert_eq!(
        check_raster_coverage_on_edges_and_vertices_agrees_on_a_three_by_three_frame(),
        Ok(())
    );
}

fn check_raster_coverage_on_edges_and_vertices_agrees_on_a_three_by_three_frame() -> Result<(), Fail>
{
    // The triangle A = (128, 128), B = (640, 384), C = (128, 640), in lattice units. Edge AB lies
    // on y = 128 + (x - 128) / 2, edge AC on x = 128, and edge BC on y = 384 + (640 - x) / 2. The
    // pixel centers of the 3 by 3 frame are at 128, 384, and 640 on each axis.
    let vertices = [
        Point {
            across: over_256(Numerator(128))?,
            down: over_256(Numerator(128))?,
        },
        Point {
            across: over_256(Numerator(640))?,
            down: over_256(Numerator(384))?,
        },
        Point {
            across: over_256(Numerator(128))?,
            down: over_256(Numerator(640))?,
        },
    ];
    let mut frame = Frame::zeroed(Dimensions::new(
        Width::new(3).or_fail()?,
        Height::new(3).or_fail()?,
    ))
    .or_fail()?;
    fill_polygon(&mut frame, &vertices, RED).or_fail()?;
    // Each center is decided by hand. A center on an edge, including a vertex, is inside. A center
    // off every edge follows the even-odd rule with the half-open crossing rule. Centers at
    // (384, 128) and (640, 128) lie above edge AB. Centers at (384, 640) and (640, 640) lie below
    // edge BC, since BC is at y = 512 when x = 384 and at y = 384 when x = 640. Center (384, 384)
    // lies strictly between AB, at y = 256, and BC, at y = 512, so it is inside. Center (640, 128),
    // on the row of vertex A, has an even crossing count, since both edges that cross its row lie
    // to its left.
    let shape = LatticeShape::polygon(
        [point!(128, 128), point!(640, 384), point!(128, 640)],
        Vec::new(),
    );
    let centers = [
        (PixelColumn(0), PixelRow(0), point!(128, 128), true),
        (PixelColumn(1), PixelRow(0), point!(384, 128), false),
        (PixelColumn(2), PixelRow(0), point!(640, 128), false),
        (PixelColumn(0), PixelRow(1), point!(128, 384), true),
        (PixelColumn(1), PixelRow(1), point!(384, 384), true),
        (PixelColumn(2), PixelRow(1), point!(640, 384), true),
        (PixelColumn(0), PixelRow(2), point!(128, 640), true),
        (PixelColumn(1), PixelRow(2), point!(384, 640), false),
        (PixelColumn(2), PixelRow(2), point!(640, 640), false),
    ];
    for (column, row, center, expected) in centers {
        let painted = frame.pixel(PixelCoord::new(column, row)) == Some(RED);
        let covered = contained(&shape, center)? == Containment::Inside;
        ensure_eq!(
            covered,
            expected,
            "lattice covers the center {center:?} only when expected"
        );
        ensure_eq!(
            painted,
            expected,
            "raster paints the pixel at {column:?}, {row:?} only when expected"
        );
    }
    Ok(())
}

#[test]
fn less_moves_each_square_vertex_back_by_the_shift() {
    assert_eq!(
        check_less_moves_each_square_vertex_back_by_the_shift(),
        Ok(())
    );
}

fn check_less_moves_each_square_vertex_back_by_the_shift() -> Result<(), Fail> {
    // Moving back by (256, 512) returns each shifted square vertex to the one listed first:
    // (512, 768) to (256, 256), (1024, 768) to (768, 256), (1024, 1280) to (768, 768) and
    // (512, 1280) to (256, 768).
    let shift = LatticeShift {
        across: SubSensel(256),
        down: SubSensel(512),
    };
    let moves = [
        (point!(512, 768), point!(256, 256)),
        (point!(1024, 768), point!(768, 256)),
        (point!(1024, 1280), point!(768, 768)),
        (point!(512, 1280), point!(256, 768)),
    ];
    for (moved, expected) in moves {
        ensure_eq!(moved.less(shift).or_fail()?, expected);
    }
    let shape = LatticeShape::polygon(
        [point!(512, 768), point!(1024, 768), point!(1024, 1280)],
        vec![point!(512, 1280)],
    );
    ensure_eq!(contained(&shape, point!(640, 896))?, Containment::Inside);
    ensure_eq!(contained(&shape, point!(1024, 1280))?, Containment::Inside);
    ensure_eq!(contained(&shape, point!(256, 256))?, Containment::Outside);
    Ok(())
}

#[test]
fn log2_per_axis_is_the_shift_that_gives_the_samples_per_axis() {
    assert_eq!(
        check_log2_per_axis_is_the_shift_that_gives_the_samples_per_axis(),
        Ok(())
    );
}

fn check_log2_per_axis_is_the_shift_that_gives_the_samples_per_axis() -> Result<(), Fail> {
    // One sample is 2^0, two is 2^1, four is 2^2 and eight is 2^3.
    let grids = [
        (PixelGrid::One, 0_u32, 1_u32),
        (PixelGrid::Two, 1, 2),
        (PixelGrid::Four, 2, 4),
        (PixelGrid::Eight, 3, 8),
    ];
    for (grid, shift, samples) in grids {
        ensure_eq!(grid.log2_per_axis().0, shift);
        ensure_eq!(1_u32.checked_shl(shift), Some(samples));
        ensure_eq!(grid.per_axis().get(), samples);
    }
    Ok(())
}

#[test]
fn square_bounds_are_its_extreme_vertices() {
    assert_eq!(check_square_bounds_are_its_extreme_vertices(), Ok(()));
}

fn check_square_bounds_are_its_extreme_vertices() -> Result<(), Fail> {
    ensure_eq!(
        square().bounds().or_fail()?,
        LatticeBounds {
            low: point!(256, 256),
            high: point!(768, 768),
        }
    );
    Ok(())
}

#[test]
fn disc_bounds_extend_the_radius_on_every_side() {
    assert_eq!(check_disc_bounds_extend_the_radius_on_every_side(), Ok(()));
}

fn check_disc_bounds_extend_the_radius_on_every_side() -> Result<(), Fail> {
    // Center (8192, 6144) with radius 369 gives low (7823, 5775) and high
    // (8561, 6513).
    let disc = LatticeShape::disc(point!(8192, 6144), DiscRadius(369));
    ensure_eq!(
        disc.bounds().or_fail()?,
        LatticeBounds {
            low: point!(7823, 5775),
            high: point!(8561, 6513),
        }
    );
    Ok(())
}

#[test]
fn disc_bounds_past_the_i64_range_overflow() {
    assert_eq!(check_disc_bounds_past_the_i64_range_overflow(), Ok(()));
}

fn check_disc_bounds_past_the_i64_range_overflow() -> Result<(), Fail> {
    let disc = LatticeShape::disc(point!(i64::MAX, 0), DiscRadius(1));
    ensure_eq!(refusal(disc.bounds())?, LatticeError::Overflow);
    Ok(())
}
