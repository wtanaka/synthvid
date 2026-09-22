//! Rasterisation of geometry onto a frame.
//!
//! The `fill_*` and `draw_*` entry points decide pixel coverage by testing
//! exact pixel centres against exact geometry, and clip to the frame before
//! iterating, so a shape far outside the frame costs nothing. The predicates
//! that answer those questions live in the private `coverage` submodule.

mod coverage;
pub mod extent;

pub use extent::{
    cross_extent, disc_extent, line_extent, polygon_extent, rect_extent, Bounds, BoundsError,
    BoundsOrEmpty,
};

use crate::color::Rgb8;
use crate::frame::{Frame, PixelCoord};
use crate::geom::Point;
use crate::ratio::{half_ratio, int_ratio, Overflow, Ratio};
use coverage::{
    axis_range, clipped_pixel_range, disc_covers, line_covers, pixel_centre, point_in_polygon,
    PixelSpan,
};

/// Fills the pixels whose centres fall inside the disc.
///
/// The disc is `{ p : |p - centre|^2 <= radius^2 }`, tested with exact
/// [`Ratio`] arithmetic. A non-positive radius draws nothing. The bounding
/// box `[centre - radius, centre + radius]` is clipped to the frame with
/// `clipped_pixel_range` before any pixel is visited.
fn fill_disc_inner(
    frame: &mut Frame,
    centre: Point,
    radius: Ratio,
    colour: Rgb8,
) -> Result<(), Overflow> {
    if radius <= int_ratio(0) {
        return Ok(());
    }
    let across = axis_range(centre.x, radius)?;
    let down = axis_range(centre.y, radius)?;
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    let PixelSpan::Covering {
        first: columns_first,
        last: columns_last,
    } = clipped_pixel_range(across.0, across.1, width)?
    else {
        return Ok(());
    };
    let PixelSpan::Covering {
        first: rows_first,
        last: rows_last,
    } = clipped_pixel_range(down.0, down.1, height)?
    else {
        return Ok(());
    };
    let mut y = rows_first;
    loop {
        let mut x = columns_first;
        loop {
            let covered = disc_covers(centre, radius, pixel_centre(x, y))?;
            if covered {
                frame
                    .set_pixel(PixelCoord::new(x, y), colour)
                    .map_err(|_| Overflow)?;
            }
            if x == columns_last {
                break;
            }
            let Some(east) = x.checked_add(1) else {
                break;
            };
            x = east;
        }
        if y == rows_last {
            break;
        }
        let Some(south) = y.checked_add(1) else {
            break;
        };
        y = south;
    }
    Ok(())
}

/// Fills the pixels whose centres fall inside the polygon.
///
/// See `point_in_polygon` for the documented, exact edge rule: centres on
/// an edge count as inside, otherwise the even-odd rule with a half-open
/// vertex rule decides. Polygons with fewer than three vertices draw
/// nothing. The vertex bounding box is clipped to the frame before any
/// pixel is visited.
fn fill_polygon_inner(frame: &mut Frame, vertices: &[Point], colour: Rgb8) -> Result<(), Overflow> {
    if vertices.len() < 3 {
        return Ok(());
    }
    let mut iter = vertices.iter();
    let first = iter.next().ok_or(Overflow)?;
    let mut across = (first.x, first.x);
    let mut down = (first.y, first.y);
    for vertex in iter {
        if vertex.x < across.0 {
            across.0 = vertex.x;
        }
        if across.1 < vertex.x {
            across.1 = vertex.x;
        }
        if vertex.y < down.0 {
            down.0 = vertex.y;
        }
        if down.1 < vertex.y {
            down.1 = vertex.y;
        }
    }
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    let PixelSpan::Covering {
        first: columns_first,
        last: columns_last,
    } = clipped_pixel_range(across.0, across.1, width)?
    else {
        return Ok(());
    };
    let PixelSpan::Covering {
        first: rows_first,
        last: rows_last,
    } = clipped_pixel_range(down.0, down.1, height)?
    else {
        return Ok(());
    };
    let mut y = rows_first;
    loop {
        let mut x = columns_first;
        loop {
            let covered = point_in_polygon(pixel_centre(x, y), vertices)?;
            if covered {
                frame
                    .set_pixel(PixelCoord::new(x, y), colour)
                    .map_err(|_| Overflow)?;
            }
            if x == columns_last {
                break;
            }
            let Some(east) = x.checked_add(1) else {
                break;
            };
            x = east;
        }
        if y == rows_last {
            break;
        }
        let Some(south) = y.checked_add(1) else {
            break;
        };
        y = south;
    }
    Ok(())
}

/// Fills the axis-aligned rectangle from `min` to `max`.
///
/// The rectangle is the set `{ p : min.x <= p.x <= max.x,
/// min.y <= p.y <= max.y }`, using the same inclusive edge rule as
/// [`fill_polygon`]. When `max` lies strictly below `min` on either axis the
/// rectangle is empty and nothing is drawn. The implementation fills through
/// the identical polygon path with corners `min`, `(max.x, min.y)`, `max`,
/// `(min.x, max.y)`, so a rectangle and its equivalent four-vertex polygon
/// always produce byte-identical frames.
fn fill_rect_inner(
    frame: &mut Frame,
    min: Point,
    max: Point,
    colour: Rgb8,
) -> Result<(), Overflow> {
    if max.x < min.x {
        return Ok(());
    }
    if max.y < min.y {
        return Ok(());
    }
    let corners = [
        min,
        Point { x: max.x, y: min.y },
        max,
        Point { x: min.x, y: max.y },
    ];
    fill_polygon_inner(frame, &corners, colour)
}

/// Draws a 1-unit-thick line segment from `start` to `end`.
///
/// A pixel is drawn when its centre lies within `1 / 2` of the segment,
/// tested with exact [`Ratio`] arithmetic (see `line_covers`). A
/// zero-length segment draws the disc of radius `1 / 2` around the point.
/// The segment bounding box expanded by half a unit is clipped to the frame
/// before any pixel is visited.
fn draw_line_inner(
    frame: &mut Frame,
    start: Point,
    end: Point,
    colour: Rgb8,
) -> Result<(), Overflow> {
    let mut across = if start.x < end.x {
        (start.x, end.x)
    } else {
        (end.x, start.x)
    };
    let mut down = if start.y < end.y {
        (start.y, end.y)
    } else {
        (end.y, start.y)
    };
    let low_across = across.0.checked_sub(half_ratio())?;
    across.0 = low_across;
    let high_across = across.1.checked_add(half_ratio())?;
    across.1 = high_across;
    let low_down = down.0.checked_sub(half_ratio())?;
    down.0 = low_down;
    let high_down = down.1.checked_add(half_ratio())?;
    down.1 = high_down;
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    let PixelSpan::Covering {
        first: columns_first,
        last: columns_last,
    } = clipped_pixel_range(across.0, across.1, width)?
    else {
        return Ok(());
    };
    let PixelSpan::Covering {
        first: rows_first,
        last: rows_last,
    } = clipped_pixel_range(down.0, down.1, height)?
    else {
        return Ok(());
    };
    let mut y = rows_first;
    loop {
        let mut x = columns_first;
        loop {
            let covered = line_covers(start, end, pixel_centre(x, y))?;
            if covered {
                frame
                    .set_pixel(PixelCoord::new(x, y), colour)
                    .map_err(|_| Overflow)?;
            }
            if x == columns_last {
                break;
            }
            let Some(east) = x.checked_add(1) else {
                break;
            };
            x = east;
        }
        if y == rows_last {
            break;
        }
        let Some(south) = y.checked_add(1) else {
            break;
        };
        y = south;
    }
    Ok(())
}

/// Draws an axis-aligned cross centred at `centre`.
///
/// `arm` is the half-length from the centre to the tip of each bar along its
/// axis; `thickness` is the full width of each bar. The cross is the union of
/// the horizontal bar `[cx - arm, cx + arm]` by
/// `[cy - thickness / 2, cy + thickness / 2]` and the vertical bar
/// `[cx - thickness / 2, cx + thickness / 2]` by `[cy - arm, cy + arm]`,
/// each filled with the inclusive rectangle rule of [`fill_rect`]. A
/// negative `arm` or a non-positive `thickness` draws nothing.
fn draw_cross_inner(
    frame: &mut Frame,
    centre: Point,
    arm: Ratio,
    thickness: Ratio,
    colour: Rgb8,
) -> Result<(), Overflow> {
    if arm < int_ratio(0) {
        return Ok(());
    }
    if thickness <= int_ratio(0) {
        return Ok(());
    }
    let half_thick = thickness.checked_div(int_ratio(2))?;
    {
        let bar_left = centre.x.checked_sub(arm)?;
        let bar_right = centre.x.checked_add(arm)?;
        let bar_top = centre.y.checked_sub(half_thick)?;
        let bar_bottom = centre.y.checked_add(half_thick)?;
        fill_rect_inner(
            frame,
            Point {
                x: bar_left,
                y: bar_top,
            },
            Point {
                x: bar_right,
                y: bar_bottom,
            },
            colour,
        )?;
    }
    {
        let bar_left = centre.x.checked_sub(half_thick)?;
        let bar_right = centre.x.checked_add(half_thick)?;
        let bar_top = centre.y.checked_sub(arm)?;
        let bar_bottom = centre.y.checked_add(arm)?;
        fill_rect_inner(
            frame,
            Point {
                x: bar_left,
                y: bar_top,
            },
            Point {
                x: bar_right,
                y: bar_bottom,
            },
            colour,
        )?;
    }
    Ok(())
}

/// Fills the pixels whose centres fall inside the disc.
///
/// A non-positive radius, or a disc entirely off screen, draws nothing and
/// returns `Ok`. Returns `Err(Overflow)` when the bounding box cannot be
/// computed in exact arithmetic.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the extent overflows exact arithmetic.
pub fn fill_disc(
    frame: &mut Frame,
    centre: Point,
    radius: Ratio,
    colour: Rgb8,
) -> Result<(), Overflow> {
    fill_disc_inner(frame, centre, radius, colour)
}

/// Fills the pixels whose centres fall inside the polygon.
///
/// Fewer than three vertices, or a polygon entirely off screen, draws nothing
/// and returns `Ok`.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the extent overflows exact arithmetic.
pub fn fill_polygon(frame: &mut Frame, vertices: &[Point], colour: Rgb8) -> Result<(), Overflow> {
    fill_polygon_inner(frame, vertices, colour)
}

/// Fills the axis-aligned rectangle from `min` to `max`.
///
/// An inverted or off-screen rectangle draws nothing and returns `Ok`.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the extent overflows exact arithmetic.
pub fn fill_rect(frame: &mut Frame, min: Point, max: Point, colour: Rgb8) -> Result<(), Overflow> {
    fill_rect_inner(frame, min, max, colour)
}

/// Draws a 1-unit-thick line segment from `start` to `end`.
///
/// A segment entirely off screen draws nothing and returns `Ok`.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the extent overflows exact arithmetic.
pub fn draw_line(
    frame: &mut Frame,
    start: Point,
    end: Point,
    colour: Rgb8,
) -> Result<(), Overflow> {
    draw_line_inner(frame, start, end, colour)
}

/// Draws an axis-aligned cross centred at `centre`.
///
/// A negative `arm` or a non-positive `thickness` draws nothing and returns
/// `Ok`.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the extent overflows exact arithmetic.
pub fn draw_cross(
    frame: &mut Frame,
    centre: Point,
    arm: Ratio,
    thickness: Ratio,
    colour: Rgb8,
) -> Result<(), Overflow> {
    draw_cross_inner(frame, centre, arm, thickness, colour)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgb8;
    use crate::frame::Frame;
    use crate::geom::{Affine, Point, Vector};
    use crate::testutil::{black_8x8, make_ratio};
    use crate::units::{Dimensions, Height, Width};

    #[test]
    fn test_raster_outside_draws_nothing() {
        let mut frame = black_8x8().unwrap();
        let before = Frame::zeroed(frame.dimensions()).unwrap();
        let red = Rgb8::new(255, 0, 0);
        let far_x = make_ratio(100, 1).unwrap();
        let far_y = make_ratio(100, 1).unwrap();
        let small = make_ratio(2, 1).unwrap();
        let unit = make_ratio(1, 1).unwrap();
        let far = Point::new(far_x, far_y);
        fill_disc(&mut frame, far, small, red).expect("drawing a test fixture must not overflow");
        let near_x = make_ratio(102, 1).unwrap();
        let near_y = make_ratio(102, 1).unwrap();
        let far_poly = [far, Point::new(near_x, far_y), Point::new(near_x, near_y)];
        let slice = &far_poly[0..3];
        fill_polygon(&mut frame, slice, red).expect("drawing a test fixture must not overflow");
        fill_rect(&mut frame, far, Point::new(near_x, near_y), red)
            .expect("drawing a test fixture must not overflow");
        draw_line(&mut frame, far, Point::new(near_x, near_y), red)
            .expect("drawing a test fixture must not overflow");
        draw_cross(&mut frame, far, small, unit, red)
            .expect("drawing a test fixture must not overflow");
        assert_eq!(
            frame.data(),
            before.data(),
            "shapes entirely outside must draw nothing"
        );
    }

    #[test]
    fn test_raster_straddling_edges_partial() {
        let mut frame = black_8x8().unwrap();
        let red = Rgb8::new(255, 0, 0);
        let black = Rgb8::new(0, 0, 0);
        let neg_two = make_ratio(-2, 1).unwrap();
        let ten = make_ratio(10, 1).unwrap();
        fill_rect_inner(
            &mut frame,
            Point::new(neg_two, neg_two),
            Point::new(ten, ten),
            red,
        )
        .expect("drawing a test fixture must not overflow");
        for row in 0..8_u16 {
            for col in 0..8_u16 {
                assert_eq!(
                    frame.pixel(PixelCoord::new(col, row)),
                    Some(red),
                    "oversized rect must fill every pixel"
                );
            }
        }
        let mut partial = black_8x8().unwrap();
        let three = make_ratio(3, 1).unwrap();
        fill_rect_inner(
            &mut partial,
            Point::new(neg_two, neg_two),
            Point::new(three, three),
            red,
        )
        .expect("drawing a test fixture must not overflow");
        assert_eq!(
            partial.pixel(PixelCoord::new(0, 0)),
            Some(red),
            "partial rect must cover the top-left pixel"
        );
        assert_eq!(
            partial.pixel(PixelCoord::new(2, 2)),
            Some(red),
            "partial rect must cover pixel (2, 2)"
        );
        assert_eq!(
            partial.pixel(PixelCoord::new(3, 3)),
            Some(black),
            "partial rect must not cover pixel (3, 3)"
        );
        assert_eq!(
            partial.pixel(PixelCoord::new(7, 7)),
            Some(black),
            "partial rect must not cover the bottom-right pixel"
        );
    }

    #[test]
    fn test_polygon_rect_identical() {
        let dims_w = Width::new(8).unwrap();
        let dims_h = Height::new(8).unwrap();
        let dims = Dimensions::new(dims_w, dims_h);
        let mut via_rect = Frame::zeroed(dims).unwrap();
        let mut via_poly = Frame::zeroed(dims).unwrap();
        let red = Rgb8::new(10, 200, 30);
        let two = make_ratio(2, 1).unwrap();
        let six = make_ratio(6, 1).unwrap();
        let four = make_ratio(4, 1).unwrap();
        let lower = Point::new(two, two);
        let upper = Point::new(six, four);
        fill_rect(&mut via_rect, lower, upper, red)
            .expect("drawing a test fixture must not overflow");
        let corners = [lower, Point::new(six, two), upper, Point::new(two, four)];
        let slice = &corners[0..4];
        fill_polygon(&mut via_poly, slice, red).expect("drawing a test fixture must not overflow");
        assert_eq!(
            via_rect.data(),
            via_poly.data(),
            "rectangle and equivalent polygon must be byte-identical"
        );
    }

    #[test]
    fn test_quarter_turn_square_identical() {
        let dims_w = Width::new(10).unwrap();
        let dims_h = Height::new(10).unwrap();
        let dims = Dimensions::new(dims_w, dims_h);
        let mut original = Frame::zeroed(dims).unwrap();
        let mut rotated = Frame::zeroed(dims).unwrap();
        let white = Rgb8::new(255, 255, 255);
        let two = make_ratio(2, 1).unwrap();
        let six = make_ratio(6, 1).unwrap();
        let four = make_ratio(4, 1).unwrap();
        let neg_four = make_ratio(-4, 1).unwrap();
        let square = [
            Point::new(two, two),
            Point::new(six, two),
            Point::new(six, six),
            Point::new(two, six),
        ];
        let square_slice = &square[0..4];
        fill_polygon(&mut original, square_slice, white)
            .expect("drawing a test fixture must not overflow");
        let to_origin = Affine::translation(Vector::new(neg_four, neg_four));
        let spin = Affine::quarter_turns(1);
        let back_home = Affine::translation(Vector::new(four, four));
        let first_leg = to_origin.then(spin).unwrap();
        let about_centre = first_leg.then(back_home).unwrap();
        let mut turned = [Point::new(two, two); 4];
        let mut idx: usize = 0;
        for corner in square_slice {
            let mapped = about_centre.apply(*corner).unwrap();
            let slot = turned.get_mut(idx).unwrap();
            *slot = mapped;
            idx = idx.wrapping_add(1);
        }
        let turned_slice = &turned[0..4];
        fill_polygon(&mut rotated, turned_slice, white)
            .expect("drawing a test fixture must not overflow");
        assert_eq!(
            original.data(),
            rotated.data(),
            "quarter-turned square must match the original exactly"
        );
    }

    #[test]
    fn test_line_and_cross_basic() {
        let mut frame = black_8x8().unwrap();
        let red = Rgb8::new(255, 0, 0);
        let black = Rgb8::new(0, 0, 0);
        let zero = make_ratio(0, 1).unwrap();
        let seven = make_ratio(7, 1).unwrap();
        let four = make_ratio(4, 1).unwrap();
        let two = make_ratio(2, 1).unwrap();
        let one = make_ratio(1, 1).unwrap();
        draw_line(
            &mut frame,
            Point::new(zero, zero),
            Point::new(seven, seven),
            red,
        )
        .expect("drawing a test fixture must not overflow");
        assert_eq!(
            frame.pixel(PixelCoord::new(0, 0)),
            Some(red),
            "diagonal line must cover the start pixel"
        );
        assert_eq!(
            frame.pixel(PixelCoord::new(3, 3)),
            Some(red),
            "diagonal line must cover an interior pixel"
        );
        let mut cross_frame = black_8x8().unwrap();
        draw_cross(&mut cross_frame, Point::new(four, four), two, one, red)
            .expect("drawing a test fixture must not overflow");
        assert_eq!(
            cross_frame.pixel(PixelCoord::new(4, 4)),
            Some(red),
            "cross must cover its centre"
        );
        assert_eq!(
            cross_frame.pixel(PixelCoord::new(0, 0)),
            Some(black),
            "cross must not reach the corner"
        );
    }
}
