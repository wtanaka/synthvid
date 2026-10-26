//! Shape drawing for frame rendering.
//!
//! [`draw_object`] draws one [`Shape`] centered on a scene-space position
//! through the view placement of the parent module. Disc centers pass
//! through the view transform and radii scale by the magnification;
//! rectangles, polygon vertices, and cross bars map corner by corner, so
//! view rotation turns them into rotated polygons rather than being
//! ignored. A placement that overflows exact arithmetic is an error; see
//! [`RenderError`](super::RenderError). All arithmetic is exact [`Ratio`]
//! arithmetic.

use crate::color::Rgb8;
use crate::frame::Frame;
use crate::geom::Point;
use crate::raster::{fill_disc, fill_polygon, HalfSize};
use crate::ratio::{Numerator, Overflow, Ratio};
use crate::scene::Shape;

use super::placement::Slot;
use super::{RenderError, ViewFrame};

/// Returns the four corners of an axis-aligned rectangle.
///
/// The corners run clockwise from the top-left `(left, top)`. Returns `Err(Overflow)`
/// on overflow; the caller reports [`RenderError::ObjectOverflow`].
pub(super) fn rect_corners(centre: Point, half: HalfSize) -> Result<[Point; 4], Overflow> {
    let left = centre.across.checked_sub(half.width)?;
    let right = centre.across.checked_add(half.width)?;
    let top = centre.down.checked_sub(half.height)?;
    let bottom = centre.down.checked_add(half.height)?;
    Ok([
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
    ])
}

/// What one object is drawn with: the view placement for the frame, the
/// object's fill colour, and the frame and object an overflow is reported
/// against.
#[derive(Copy, Clone, Debug)]
pub(super) struct Brush<'view> {
    /// The view placement for the frame being drawn.
    pub(super) view: &'view ViewFrame,
    /// The colour the object is filled with.
    pub(super) colour: Rgb8,
    /// The frame and object being drawn.
    pub(super) slot: Slot,
}

/// Fills the polygon through `scene` after mapping each vertex to the screen.
///
/// # Errors
///
/// Returns [`RenderError::ObjectOverflow`] when a vertex view transform
/// overflows exact arithmetic.
fn draw_mapped_polygon(
    frame: &mut Frame,
    scene: &[Point],
    brush: Brush<'_>,
) -> Result<(), RenderError> {
    let mut mapped = Vec::with_capacity(scene.len());
    for vertex in scene {
        let Ok(screen) = brush.view.transform.apply(*vertex) else {
            return Err(brush.slot.overflow());
        };
        mapped.push(screen);
    }
    fill_polygon(frame, &mapped, brush.colour).map_err(|Overflow| brush.slot.overflow())
}

/// Shifts polygon vertices from shape space to scene space.
///
/// Shape vertices are offsets from the object position, so each scene vertex
/// is `at + vertex`. Returns `Err(Overflow)` on overflow; the caller reports
/// [`RenderError::ObjectOverflow`].
pub(super) fn shifted_vertices(at: Point, vertices: &[Point]) -> Result<Vec<Point>, Overflow> {
    let mut scene = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        let shifted_x = at.across.checked_add(vertex.across)?;
        let shifted_y = at.down.checked_add(vertex.down)?;
        scene.push(Point {
            across: shifted_x,
            down: shifted_y,
        });
    }
    Ok(scene)
}

/// Draws a cross shape through the view transform.
///
/// The cross is the union of its horizontal and vertical bars, each mapped
/// corner by corner so that view rotation turns the bars into rotated
/// rectangles rather than being ignored. With the identity view the corners
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
    brush: Brush<'_>,
    geometry: (Ratio, Ratio),
) -> Result<(), RenderError> {
    let (arm, thickness) = geometry;
    if arm < Ratio::from_integer(Numerator(0)) {
        return Ok(());
    }
    if thickness <= Ratio::from_integer(Numerator(0)) {
        return Ok(());
    }
    let Ok(half) = thickness.checked_div(Ratio::from_integer(Numerator(2))) else {
        return Err(brush.slot.overflow());
    };
    let flat = rect_corners(
        at,
        HalfSize {
            width: arm,
            height: half,
        },
    )
    .map_err(|Overflow| brush.slot.overflow())?;
    draw_mapped_polygon(frame, &flat, brush)?;
    let tall = rect_corners(
        at,
        HalfSize {
            width: half,
            height: arm,
        },
    )
    .map_err(|Overflow| brush.slot.overflow())?;
    draw_mapped_polygon(frame, &tall, brush)
}

/// Draws one shape centred on `at` through the view placement.
///
/// `at` is the object position in scene space. Disc radii scale by the view
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
    brush: Brush<'_>,
) -> Result<(), RenderError> {
    let overflow = || brush.slot.overflow();
    match shape {
        Shape::Disc { radius } => {
            let Ok(centre) = brush.view.transform.apply(at) else {
                return Err(overflow());
            };
            let Ok(scaled) = radius.checked_mul(brush.view.zoom) else {
                return Err(overflow());
            };
            fill_disc(frame, centre, scaled, brush.colour).map_err(|Overflow| overflow())
        }
        Shape::Rect {
            half_width,
            half_height,
        } => {
            let corners = rect_corners(
                at,
                HalfSize {
                    width: *half_width,
                    height: *half_height,
                },
            )
            .map_err(|Overflow| overflow())?;
            draw_mapped_polygon(frame, &corners, brush)
        }
        Shape::Polygon { vertices } => {
            let scene = shifted_vertices(at, vertices).map_err(|Overflow| overflow())?;
            draw_mapped_polygon(frame, &scene, brush)
        }
        Shape::Cross { arm, thickness } => draw_cross_shape(frame, at, brush, (*arm, *thickness)),
    }
}

#[cfg(test)]
mod tests;
