//! Shape drawing for frame rendering.
//!
//! [`draw_object`] draws one [`Shape`] centred on a world-space position
//! through the camera placement of the parent module. Disc centres pass
//! through the camera transform and radii scale by the magnification;
//! rectangles, polygon vertices, and cross bars map corner by corner, so
//! camera rotation turns them into rotated polygons rather than being
//! ignored. Anything that overflows skips the shape, leaving earlier pixels
//! untouched. All arithmetic is exact [`Ratio`] arithmetic.

use crate::color::Rgb8;
use crate::frame::Frame;
use crate::geom::Point;
use crate::raster::{fill_disc, fill_polygon};
use crate::ratio::{int_ratio, Ratio};
use crate::scene::Shape;

use super::CameraFrame;

/// Returns the four corners of an axis-aligned rectangle.
///
/// The corners run clockwise from the top-left `(left, top)`. Returns `None`
/// on overflow; the caller then draws nothing.
fn rect_corners(centre: Point, half_width: Ratio, half_height: Ratio) -> Option<[Point; 4]> {
    let left = centre.x.checked_sub(half_width)?;
    let right = centre.x.checked_add(half_width)?;
    let top = centre.y.checked_sub(half_height)?;
    let bottom = centre.y.checked_add(half_height)?;
    Some([
        Point::new(left, top),
        Point::new(right, top),
        Point::new(right, bottom),
        Point::new(left, bottom),
    ])
}

/// Fills the polygon through `world` after mapping each vertex to the screen.
///
/// A vertex whose camera transform overflows skips the whole shape, leaving
/// earlier pixels untouched.
fn draw_mapped_polygon(frame: &mut Frame, world: &[Point], camera: &CameraFrame, colour: Rgb8) {
    let mut mapped = Vec::with_capacity(world.len());
    for vertex in world {
        let Some(screen) = camera.transform.apply(*vertex) else {
            return;
        };
        mapped.push(screen);
    }
    fill_polygon(frame, &mapped, colour);
}

/// Shifts polygon vertices from shape space to world space.
///
/// Shape vertices are offsets from the object position, so each world vertex
/// is `at + vertex`. Returns `None` on overflow; the caller then draws
/// nothing.
fn shifted_vertices(at: Point, vertices: &[Point]) -> Option<Vec<Point>> {
    let mut world = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        let x = at.x.checked_add(vertex.x)?;
        let y = at.y.checked_add(vertex.y)?;
        world.push(Point::new(x, y));
    }
    Some(world)
}

/// Draws a cross shape through the camera transform.
///
/// The cross is the union of its horizontal and vertical bars, each mapped
/// corner by corner so that camera rotation turns the bars into rotated
/// rectangles rather than being ignored. With the identity camera the corners
/// match the untransformed bars exactly. A negative arm or a non-positive
/// thickness draws nothing.
fn draw_cross_shape(
    frame: &mut Frame,
    at: Point,
    arm: Ratio,
    thickness: Ratio,
    camera: &CameraFrame,
    colour: Rgb8,
) {
    if arm < int_ratio(0) {
        return;
    }
    if thickness <= int_ratio(0) {
        return;
    }
    let Some(half) = thickness.checked_div(int_ratio(2)) else {
        return;
    };
    let Some(flat) = rect_corners(at, arm, half) else {
        return;
    };
    draw_mapped_polygon(frame, &flat, camera, colour);
    let Some(tall) = rect_corners(at, half, arm) else {
        return;
    };
    draw_mapped_polygon(frame, &tall, camera, colour);
}

/// Draws one shape centred on `at` through the camera placement.
///
/// `at` is the object position in world space. Disc radii scale by the camera
/// magnification; everything else maps corner by corner. Anything that
/// overflows skips the shape; the frame keeps whatever earlier shapes drew.
pub(super) fn draw_object(
    frame: &mut Frame,
    shape: &Shape,
    at: Point,
    camera: &CameraFrame,
    colour: Rgb8,
) {
    match shape {
        Shape::Disc { radius } => {
            let Some(centre) = camera.transform.apply(at) else {
                return;
            };
            let Some(scaled) = radius.checked_mul(camera.zoom) else {
                return;
            };
            fill_disc(frame, centre, scaled, colour);
        }
        Shape::Rect {
            half_width,
            half_height,
        } => {
            let Some(corners) = rect_corners(at, *half_width, *half_height) else {
                return;
            };
            draw_mapped_polygon(frame, &corners, camera, colour);
        }
        Shape::Polygon { vertices } => {
            let Some(world) = shifted_vertices(at, vertices) else {
                return;
            };
            draw_mapped_polygon(frame, &world, camera, colour);
        }
        Shape::Cross { arm, thickness } => {
            draw_cross_shape(frame, at, *arm, *thickness, camera, colour);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Affine;
    use crate::testutil::make_ratio;
    use crate::units::{Dimensions, Height, Width};

    /// Builds the identity camera placement used for shape tests.
    fn identity_placement() -> CameraFrame {
        CameraFrame {
            transform: Affine::identity(),
            zoom: int_ratio(1),
        }
    }

    /// Draws one shape centred at `(9 / 2, 9 / 2)` on an 8x8 black frame.
    fn render_shape(shape: &Shape, fill: Rgb8) -> Option<Frame> {
        let width = Width::new(8)?;
        let height = Height::new(8)?;
        let mut frame = Frame::zeroed(Dimensions::new(width, height))?;
        frame.fill(Rgb8::new(0, 0, 0));
        let half = make_ratio(9, 2)?;
        let at = Point::new(half, half);
        let camera = identity_placement();
        draw_object(&mut frame, shape, at, &camera, fill);
        Some(frame)
    }

    #[test]
    fn test_every_shape_draws_pixels() {
        let green = Rgb8::new(0, 255, 0);
        let black = Rgb8::new(0, 0, 0);
        let Some(one) = make_ratio(1, 1) else { return };
        let Some(two) = make_ratio(2, 1) else { return };
        let Some(half) = make_ratio(9, 2) else { return };
        let centre = Point::new(half, half);
        let shapes = [
            Shape::Disc { radius: two },
            Shape::Rect {
                half_width: two,
                half_height: one,
            },
            Shape::Polygon {
                vertices: vec![centre, Point::new(one, half), Point::new(half, one)],
            },
            Shape::Cross {
                arm: two,
                thickness: one,
            },
        ];
        for shape in &shapes {
            let Some(frame) = render_shape(shape, green) else {
                return;
            };
            let mut painted = false;
            for y in 0..8_u16 {
                for x in 0..8_u16 {
                    if frame.pixel(x, y) != Some(black) {
                        painted = true;
                    }
                }
            }
            assert!(painted, "every shape must paint at least one pixel");
        }
    }

    #[test]
    fn test_rect_matches_manual_polygon() {
        let red = Rgb8::new(200, 30, 30);
        let Some(half_width) = make_ratio(2, 1) else {
            return;
        };
        let Some(half_height) = make_ratio(1, 1) else {
            return;
        };
        let Some(half) = make_ratio(9, 2) else { return };
        let Some(rect_frame) = render_shape(
            &Shape::Rect {
                half_width,
                half_height,
            },
            red,
        ) else {
            return;
        };
        let Some(left) = half.checked_sub(half_width) else {
            return;
        };
        let Some(right) = half.checked_add(half_width) else {
            return;
        };
        let Some(top) = half.checked_sub(half_height) else {
            return;
        };
        let Some(bottom) = half.checked_add(half_height) else {
            return;
        };
        let corners = [
            Point::new(left, top),
            Point::new(right, top),
            Point::new(right, bottom),
            Point::new(left, bottom),
        ];
        let width = Width::new(8);
        let height = Height::new(8);
        let (Some(w), Some(h)) = (width, height) else {
            return;
        };
        let Some(mut poly_frame) = Frame::zeroed(Dimensions::new(w, h)) else {
            return;
        };
        poly_frame.fill(Rgb8::new(0, 0, 0));
        fill_polygon(&mut poly_frame, &corners, red);
        assert_eq!(
            rect_frame.data(),
            poly_frame.data(),
            "a rect must match the polygon through its absolute corners"
        );
    }
}
