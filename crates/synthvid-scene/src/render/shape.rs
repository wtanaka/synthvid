//! Shape drawing for frame rendering.
//!
//! [`draw_object`] draws one [`Shape`] centred on a scene-space position
//! through the camera placement of the parent module. Disc centres pass
//! through the camera transform and radii scale by the magnification;
//! rectangles, polygon vertices, and cross bars map corner by corner, so
//! camera rotation turns them into rotated polygons rather than being
//! ignored. A placement that overflows exact arithmetic is an error; see
//! [`RenderError`](super::RenderError). All arithmetic is exact [`Ratio`]
//! arithmetic.

use crate::color::Rgb8;
use crate::frame::Frame;
use crate::geom::Point;
use crate::raster::{fill_disc, fill_polygon};
use crate::ratio::{int_ratio, Overflow, Ratio};
use crate::scene::Shape;
use crate::units::{FrameIndex, ObjectIndex};

use super::{CameraFrame, RenderError};

/// Returns the four corners of an axis-aligned rectangle.
///
/// The corners run clockwise from the top-left `(left, top)`. Returns `Err(Overflow)`
/// on overflow; the caller reports [`RenderError::ObjectOverflow`].
fn rect_corners(
    centre: Point,
    half_width: Ratio,
    half_height: Ratio,
) -> Result<[Point; 4], Overflow> {
    let left = centre.x.checked_sub(half_width)?;
    let right = centre.x.checked_add(half_width)?;
    let top = centre.y.checked_sub(half_height)?;
    let bottom = centre.y.checked_add(half_height)?;
    Ok([
        Point::new(left, top),
        Point::new(right, top),
        Point::new(right, bottom),
        Point::new(left, bottom),
    ])
}

/// Fills the polygon through `scene` after mapping each vertex to the screen.
///
/// # Errors
///
/// Returns [`RenderError::ObjectOverflow`] when a vertex camera transform
/// overflows exact arithmetic.
fn draw_mapped_polygon(
    frame: &mut Frame,
    scene: &[Point],
    camera: &CameraFrame,
    colour: Rgb8,
    frame_index: FrameIndex,
    object: ObjectIndex,
) -> Result<(), RenderError> {
    let mut mapped = Vec::with_capacity(scene.len());
    for vertex in scene {
        let Ok(screen) = camera.transform.apply(*vertex) else {
            return Err(RenderError::ObjectOverflow {
                frame: frame_index,
                object,
            });
        };
        mapped.push(screen);
    }
    fill_polygon(frame, &mapped, colour).map_err(|_| RenderError::ObjectOverflow {
        frame: frame_index,
        object,
    })?;
    Ok(())
}

/// Shifts polygon vertices from shape space to scene space.
///
/// Shape vertices are offsets from the object position, so each scene vertex
/// is `at + vertex`. Returns `Err(Overflow)` on overflow; the caller reports
/// [`RenderError::ObjectOverflow`].
fn shifted_vertices(at: Point, vertices: &[Point]) -> Result<Vec<Point>, Overflow> {
    let mut scene = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        let x = at.x.checked_add(vertex.x)?;
        let y = at.y.checked_add(vertex.y)?;
        scene.push(Point::new(x, y));
    }
    Ok(scene)
}

/// Draws a cross shape through the camera transform.
///
/// The cross is the union of its horizontal and vertical bars, each mapped
/// corner by corner so that camera rotation turns the bars into rotated
/// rectangles rather than being ignored. With the identity camera the corners
/// match the untransformed bars exactly. A negative arm or a non-positive
/// thickness draws nothing.
///
/// # Errors
///
/// Returns [`RenderError::ObjectOverflow`] when any intermediate value
/// overflows exact arithmetic.
fn draw_cross_shape(
    frame: &mut Frame,
    at: Point,
    camera: &CameraFrame,
    colour: Rgb8,
    frame_index: FrameIndex,
    object: ObjectIndex,
    geometry: (Ratio, Ratio),
) -> Result<(), RenderError> {
    let (arm, thickness) = geometry;
    if arm < int_ratio(0) {
        return Ok(());
    }
    if thickness <= int_ratio(0) {
        return Ok(());
    }
    let overflow = || RenderError::ObjectOverflow {
        frame: frame_index,
        object,
    };
    let Ok(half) = thickness.checked_div(int_ratio(2)) else {
        return Err(overflow());
    };
    let flat = rect_corners(at, arm, half).map_err(|_| overflow())?;
    draw_mapped_polygon(frame, &flat, camera, colour, frame_index, object)?;
    let tall = rect_corners(at, half, arm).map_err(|_| overflow())?;
    draw_mapped_polygon(frame, &tall, camera, colour, frame_index, object)?;
    Ok(())
}

/// Draws one shape centred on `at` through the camera placement.
///
/// `at` is the object position in scene space. Disc radii scale by the camera
/// magnification; everything else maps corner by corner.
///
/// # Errors
///
/// Returns [`RenderError::ObjectOverflow`] when the placement for
/// `frame_index` and `object` overflows exact arithmetic.
pub(super) fn draw_object(
    frame: &mut Frame,
    shape: &Shape,
    at: Point,
    camera: &CameraFrame,
    colour: Rgb8,
    frame_index: FrameIndex,
    object: ObjectIndex,
) -> Result<(), RenderError> {
    let overflow = || RenderError::ObjectOverflow {
        frame: frame_index,
        object,
    };
    match shape {
        Shape::Disc { radius } => {
            let Ok(centre) = camera.transform.apply(at) else {
                return Err(overflow());
            };
            let Ok(scaled) = radius.checked_mul(camera.zoom) else {
                return Err(overflow());
            };
            fill_disc(frame, centre, scaled, colour).map_err(|_| overflow())?;
            Ok(())
        }
        Shape::Rect {
            half_width,
            half_height,
        } => {
            let corners = rect_corners(at, *half_width, *half_height).map_err(|_| overflow())?;
            draw_mapped_polygon(frame, &corners, camera, colour, frame_index, object)?;
            Ok(())
        }
        Shape::Polygon { vertices } => {
            let scene = shifted_vertices(at, vertices).map_err(|_| overflow())?;
            draw_mapped_polygon(frame, &scene, camera, colour, frame_index, object)?;
            Ok(())
        }
        Shape::Cross { arm, thickness } => {
            draw_cross_shape(
                frame,
                at,
                camera,
                colour,
                frame_index,
                object,
                (*arm, *thickness),
            )?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::PixelCoord;
    use crate::geom::Similarity;
    use crate::testutil::make_ratio;
    use crate::units::{Dimensions, Height, Width};

    /// Builds the identity camera placement used for shape tests.
    fn identity_placement() -> CameraFrame {
        CameraFrame {
            transform: Similarity::identity(),
            zoom: int_ratio(1),
        }
    }

    /// Draws one shape centred at `(9 / 2, 9 / 2)` on an 8x8 black frame.
    fn render_shape(shape: &Shape, fill: Rgb8) -> Frame {
        let width = Width::new(8).unwrap();
        let height = Height::new(8).unwrap();
        let mut frame = Frame::zeroed(Dimensions::new(width, height)).unwrap();
        frame.fill(Rgb8::new(0, 0, 0));
        let half = make_ratio(9, 2).unwrap();
        let at = Point::new(half, half);
        let camera = identity_placement();
        let frame_index = FrameIndex::new(0);
        let object = ObjectIndex::new(0);
        draw_object(&mut frame, shape, at, &camera, fill, frame_index, object).unwrap();
        frame
    }

    #[test]
    fn test_every_shape_draws_pixels() {
        let green = Rgb8::new(0, 255, 0);
        let black = Rgb8::new(0, 0, 0);
        let one = make_ratio(1, 1).unwrap();
        let two = make_ratio(2, 1).unwrap();
        let half = make_ratio(9, 2).unwrap();
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
            let frame = render_shape(shape, green);
            let mut painted = false;
            for y in 0..8_u16 {
                for x in 0..8_u16 {
                    if frame.pixel(PixelCoord::new(x, y)) != Some(black) {
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
        let half_width = make_ratio(2, 1).unwrap();
        let half_height = make_ratio(1, 1).unwrap();
        let half = make_ratio(9, 2).unwrap();
        let rect_frame = render_shape(
            &Shape::Rect {
                half_width,
                half_height,
            },
            red,
        );
        let left = half.checked_sub(half_width).unwrap();
        let right = half.checked_add(half_width).unwrap();
        let top = half.checked_sub(half_height).unwrap();
        let bottom = half.checked_add(half_height).unwrap();
        let corners = [
            Point::new(left, top),
            Point::new(right, top),
            Point::new(right, bottom),
            Point::new(left, bottom),
        ];
        let w = Width::new(8).unwrap();
        let h = Height::new(8).unwrap();
        let mut poly_frame = Frame::zeroed(Dimensions::new(w, h)).unwrap();
        poly_frame.fill(Rgb8::new(0, 0, 0));
        fill_polygon(&mut poly_frame, &corners, red)
            .expect("drawing a test fixture must not overflow");
        assert_eq!(
            rect_frame.data(),
            poly_frame.data(),
            "a rect must match the polygon through its absolute corners"
        );
    }
}
