use super::*;
use crate::color::Channel;
use crate::frame::{PixelColumn, PixelCoord, PixelRow};
use crate::geom::Similarity;
use crate::testutil::Frac;
use crate::units::{Dimensions, FrameIndex, Height, ObjectIndex, Width};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// Builds the identity camera placement used for shape tests.
fn identity_placement() -> CameraFrame {
    CameraFrame {
        transform: Similarity::identity(),
        zoom: Ratio::from_integer(Numerator(1)),
    }
}

/// Draws one shape centred at `(9 / 2, 9 / 2)` on an 8x8 black frame.
fn render_shape(shape: &Shape, fill: Rgb8) -> Result<Frame, Fail> {
    let width = Width::new(8).or_fail()?;
    let height = Height::new(8).or_fail()?;
    let mut frame = Frame::zeroed(Dimensions::new(width, height)).or_fail()?;
    frame.fill(Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    });
    let half = Frac(9, 2).ratio()?;
    let at = Point {
        across: half,
        down: half,
    };
    let camera = identity_placement();
    let brush = Brush {
        camera: &camera,
        colour: fill,
        slot: Slot {
            frame: FrameIndex::new(0),
            object: ObjectIndex::new(0),
        },
    };
    draw_object(&mut frame, shape, at, brush).or_fail()?;
    Ok(frame)
}

#[test]
fn every_shape_draws_pixels() {
    assert_eq!(check_every_shape_draws_pixels(), Ok(()));
}

fn check_every_shape_draws_pixels() -> Result<(), Fail> {
    let green = Rgb8 {
        red: Channel(0),
        green: Channel(255),
        blue: Channel(0),
    };
    let black = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    };
    let one = Ratio::from_integer(Numerator(1));
    let two = Ratio::from_integer(Numerator(2));
    let half = Frac(9, 2).ratio()?;
    let centre = Point {
        across: half,
        down: half,
    };
    let shapes = [
        Shape::Disc { radius: two },
        Shape::Rect {
            half_width: two,
            half_height: one,
        },
        Shape::Polygon {
            vertices: vec![
                centre,
                Point {
                    across: one,
                    down: half,
                },
                Point {
                    across: half,
                    down: one,
                },
            ],
        },
        Shape::Cross {
            arm: two,
            thickness: one,
        },
    ];
    for shape in &shapes {
        let frame = render_shape(shape, green)?;
        let mut painted = false;
        for row in 0..8_u16 {
            for column in 0..8_u16 {
                if frame.pixel(PixelCoord::new(PixelColumn(column), PixelRow(row))) != Some(black) {
                    painted = true;
                }
            }
        }
        ensure!(painted, "every shape must paint at least one pixel");
    }
    Ok(())
}

#[test]
fn rect_matches_manual_polygon() {
    assert_eq!(check_rect_matches_manual_polygon(), Ok(()));
}

fn check_rect_matches_manual_polygon() -> Result<(), Fail> {
    let red = Rgb8 {
        red: Channel(200),
        green: Channel(30),
        blue: Channel(30),
    };
    let half_width = Ratio::from_integer(Numerator(2));
    let half_height = Ratio::from_integer(Numerator(1));
    let half = Frac(9, 2).ratio()?;
    let rect_frame = render_shape(
        &Shape::Rect {
            half_width,
            half_height,
        },
        red,
    )?;
    let left = half.checked_sub(half_width).or_fail()?;
    let right = half.checked_add(half_width).or_fail()?;
    let top = half.checked_sub(half_height).or_fail()?;
    let bottom = half.checked_add(half_height).or_fail()?;
    let corners = [
        Point {
            across: left,
            down: top,
        },
        Point {
            across: right,
            down: top,
        },
        Point {
            across: right,
            down: bottom,
        },
        Point {
            across: left,
            down: bottom,
        },
    ];
    let frame_width = Width::new(8).or_fail()?;
    let frame_height = Height::new(8).or_fail()?;
    let mut poly_frame = Frame::zeroed(Dimensions::new(frame_width, frame_height)).or_fail()?;
    poly_frame.fill(Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    });
    fill_polygon(&mut poly_frame, &corners, red).or_fail()?;
    ensure_eq!(
        rect_frame.data(),
        poly_frame.data(),
        "a rect must match the polygon through its absolute corners"
    );
    Ok(())
}
