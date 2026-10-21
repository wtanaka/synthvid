use super::super::coverage::pixel_centre;
use super::super::coverage_types::{Column, Row};
use super::*;
use crate::color::{Channel, Rgb8};
use crate::frame::{Frame, PixelColumn, PixelCoord, PixelRow};
use crate::raster::{draw_cross, draw_line, fill_disc, fill_polygon, fill_rect};
use crate::testutil::black_8x8;
use crate::units::{Dimensions, Height, Width};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// Which test shape a painted frame holds, named in failure messages.
#[derive(Copy, Clone, Debug)]
enum Drawn {
    Disc,
    Polygon,
    Rect,
    Line,
    Cross,
}

/// Builds a frame large enough to hold every test shape whole.
fn large_frame() -> Result<Frame, Fail> {
    let width = Width::new(32).or_fail()?;
    let height = Height::new(32).or_fail()?;
    Frame::zeroed(Dimensions::new(width, height)).or_fail()
}

/// Collects the centers of all non-background pixels in a frame.
fn painted_centres(frame: &Frame, ground: Rgb8) -> Vec<Point> {
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    let mut found = Vec::new();
    for row in 0..height {
        for column in 0..width {
            if frame.pixel(PixelCoord::new(PixelColumn(column), PixelRow(row))) != Some(ground) {
                found.push(pixel_centre(Column(column), Row(row)));
            }
        }
    }
    found
}

/// Checks every painted pixel center lies inside `bounds`.
fn ensure_inside(bounds: Bounds, frame: &Frame, ground: Rgb8, label: Drawn) -> Result<(), Fail> {
    for sample in painted_centres(frame, ground) {
        ensure!(
            bounds.min_x <= sample.across,
            "{label:?}: painted x must not lie left of the extent"
        );
        ensure!(
            sample.across <= bounds.max_x,
            "{label:?}: painted x must not lie right of the extent"
        );
        ensure!(
            bounds.min_y <= sample.down,
            "{label:?}: painted y must not lie above the extent"
        );
        ensure!(
            sample.down <= bounds.max_y,
            "{label:?}: painted y must not lie below the extent"
        );
    }
    Ok(())
}

/// Returns the box of a covering extent, failing on an empty one.
fn covering(extent: BoundsOrEmpty, label: Drawn) -> Result<Bounds, Fail> {
    match extent {
        BoundsOrEmpty::Covering(bounds) => Ok(bounds),
        BoundsOrEmpty::Empty => Err(Fail::here(format_args!(
            "{label:?} extent should not be empty"
        ))),
    }
}

#[test]
fn bounds_intersect_area() {
    assert_eq!(check_bounds_intersect_area(), Ok(()));
}

fn check_bounds_intersect_area() -> Result<(), Fail> {
    let a_min_x = Ratio::from_integer(Numerator(0));
    let a_min_y = Ratio::from_integer(Numerator(0));
    let a_max_x = Ratio::from_integer(Numerator(4));
    let a_max_y = Ratio::from_integer(Numerator(4));
    let first = Bounds::new(Corners {
        min: Point {
            across: a_min_x,
            down: a_min_y,
        },
        max: Point {
            across: a_max_x,
            down: a_max_y,
        },
    })
    .or_fail()?;
    let b_min_x = Ratio::from_integer(Numerator(2));
    let b_min_y = Ratio::from_integer(Numerator(2));
    let b_max_x = Ratio::from_integer(Numerator(6));
    let b_max_y = Ratio::from_integer(Numerator(6));
    let second = Bounds::new(Corners {
        min: Point {
            across: b_min_x,
            down: b_min_y,
        },
        max: Point {
            across: b_max_x,
            down: b_max_y,
        },
    })
    .or_fail()?;
    let overlap = first.intersect(second).or_fail()?;
    ensure_eq!(overlap.min_x, b_min_x, "overlap must start at 2");
    ensure_eq!(overlap.max_x, a_max_x, "overlap must end at 4");
    let two = Ratio::from_integer(Numerator(2));
    let expected = two.checked_mul(two).or_fail()?;
    ensure_eq!(overlap.area(), Ok(expected), "2x2 overlap must have area 4");
    let sixteen = Ratio::from_integer(Numerator(16));
    ensure_eq!(first.area(), Ok(sixteen), "4x4 box must have area 16");
    check_bounds_rejections(first)
}

/// Checks that `first`, the box from `(0, 0)` to `(4, 4)`, does not meet a
/// disjoint box, and that its corners swapped on one axis are refused.
fn check_bounds_rejections(first: Bounds) -> Result<(), Fail> {
    let far_min = Ratio::from_integer(Numerator(10));
    let far_max = Ratio::from_integer(Numerator(12));
    let far = Bounds::new(Corners {
        min: Point {
            across: far_min,
            down: far_min,
        },
        max: Point {
            across: far_max,
            down: far_max,
        },
    })
    .or_fail()?;
    ensure_eq!(
        first.intersect(far),
        Err(BoundsError::Inverted),
        "disjoint boxes must not intersect"
    );
    ensure_eq!(
        Bounds::new(Corners {
            min: Point {
                across: first.max_x,
                down: first.min_y
            },
            max: Point {
                across: first.min_x,
                down: first.max_y
            }
        }),
        Err(BoundsError::Inverted),
        "an inverted box must be rejected"
    );
    Ok(())
}

#[test]
fn disc_extent_contains_pixels() {
    assert_eq!(check_disc_extent_contains_pixels(), Ok(()));
}

fn check_disc_extent_contains_pixels() -> Result<(), Fail> {
    let mut frame = large_frame()?;
    let ground = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    };
    frame.fill(ground);
    let paint = Rgb8 {
        red: Channel(255),
        green: Channel(0),
        blue: Channel(0),
    };
    let cx = Ratio::from_integer(Numerator(16));
    let cy = Ratio::from_integer(Numerator(16));
    let radius = Ratio::from_integer(Numerator(5));
    let centre = Point {
        across: cx,
        down: cy,
    };
    fill_disc(&mut frame, centre, radius, paint).or_fail()?;
    let bounds = covering(disc_extent(centre, radius).or_fail()?, Drawn::Disc)?;
    ensure_inside(bounds, &frame, ground, Drawn::Disc)?;
    ensure!(
        !painted_centres(&frame, ground).is_empty(),
        "the test disc must paint at least one pixel"
    );
    let zero = Ratio::from_integer(Numerator(0));
    ensure_eq!(
        disc_extent(centre, zero),
        Ok(BoundsOrEmpty::Empty),
        "a zero radius must have no extent"
    );
    Ok(())
}

#[test]
fn polygon_extent_contains_pixels() {
    assert_eq!(check_polygon_extent_contains_pixels(), Ok(()));
}

fn check_polygon_extent_contains_pixels() -> Result<(), Fail> {
    let mut frame = large_frame()?;
    let ground = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    };
    frame.fill(ground);
    let paint = Rgb8 {
        red: Channel(0),
        green: Channel(255),
        blue: Channel(0),
    };
    let twelve = Ratio::from_integer(Numerator(12));
    let twenty = Ratio::from_integer(Numerator(20));
    let sixteen = Ratio::from_integer(Numerator(16));
    let vertices = [
        Point {
            across: twelve,
            down: twelve,
        },
        Point {
            across: twenty,
            down: twelve,
        },
        Point {
            across: sixteen,
            down: twenty,
        },
    ];
    let slice = &vertices[0..3];
    fill_polygon(&mut frame, slice, paint).or_fail()?;
    let bounds = covering(polygon_extent(slice).or_fail()?, Drawn::Polygon)?;
    ensure_inside(bounds, &frame, ground, Drawn::Polygon)?;
    ensure!(
        !painted_centres(&frame, ground).is_empty(),
        "the test triangle must paint at least one pixel"
    );
    let pair = &vertices[0..2];
    ensure_eq!(
        polygon_extent(pair),
        Ok(BoundsOrEmpty::Empty),
        "two vertices must have no extent"
    );
    Ok(())
}

#[test]
fn rect_extent_contains_pixels() {
    assert_eq!(check_rect_extent_contains_pixels(), Ok(()));
}

fn check_rect_extent_contains_pixels() -> Result<(), Fail> {
    let mut frame = large_frame()?;
    let ground = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    };
    frame.fill(ground);
    let paint = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(255),
    };
    let ten = Ratio::from_integer(Numerator(10));
    let twenty = Ratio::from_integer(Numerator(20));
    let lower = Point {
        across: ten,
        down: ten,
    };
    let upper = Point {
        across: twenty,
        down: twenty,
    };
    let upright = Corners {
        min: lower,
        max: upper,
    };
    fill_rect(&mut frame, upright, paint).or_fail()?;
    let bounds = covering(rect_extent(upright), Drawn::Rect)?;
    ensure_inside(bounds, &frame, ground, Drawn::Rect)?;
    ensure_eq!(
        rect_extent(Corners {
            min: upper,
            max: lower,
        }),
        BoundsOrEmpty::Empty,
        "an inverted rectangle must have no extent"
    );
    Ok(())
}

#[test]
fn line_extent_contains_pixels() {
    assert_eq!(check_line_extent_contains_pixels(), Ok(()));
}

fn check_line_extent_contains_pixels() -> Result<(), Fail> {
    let mut frame = large_frame()?;
    let ground = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    };
    frame.fill(ground);
    let paint = Rgb8 {
        red: Channel(255),
        green: Channel(255),
        blue: Channel(0),
    };
    let ten = Ratio::from_integer(Numerator(10));
    let twenty_two = Ratio::from_integer(Numerator(22));
    let sixteen = Ratio::from_integer(Numerator(16));
    let start = Point {
        across: ten,
        down: sixteen,
    };
    let end = Point {
        across: twenty_two,
        down: sixteen,
    };
    draw_line(&mut frame, Segment { start, end }, paint).or_fail()?;
    let bounds = covering(line_extent(Segment { start, end }).or_fail()?, Drawn::Line)?;
    ensure_inside(bounds, &frame, ground, Drawn::Line)?;
    ensure!(
        !painted_centres(&frame, ground).is_empty(),
        "the test line must paint at least one pixel"
    );
    Ok(())
}

#[test]
fn cross_extent_contains_pixels() {
    assert_eq!(check_cross_extent_contains_pixels(), Ok(()));
}

fn check_cross_extent_contains_pixels() -> Result<(), Fail> {
    let mut frame = large_frame()?;
    let ground = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    };
    frame.fill(ground);
    let paint = Rgb8 {
        red: Channel(255),
        green: Channel(0),
        blue: Channel(255),
    };
    let sixteen = Ratio::from_integer(Numerator(16));
    let arm = Ratio::from_integer(Numerator(6));
    let thick = Ratio::from_integer(Numerator(2));
    let centre = Point {
        across: sixteen,
        down: sixteen,
    };
    let cross = Cross {
        centre,
        arm,
        thickness: thick,
    };
    draw_cross(&mut frame, cross, paint).or_fail()?;
    let bounds = covering(cross_extent(cross).or_fail()?, Drawn::Cross)?;
    ensure_inside(bounds, &frame, ground, Drawn::Cross)?;
    ensure!(
        !painted_centres(&frame, ground).is_empty(),
        "the test cross must paint at least one pixel"
    );
    let neg = Ratio::from_integer(Numerator(-1));
    ensure_eq!(
        cross_extent(Cross {
            centre,
            arm: neg,
            thickness: thick,
        }),
        Ok(BoundsOrEmpty::Empty),
        "a negative arm must have no extent"
    );
    Ok(())
}

#[test]
fn straddling_shapes_clip_correctly() {
    assert_eq!(check_straddling_shapes_clip_correctly(), Ok(()));
}

fn check_straddling_shapes_clip_correctly() -> Result<(), Fail> {
    let mut frame = black_8x8()?;
    let paint = Rgb8 {
        red: Channel(200),
        green: Channel(40),
        blue: Channel(40),
    };
    let neg_two = Ratio::from_integer(Numerator(-2));
    let ten = Ratio::from_integer(Numerator(10));
    let four = Ratio::from_integer(Numerator(4));
    fill_disc(
        &mut frame,
        Point {
            across: four,
            down: four,
        },
        ten,
        paint,
    )
    .or_fail()?;
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(0), PixelRow(0))),
        Some(paint),
        "a huge disc must cover the corner pixel"
    );
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(7), PixelRow(7))),
        Some(paint),
        "a huge disc must cover the far corner pixel"
    );
    let corners = [
        Point {
            across: neg_two,
            down: neg_two,
        },
        Point {
            across: ten,
            down: neg_two,
        },
        Point {
            across: ten,
            down: ten,
        },
        Point {
            across: neg_two,
            down: ten,
        },
    ];
    let huge = &corners[0..4];
    fill_polygon(&mut frame, huge, paint).or_fail()?;
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(7), PixelRow(7))),
        Some(paint),
        "a huge polygon must cover the far corner pixel"
    );
    check_straddling_line_and_cross(&mut frame, paint)
}

/// Checks that a line and a cross reaching past the edges of the 8x8
/// `frame` still paint the pixels they cover inside it.
fn check_straddling_line_and_cross(frame: &mut Frame, paint: Rgb8) -> Result<(), Fail> {
    let neg_two = Ratio::from_integer(Numerator(-2));
    let ten = Ratio::from_integer(Numerator(10));
    let four = Ratio::from_integer(Numerator(4));
    let three = Ratio::from_integer(Numerator(3));
    draw_line(
        frame,
        Segment {
            start: Point {
                across: neg_two,
                down: neg_two,
            },
            end: Point {
                across: ten,
                down: ten,
            },
        },
        paint,
    )
    .or_fail()?;
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(4), PixelRow(4))),
        Some(paint),
        "a straddling line must cover the center pixel"
    );
    draw_cross(
        frame,
        Cross {
            centre: Point {
                across: four,
                down: four,
            },
            arm: ten,
            thickness: three,
        },
        paint,
    )
    .or_fail()?;
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(4), PixelRow(0))),
        Some(paint),
        "a straddling cross must cover the top-middle pixel"
    );
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(0), PixelRow(0))),
        Some(paint),
        "a straddling cross must cover the corner through the disc"
    );
    Ok(())
}
