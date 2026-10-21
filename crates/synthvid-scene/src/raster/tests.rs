use super::*;
use crate::color::Channel;
use crate::geom::{Affine, QuarterTurn, Vector};
use crate::testutil::black_8x8;
use crate::units::{Dimensions, Height, Width};
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

/// Full red, the fill most raster tests draw with.
const RED: Rgb8 = Rgb8 {
    red: Channel(255),
    green: Channel(0),
    blue: Channel(0),
};

/// Black, the colour of an untouched test frame.
const BLACK: Rgb8 = Rgb8 {
    red: Channel(0),
    green: Channel(0),
    blue: Channel(0),
};

/// The point whose two coordinates are both `coordinate`.
const fn on_diagonal(coordinate: Ratio) -> Point {
    Point {
        across: coordinate,
        down: coordinate,
    }
}

#[test]
fn raster_outside_draws_nothing() {
    assert_eq!(check_raster_outside_draws_nothing(), Ok(()));
}

fn check_raster_outside_draws_nothing() -> Result<(), Fail> {
    let mut frame = black_8x8()?;
    let before = Frame::zeroed(frame.dimensions()).or_fail()?;
    let red = RED;
    let hundred = Ratio::from_integer(Numerator(100));
    let small = Ratio::from_integer(Numerator(2));
    let unit = Ratio::from_integer(Numerator(1));
    let far = on_diagonal(hundred);
    fill_disc(&mut frame, far, small, red).or_fail()?;
    let past = Ratio::from_integer(Numerator(102));
    let near = on_diagonal(past);
    let far_poly = [
        far,
        Point {
            across: past,
            down: hundred,
        },
        near,
    ];
    fill_polygon(&mut frame, &far_poly, red).or_fail()?;
    fill_rect(
        &mut frame,
        Corners {
            min: far,
            max: near,
        },
        red,
    )
    .or_fail()?;
    draw_line(
        &mut frame,
        Segment {
            start: far,
            end: near,
        },
        red,
    )
    .or_fail()?;
    draw_cross(
        &mut frame,
        Cross {
            centre: far,
            arm: small,
            thickness: unit,
        },
        red,
    )
    .or_fail()?;
    ensure_eq!(
        frame.data(),
        before.data(),
        "shapes entirely outside must draw nothing"
    );
    Ok(())
}

#[test]
fn raster_straddling_edges_partial() {
    assert_eq!(check_raster_straddling_edges_partial(), Ok(()));
}

fn check_raster_straddling_edges_partial() -> Result<(), Fail> {
    let mut frame = black_8x8()?;
    let red = RED;
    let neg_two = Ratio::from_integer(Numerator(-2));
    let ten = Ratio::from_integer(Numerator(10));
    fill_rect_inner(
        &mut frame,
        Corners {
            min: on_diagonal(neg_two),
            max: on_diagonal(ten),
        },
        red,
    )
    .or_fail()?;
    for row in 0..8_u16 {
        for col in 0..8_u16 {
            ensure_eq!(
                frame.pixel(PixelCoord::new(PixelColumn(col), PixelRow(row))),
                Some(red),
                "oversized rect must fill every pixel"
            );
        }
    }
    check_raster_partial_rect()
}

/// Checks that a rectangle reaching past the top-left corner of an 8x8
/// frame paints only the pixels it covers.
fn check_raster_partial_rect() -> Result<(), Fail> {
    let red = RED;
    let black = BLACK;
    let neg_two = Ratio::from_integer(Numerator(-2));
    let mut partial = black_8x8()?;
    let three = Ratio::from_integer(Numerator(3));
    fill_rect_inner(
        &mut partial,
        Corners {
            min: on_diagonal(neg_two),
            max: on_diagonal(three),
        },
        red,
    )
    .or_fail()?;
    ensure_eq!(
        partial.pixel(PixelCoord::new(PixelColumn(0), PixelRow(0))),
        Some(red),
        "partial rect must cover the top-left pixel"
    );
    ensure_eq!(
        partial.pixel(PixelCoord::new(PixelColumn(2), PixelRow(2))),
        Some(red),
        "partial rect must cover pixel (2, 2)"
    );
    ensure_eq!(
        partial.pixel(PixelCoord::new(PixelColumn(3), PixelRow(3))),
        Some(black),
        "partial rect must not cover pixel (3, 3)"
    );
    ensure_eq!(
        partial.pixel(PixelCoord::new(PixelColumn(7), PixelRow(7))),
        Some(black),
        "partial rect must not cover the bottom-right pixel"
    );
    Ok(())
}

#[test]
fn polygon_rect_identical() {
    assert_eq!(check_polygon_rect_identical(), Ok(()));
}

fn check_polygon_rect_identical() -> Result<(), Fail> {
    let dims_w = Width::new(8).or_fail()?;
    let dims_h = Height::new(8).or_fail()?;
    let dims = Dimensions::new(dims_w, dims_h);
    let mut via_rect = Frame::zeroed(dims).or_fail()?;
    let mut via_poly = Frame::zeroed(dims).or_fail()?;
    let red = Rgb8 {
        red: Channel(10),
        green: Channel(200),
        blue: Channel(30),
    };
    let two = Ratio::from_integer(Numerator(2));
    let six = Ratio::from_integer(Numerator(6));
    let four = Ratio::from_integer(Numerator(4));
    let lower = on_diagonal(two);
    let upper = Point {
        across: six,
        down: four,
    };
    fill_rect(
        &mut via_rect,
        Corners {
            min: lower,
            max: upper,
        },
        red,
    )
    .or_fail()?;
    let corners = [
        lower,
        Point {
            across: six,
            down: two,
        },
        upper,
        Point {
            across: two,
            down: four,
        },
    ];
    let slice = &corners[0..4];
    fill_polygon(&mut via_poly, slice, red).or_fail()?;
    ensure_eq!(
        via_rect.data(),
        via_poly.data(),
        "rectangle and equivalent polygon must be byte-identical"
    );
    Ok(())
}

#[test]
fn quarter_turn_square_identical() {
    assert_eq!(check_quarter_turn_square_identical(), Ok(()));
}

fn check_quarter_turn_square_identical() -> Result<(), Fail> {
    let dims_w = Width::new(10).or_fail()?;
    let dims_h = Height::new(10).or_fail()?;
    let dims = Dimensions::new(dims_w, dims_h);
    let mut original = Frame::zeroed(dims).or_fail()?;
    let mut rotated = Frame::zeroed(dims).or_fail()?;
    let white = Rgb8 {
        red: Channel(255),
        green: Channel(255),
        blue: Channel(255),
    };
    let two = Ratio::from_integer(Numerator(2));
    let six = Ratio::from_integer(Numerator(6));
    let four = Ratio::from_integer(Numerator(4));
    let neg_four = Ratio::from_integer(Numerator(-4));
    let square = [
        on_diagonal(two),
        Point {
            across: six,
            down: two,
        },
        on_diagonal(six),
        Point {
            across: two,
            down: six,
        },
    ];
    let square_slice = &square[0..4];
    fill_polygon(&mut original, square_slice, white).or_fail()?;
    let to_origin = Affine::translation(Vector {
        across: neg_four,
        down: neg_four,
    });
    let spin = Affine::quarter_turns(QuarterTurn::One);
    let back_home = Affine::translation(Vector {
        across: four,
        down: four,
    });
    let first_leg = to_origin.then(spin).or_fail()?;
    let about_centre = first_leg.then(back_home).or_fail()?;
    let mut turned = [on_diagonal(two); 4];
    for (slot, corner) in turned.iter_mut().zip(square_slice) {
        *slot = about_centre.apply(*corner).or_fail()?;
    }
    let turned_slice = &turned[0..4];
    fill_polygon(&mut rotated, turned_slice, white).or_fail()?;
    ensure_eq!(
        original.data(),
        rotated.data(),
        "quarter-turned square must match the original exactly"
    );
    Ok(())
}

#[test]
fn line_and_cross_basic() {
    assert_eq!(check_line_and_cross_basic(), Ok(()));
}

fn check_line_and_cross_basic() -> Result<(), Fail> {
    let mut frame = black_8x8()?;
    let red = RED;
    let black = BLACK;
    let zero = Ratio::from_integer(Numerator(0));
    let seven = Ratio::from_integer(Numerator(7));
    let four = Ratio::from_integer(Numerator(4));
    let two = Ratio::from_integer(Numerator(2));
    let one = Ratio::from_integer(Numerator(1));
    draw_line(
        &mut frame,
        Segment {
            start: on_diagonal(zero),
            end: on_diagonal(seven),
        },
        red,
    )
    .or_fail()?;
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(0), PixelRow(0))),
        Some(red),
        "diagonal line must cover the start pixel"
    );
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(3), PixelRow(3))),
        Some(red),
        "diagonal line must cover an interior pixel"
    );
    let mut cross_frame = black_8x8()?;
    draw_cross(
        &mut cross_frame,
        Cross {
            centre: on_diagonal(four),
            arm: two,
            thickness: one,
        },
        red,
    )
    .or_fail()?;
    ensure_eq!(
        cross_frame.pixel(PixelCoord::new(PixelColumn(4), PixelRow(4))),
        Some(red),
        "cross must cover its centre"
    );
    ensure_eq!(
        cross_frame.pixel(PixelCoord::new(PixelColumn(0), PixelRow(0))),
        Some(black),
        "cross must not reach the corner"
    );
    Ok(())
}
