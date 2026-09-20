//! Rasterisation of geometry onto a frame.
//!
//! The `fill_*` and `draw_*` entry points decide pixel coverage by testing
//! exact pixel centres against exact geometry, and clip to the frame before
//! iterating, so a shape far outside the frame costs nothing. The predicates
//! that answer those questions live in the private `coverage` submodule.

mod coverage;
pub mod extent;

pub use extent::{cross_extent, disc_extent, line_extent, polygon_extent, rect_extent, Bounds};

use crate::color::Rgb8;
use crate::frame::Frame;
use crate::geom::Point;
use crate::ratio::{half_ratio, int_ratio, Ratio};
use coverage::{
    axis_range, clipped_pixel_range, disc_covers, line_covers, pixel_centre, point_in_polygon,
};

/// Fills the pixels whose centres fall inside the disc.
///
/// The disc is `{ p : |p - centre|^2 <= radius^2 }`, tested with exact
/// [`Ratio`] arithmetic. A non-positive radius draws nothing. The bounding
/// box `[centre - radius, centre + radius]` is clipped to the frame with
/// `clipped_pixel_range` before any pixel is visited.
pub fn fill_disc(frame: &mut Frame, centre: Point, radius: Ratio, colour: Rgb8) {
    if radius <= int_ratio(0) {
        return;
    }
    let Some(across) = axis_range(centre.x, radius) else {
        return;
    };
    let Some(down) = axis_range(centre.y, radius) else {
        return;
    };
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    let Some(columns) = clipped_pixel_range(across.0, across.1, width) else {
        return;
    };
    let Some(rows) = clipped_pixel_range(down.0, down.1, height) else {
        return;
    };
    let mut y = rows.0;
    loop {
        let mut x = columns.0;
        loop {
            let covered =
                pixel_centre(x, y).is_some_and(|sample| disc_covers(centre, radius, sample));
            if covered {
                let _ = frame.set_pixel(x, y, colour);
            }
            if x == columns.1 {
                break;
            }
            let Some(east) = x.checked_add(1) else {
                break;
            };
            x = east;
        }
        if y == rows.1 {
            break;
        }
        let Some(south) = y.checked_add(1) else {
            break;
        };
        y = south;
    }
}

/// Fills the pixels whose centres fall inside the polygon.
///
/// See `point_in_polygon` for the documented, exact edge rule: centres on
/// an edge count as inside, otherwise the even-odd rule with a half-open
/// vertex rule decides. Polygons with fewer than three vertices draw
/// nothing. The vertex bounding box is clipped to the frame before any
/// pixel is visited.
pub fn fill_polygon(frame: &mut Frame, vertices: &[Point], colour: Rgb8) {
    if vertices.len() < 3 {
        return;
    }
    let mut iter = vertices.iter();
    let Some(first) = iter.next() else {
        return;
    };
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
    let Some(columns) = clipped_pixel_range(across.0, across.1, width) else {
        return;
    };
    let Some(rows) = clipped_pixel_range(down.0, down.1, height) else {
        return;
    };
    let mut y = rows.0;
    loop {
        let mut x = columns.0;
        loop {
            let covered =
                pixel_centre(x, y).is_some_and(|sample| point_in_polygon(sample, vertices));
            if covered {
                let _ = frame.set_pixel(x, y, colour);
            }
            if x == columns.1 {
                break;
            }
            let Some(east) = x.checked_add(1) else {
                break;
            };
            x = east;
        }
        if y == rows.1 {
            break;
        }
        let Some(south) = y.checked_add(1) else {
            break;
        };
        y = south;
    }
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
pub fn fill_rect(frame: &mut Frame, min: Point, max: Point, colour: Rgb8) {
    if max.x < min.x {
        return;
    }
    if max.y < min.y {
        return;
    }
    let corners = [
        min,
        Point { x: max.x, y: min.y },
        max,
        Point { x: min.x, y: max.y },
    ];
    let Some(slice) = corners.get(0..4) else {
        return;
    };
    fill_polygon(frame, slice, colour);
}

/// Draws a 1-unit-thick line segment from `start` to `end`.
///
/// A pixel is drawn when its centre lies within `1 / 2` of the segment,
/// tested with exact [`Ratio`] arithmetic (see `line_covers`). A
/// zero-length segment draws the disc of radius `1 / 2` around the point.
/// The segment bounding box expanded by half a unit is clipped to the frame
/// before any pixel is visited.
pub fn draw_line(frame: &mut Frame, start: Point, end: Point, colour: Rgb8) {
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
    let Some(low_across) = across.0.checked_sub(half_ratio()) else {
        return;
    };
    across.0 = low_across;
    let Some(high_across) = across.1.checked_add(half_ratio()) else {
        return;
    };
    across.1 = high_across;
    let Some(low_down) = down.0.checked_sub(half_ratio()) else {
        return;
    };
    down.0 = low_down;
    let Some(high_down) = down.1.checked_add(half_ratio()) else {
        return;
    };
    down.1 = high_down;
    let width = frame.width().get().get();
    let height = frame.height().get().get();
    let Some(columns) = clipped_pixel_range(across.0, across.1, width) else {
        return;
    };
    let Some(rows) = clipped_pixel_range(down.0, down.1, height) else {
        return;
    };
    let mut y = rows.0;
    loop {
        let mut x = columns.0;
        loop {
            let covered = pixel_centre(x, y).is_some_and(|sample| line_covers(start, end, sample));
            if covered {
                let _ = frame.set_pixel(x, y, colour);
            }
            if x == columns.1 {
                break;
            }
            let Some(east) = x.checked_add(1) else {
                break;
            };
            x = east;
        }
        if y == rows.1 {
            break;
        }
        let Some(south) = y.checked_add(1) else {
            break;
        };
        y = south;
    }
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
pub fn draw_cross(frame: &mut Frame, centre: Point, arm: Ratio, thickness: Ratio, colour: Rgb8) {
    if arm < int_ratio(0) {
        return;
    }
    if thickness <= int_ratio(0) {
        return;
    }
    let Some(half_thick) = thickness.checked_div(int_ratio(2)) else {
        return;
    };
    {
        let Some(bar_left) = centre.x.checked_sub(arm) else {
            return;
        };
        let Some(bar_right) = centre.x.checked_add(arm) else {
            return;
        };
        let Some(bar_top) = centre.y.checked_sub(half_thick) else {
            return;
        };
        let Some(bar_bottom) = centre.y.checked_add(half_thick) else {
            return;
        };
        fill_rect(
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
        );
    }
    {
        let Some(bar_left) = centre.x.checked_sub(half_thick) else {
            return;
        };
        let Some(bar_right) = centre.x.checked_add(half_thick) else {
            return;
        };
        let Some(bar_top) = centre.y.checked_sub(arm) else {
            return;
        };
        let Some(bar_bottom) = centre.y.checked_add(arm) else {
            return;
        };
        fill_rect(
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
        );
    }
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
        let Some(mut frame) = black_8x8() else { return };
        let Some(before) = Frame::zeroed(frame.dimensions()) else {
            return;
        };
        let red = Rgb8::new(255, 0, 0);
        let Some(far_x) = make_ratio(100, 1) else {
            return;
        };
        let Some(far_y) = make_ratio(100, 1) else {
            return;
        };
        let Some(small) = make_ratio(2, 1) else {
            return;
        };
        let Some(unit) = make_ratio(1, 1) else { return };
        let far = Point::new(far_x, far_y);
        fill_disc(&mut frame, far, small, red);
        let Some(near_x) = make_ratio(102, 1) else {
            return;
        };
        let Some(near_y) = make_ratio(102, 1) else {
            return;
        };
        let far_poly = [far, Point::new(near_x, far_y), Point::new(near_x, near_y)];
        let Some(slice) = far_poly.get(0..3) else {
            return;
        };
        fill_polygon(&mut frame, slice, red);
        fill_rect(&mut frame, far, Point::new(near_x, near_y), red);
        draw_line(&mut frame, far, Point::new(near_x, near_y), red);
        draw_cross(&mut frame, far, small, unit, red);
        assert_eq!(
            frame.data(),
            before.data(),
            "shapes entirely outside must draw nothing"
        );
    }

    #[test]
    fn test_raster_straddling_edges_partial() {
        let Some(mut frame) = black_8x8() else { return };
        let red = Rgb8::new(255, 0, 0);
        let black = Rgb8::new(0, 0, 0);
        let Some(neg_two) = make_ratio(-2, 1) else {
            return;
        };
        let Some(ten) = make_ratio(10, 1) else { return };
        fill_rect(
            &mut frame,
            Point::new(neg_two, neg_two),
            Point::new(ten, ten),
            red,
        );
        for row in 0..8_u16 {
            for col in 0..8_u16 {
                assert_eq!(
                    frame.pixel(col, row),
                    Some(red),
                    "oversized rect must fill every pixel"
                );
            }
        }
        let Some(mut partial) = black_8x8() else {
            return;
        };
        let Some(three) = make_ratio(3, 1) else {
            return;
        };
        fill_rect(
            &mut partial,
            Point::new(neg_two, neg_two),
            Point::new(three, three),
            red,
        );
        assert_eq!(
            partial.pixel(0, 0),
            Some(red),
            "partial rect must cover the top-left pixel"
        );
        assert_eq!(
            partial.pixel(2, 2),
            Some(red),
            "partial rect must cover pixel (2, 2)"
        );
        assert_eq!(
            partial.pixel(3, 3),
            Some(black),
            "partial rect must not cover pixel (3, 3)"
        );
        assert_eq!(
            partial.pixel(7, 7),
            Some(black),
            "partial rect must not cover the bottom-right pixel"
        );
    }

    #[test]
    fn test_polygon_rect_identical() {
        let Some(dims_w) = Width::new(8) else { return };
        let Some(dims_h) = Height::new(8) else { return };
        let dims = Dimensions::new(dims_w, dims_h);
        let Some(mut via_rect) = Frame::zeroed(dims) else {
            return;
        };
        let Some(mut via_poly) = Frame::zeroed(dims) else {
            return;
        };
        let red = Rgb8::new(10, 200, 30);
        let Some(two) = make_ratio(2, 1) else { return };
        let Some(six) = make_ratio(6, 1) else { return };
        let Some(four) = make_ratio(4, 1) else { return };
        let lower = Point::new(two, two);
        let upper = Point::new(six, four);
        fill_rect(&mut via_rect, lower, upper, red);
        let corners = [lower, Point::new(six, two), upper, Point::new(two, four)];
        let Some(slice) = corners.get(0..4) else {
            return;
        };
        fill_polygon(&mut via_poly, slice, red);
        assert_eq!(
            via_rect.data(),
            via_poly.data(),
            "rectangle and equivalent polygon must be byte-identical"
        );
    }

    #[test]
    fn test_quarter_turn_square_identical() {
        let Some(dims_w) = Width::new(10) else { return };
        let Some(dims_h) = Height::new(10) else {
            return;
        };
        let dims = Dimensions::new(dims_w, dims_h);
        let Some(mut original) = Frame::zeroed(dims) else {
            return;
        };
        let Some(mut rotated) = Frame::zeroed(dims) else {
            return;
        };
        let white = Rgb8::new(255, 255, 255);
        let Some(two) = make_ratio(2, 1) else { return };
        let Some(six) = make_ratio(6, 1) else { return };
        let Some(four) = make_ratio(4, 1) else { return };
        let Some(neg_four) = make_ratio(-4, 1) else {
            return;
        };
        let square = [
            Point::new(two, two),
            Point::new(six, two),
            Point::new(six, six),
            Point::new(two, six),
        ];
        let Some(square_slice) = square.get(0..4) else {
            return;
        };
        fill_polygon(&mut original, square_slice, white);
        let to_origin = Affine::translation(Vector::new(neg_four, neg_four));
        let spin = Affine::quarter_turns(1);
        let back_home = Affine::translation(Vector::new(four, four));
        let Some(first_leg) = to_origin.then(spin) else {
            return;
        };
        let Some(about_centre) = first_leg.then(back_home) else {
            return;
        };
        let mut turned = [Point::new(two, two); 4];
        let mut idx: usize = 0;
        for corner in square_slice {
            let Some(mapped) = about_centre.apply(*corner) else {
                return;
            };
            let Some(slot) = turned.get_mut(idx) else {
                return;
            };
            *slot = mapped;
            idx = idx.wrapping_add(1);
        }
        let Some(turned_slice) = turned.get(0..4) else {
            return;
        };
        fill_polygon(&mut rotated, turned_slice, white);
        assert_eq!(
            original.data(),
            rotated.data(),
            "quarter-turned square must match the original exactly"
        );
    }

    #[test]
    fn test_line_and_cross_basic() {
        let Some(mut frame) = black_8x8() else { return };
        let red = Rgb8::new(255, 0, 0);
        let black = Rgb8::new(0, 0, 0);
        let Some(zero) = make_ratio(0, 1) else { return };
        let Some(seven) = make_ratio(7, 1) else {
            return;
        };
        let Some(four) = make_ratio(4, 1) else { return };
        let Some(two) = make_ratio(2, 1) else { return };
        let Some(one) = make_ratio(1, 1) else { return };
        draw_line(
            &mut frame,
            Point::new(zero, zero),
            Point::new(seven, seven),
            red,
        );
        assert_eq!(
            frame.pixel(0, 0),
            Some(red),
            "diagonal line must cover the start pixel"
        );
        assert_eq!(
            frame.pixel(3, 3),
            Some(red),
            "diagonal line must cover an interior pixel"
        );
        let Some(mut cross_frame) = black_8x8() else {
            return;
        };
        draw_cross(&mut cross_frame, Point::new(four, four), two, one, red);
        assert_eq!(
            cross_frame.pixel(4, 4),
            Some(red),
            "cross must cover its centre"
        );
        assert_eq!(
            cross_frame.pixel(0, 0),
            Some(black),
            "cross must not reach the corner"
        );
    }
}
