//! Extent boxes for the rasterisers.
//!
//! Each drawing entry point in the parent module is paired here with a pure
//! function returning the exact rational box it would cover, without touching
//! a [`Frame`](crate::Frame). The manifest reports the box the renderer drew;
//! were it to compute its own, the repository would hold two implementations
//! of one piece of geometry and the ground truth could drift from the pixels
//! quietly. A drawing function returning `()` cannot be asked what it did;
//! these can.

use crate::geom::Point;
use crate::ratio::{half_ratio, int_ratio, Ratio};

/// An exact rational bounding box.
///
/// The box is the inclusive set `{ p : min_x <= p.x <= max_x,
/// min_y <= p.y <= max_y }`, matching the inclusive edge rule of the
/// rasterisers. [`Bounds::new`] rejects an empty box where either maximum
/// lies strictly below its minimum; [`Bounds::intersect`] and
/// [`Bounds::area`] treat such a box as empty as well, so a literal built by
/// hand can never report a negative area.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Bounds {
    /// Smallest `x` in the box, in scene units.
    pub min_x: Ratio,
    /// Smallest `y` in the box, in scene units.
    pub min_y: Ratio,
    /// Largest `x` in the box, in scene units.
    pub max_x: Ratio,
    /// Largest `y` in the box, in scene units.
    pub max_y: Ratio,
}

impl Bounds {
    /// Creates a bounding box, rejecting an empty one.
    ///
    /// Returns `None` when `max_x` lies strictly below `min_x` or `max_y`
    /// lies strictly below `min_y`.
    #[must_use]
    pub fn new(min_x: Ratio, min_y: Ratio, max_x: Ratio, max_y: Ratio) -> Option<Self> {
        if max_x < min_x {
            return None;
        }
        if max_y < min_y {
            return None;
        }
        Some(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    /// Returns the overlap of two boxes.
    ///
    /// Returns `None` when the boxes are disjoint. Touching at an edge or a
    /// corner still overlaps: the result is the degenerate box along that
    /// edge or point.
    #[must_use]
    pub fn intersect(self, other: Self) -> Option<Self> {
        let min_x = if self.min_x < other.min_x {
            other.min_x
        } else {
            self.min_x
        };
        let min_y = if self.min_y < other.min_y {
            other.min_y
        } else {
            self.min_y
        };
        let max_x = if self.max_x < other.max_x {
            self.max_x
        } else {
            other.max_x
        };
        let max_y = if self.max_y < other.max_y {
            self.max_y
        } else {
            other.max_y
        };
        Self::new(min_x, min_y, max_x, max_y)
    }

    /// Returns the geometric area of the box.
    ///
    /// Computes `(max_x - min_x) * (max_y - min_y)` exactly. Returns `None`
    /// when either side is inverted or when any intermediate or final value
    /// overflows.
    #[must_use]
    pub fn area(self) -> Option<Ratio> {
        if self.max_x < self.min_x {
            return None;
        }
        if self.max_y < self.min_y {
            return None;
        }
        let width = self.max_x.checked_sub(self.min_x)?;
        let height = self.max_y.checked_sub(self.min_y)?;
        width.checked_mul(height)
    }
}

/// Returns the box covered by [`fill_disc`](super::fill_disc).
///
/// The disc is `{ p : |p - centre|^2 <= radius^2 }`, whose tight box is
/// `[centre - radius, centre + radius]` on each axis. Returns `None` for a
/// non-positive radius, which draws nothing, or on overflow.
#[must_use]
pub fn disc_extent(centre: Point, radius: Ratio) -> Option<Bounds> {
    if radius <= int_ratio(0) {
        return None;
    }
    let min_x = centre.x.checked_sub(radius)?;
    let max_x = centre.x.checked_add(radius)?;
    let min_y = centre.y.checked_sub(radius)?;
    let max_y = centre.y.checked_add(radius)?;
    Bounds::new(min_x, min_y, max_x, max_y)
}

/// Returns the box covered by [`fill_polygon`](super::fill_polygon).
///
/// The box is the minimum and maximum vertex coordinates on each axis, which
/// is tight for the inclusive edge rule: every covered centre lies on an
/// edge or strictly inside, hence within the vertex ranges. Returns `None`
/// for fewer than three vertices, which draws nothing.
#[must_use]
pub fn polygon_extent(vertices: &[Point]) -> Option<Bounds> {
    if vertices.len() < 3 {
        return None;
    }
    let mut iter = vertices.iter();
    let first = iter.next()?;
    let mut min_x = first.x;
    let mut max_x = first.x;
    let mut min_y = first.y;
    let mut max_y = first.y;
    for vertex in iter {
        if vertex.x < min_x {
            min_x = vertex.x;
        }
        if max_x < vertex.x {
            max_x = vertex.x;
        }
        if vertex.y < min_y {
            min_y = vertex.y;
        }
        if max_y < vertex.y {
            max_y = vertex.y;
        }
    }
    Bounds::new(min_x, min_y, max_x, max_y)
}

/// Returns the box covered by [`fill_rect`](super::fill_rect).
///
/// The rectangle is `{ p : min.x <= p.x <= max.x, min.y <= p.y <= max.y }`,
/// so its own corners are already the box. Returns `None` when `max` lies
/// strictly below `min` on either axis, which draws nothing.
#[must_use]
pub fn rect_extent(min: Point, max: Point) -> Option<Bounds> {
    Bounds::new(min.x, min.y, max.x, max.y)
}

/// Returns the box covered by [`draw_line`](super::draw_line).
///
/// A 1-unit-thick line is the set of points within `1 / 2` of the segment,
/// so the box is the segment range expanded by half a unit on every side.
/// This also covers a zero-length segment, which draws the disc of radius
/// `1 / 2` around the point. Returns `None` only on overflow.
#[must_use]
pub fn line_extent(start: Point, end: Point) -> Option<Bounds> {
    let half = half_ratio();
    let (mut min_x, mut max_x) = if start.x < end.x {
        (start.x, end.x)
    } else {
        (end.x, start.x)
    };
    let (mut min_y, mut max_y) = if start.y < end.y {
        (start.y, end.y)
    } else {
        (end.y, start.y)
    };
    min_x = min_x.checked_sub(half)?;
    max_x = max_x.checked_add(half)?;
    min_y = min_y.checked_sub(half)?;
    max_y = max_y.checked_add(half)?;
    Bounds::new(min_x, min_y, max_x, max_y)
}

/// Returns the box covered by [`draw_cross`](super::draw_cross).
///
/// The cross is the union of its horizontal and vertical bars, so the box is
/// `[centre - reach, centre + reach]` on each axis where `reach` is the
/// larger of `arm` and `thickness / 2`. Returns `None` for a negative `arm`
/// or a non-positive `thickness`, which draws nothing, or on overflow.
#[must_use]
pub fn cross_extent(centre: Point, arm: Ratio, thickness: Ratio) -> Option<Bounds> {
    if arm < int_ratio(0) {
        return None;
    }
    if thickness <= int_ratio(0) {
        return None;
    }
    let half = thickness.checked_div(int_ratio(2))?;
    let reach = if half < arm { arm } else { half };
    let min_x = centre.x.checked_sub(reach)?;
    let max_x = centre.x.checked_add(reach)?;
    let min_y = centre.y.checked_sub(reach)?;
    let max_y = centre.y.checked_add(reach)?;
    Bounds::new(min_x, min_y, max_x, max_y)
}

#[cfg(test)]
mod tests {
    use super::super::coverage::pixel_centre;
    use super::*;
    use crate::color::Rgb8;
    use crate::frame::Frame;
    use crate::raster::{draw_cross, draw_line, fill_disc, fill_polygon, fill_rect};
    use crate::testutil::{black_8x8, make_ratio};
    use crate::units::{Dimensions, Height, Width};

    /// Builds a frame large enough to hold every test shape whole.
    fn large_frame() -> Option<Frame> {
        let width = Width::new(32)?;
        let height = Height::new(32)?;
        Frame::zeroed(Dimensions::new(width, height))
    }

    /// Collects the centres of all non-background pixels in a frame.
    fn painted_centres(frame: &Frame, ground: Rgb8) -> Vec<Point> {
        let width = frame.width().get().get();
        let height = frame.height().get().get();
        let mut found = Vec::new();
        for y in 0..height {
            for x in 0..width {
                if frame.pixel(x, y) != Some(ground) {
                    if let Some(centre) = pixel_centre(x, y) {
                        found.push(centre);
                    }
                }
            }
        }
        found
    }

    /// Asserts every painted pixel centre lies inside `bounds`.
    fn assert_inside(bounds: Bounds, frame: &Frame, ground: Rgb8, label: &str) {
        for sample in painted_centres(frame, ground) {
            assert!(
                bounds.min_x <= sample.x,
                "{label}: painted x must not lie left of the extent"
            );
            assert!(
                sample.x <= bounds.max_x,
                "{label}: painted x must not lie right of the extent"
            );
            assert!(
                bounds.min_y <= sample.y,
                "{label}: painted y must not lie above the extent"
            );
            assert!(
                sample.y <= bounds.max_y,
                "{label}: painted y must not lie below the extent"
            );
        }
    }

    #[test]
    fn test_bounds_intersect_area() {
        let Some(a_min_x) = make_ratio(0, 1) else {
            return;
        };
        let Some(a_min_y) = make_ratio(0, 1) else {
            return;
        };
        let Some(a_max_x) = make_ratio(4, 1) else {
            return;
        };
        let Some(a_max_y) = make_ratio(4, 1) else {
            return;
        };
        let Some(first) = Bounds::new(a_min_x, a_min_y, a_max_x, a_max_y) else {
            return;
        };
        let Some(b_min_x) = make_ratio(2, 1) else {
            return;
        };
        let Some(b_min_y) = make_ratio(2, 1) else {
            return;
        };
        let Some(b_max_x) = make_ratio(6, 1) else {
            return;
        };
        let Some(b_max_y) = make_ratio(6, 1) else {
            return;
        };
        let Some(second) = Bounds::new(b_min_x, b_min_y, b_max_x, b_max_y) else {
            return;
        };
        let Some(overlap) = first.intersect(second) else {
            return;
        };
        assert_eq!(overlap.min_x, b_min_x, "overlap must start at 2");
        assert_eq!(overlap.max_x, a_max_x, "overlap must end at 4");
        let Some(two) = make_ratio(2, 1) else { return };
        let Some(expected) = two.checked_mul(two) else {
            return;
        };
        assert_eq!(
            overlap.area(),
            Some(expected),
            "2x2 overlap must have area 4"
        );
        let Some(sixteen) = make_ratio(16, 1) else {
            return;
        };
        assert_eq!(first.area(), Some(sixteen), "4x4 box must have area 16");
        let Some(far_min) = make_ratio(10, 1) else {
            return;
        };
        let Some(far_max) = make_ratio(12, 1) else {
            return;
        };
        let Some(far) = Bounds::new(far_min, far_min, far_max, far_max) else {
            return;
        };
        assert!(
            first.intersect(far).is_none(),
            "disjoint boxes must not intersect"
        );
        assert!(
            Bounds::new(a_max_x, a_min_y, a_min_x, a_max_y).is_none(),
            "an inverted box must be rejected"
        );
    }

    #[test]
    fn test_disc_extent_contains_pixels() {
        let Some(mut frame) = large_frame() else {
            return;
        };
        let ground = Rgb8::new(0, 0, 0);
        frame.fill(ground);
        let paint = Rgb8::new(255, 0, 0);
        let Some(cx) = make_ratio(16, 1) else { return };
        let Some(cy) = make_ratio(16, 1) else { return };
        let Some(radius) = make_ratio(5, 1) else {
            return;
        };
        let centre = Point::new(cx, cy);
        fill_disc(&mut frame, centre, radius, paint);
        let Some(bounds) = disc_extent(centre, radius) else {
            return;
        };
        assert_inside(bounds, &frame, ground, "disc");
        assert!(
            !painted_centres(&frame, ground).is_empty(),
            "the test disc must paint at least one pixel"
        );
        let Some(zero) = make_ratio(0, 1) else { return };
        assert!(
            disc_extent(centre, zero).is_none(),
            "a zero radius must have no extent"
        );
    }

    #[test]
    fn test_polygon_extent_contains_pixels() {
        let Some(mut frame) = large_frame() else {
            return;
        };
        let ground = Rgb8::new(0, 0, 0);
        frame.fill(ground);
        let paint = Rgb8::new(0, 255, 0);
        let Some(twelve) = make_ratio(12, 1) else {
            return;
        };
        let Some(twenty) = make_ratio(20, 1) else {
            return;
        };
        let Some(sixteen) = make_ratio(16, 1) else {
            return;
        };
        let vertices = [
            Point::new(twelve, twelve),
            Point::new(twenty, twelve),
            Point::new(sixteen, twenty),
        ];
        let Some(slice) = vertices.get(0..3) else {
            return;
        };
        fill_polygon(&mut frame, slice, paint);
        let Some(bounds) = polygon_extent(slice) else {
            return;
        };
        assert_inside(bounds, &frame, ground, "polygon");
        assert!(
            !painted_centres(&frame, ground).is_empty(),
            "the test triangle must paint at least one pixel"
        );
        let Some(pair) = vertices.get(0..2) else {
            return;
        };
        assert!(
            polygon_extent(pair).is_none(),
            "two vertices must have no extent"
        );
    }

    #[test]
    fn test_rect_extent_contains_pixels() {
        let Some(mut frame) = large_frame() else {
            return;
        };
        let ground = Rgb8::new(0, 0, 0);
        frame.fill(ground);
        let paint = Rgb8::new(0, 0, 255);
        let Some(ten) = make_ratio(10, 1) else { return };
        let Some(twenty) = make_ratio(20, 1) else {
            return;
        };
        let lower = Point::new(ten, ten);
        let upper = Point::new(twenty, twenty);
        fill_rect(&mut frame, lower, upper, paint);
        let Some(bounds) = rect_extent(lower, upper) else {
            return;
        };
        assert_inside(bounds, &frame, ground, "rect");
        assert!(
            rect_extent(upper, lower).is_none(),
            "an inverted rectangle must have no extent"
        );
    }

    #[test]
    fn test_line_extent_contains_pixels() {
        let Some(mut frame) = large_frame() else {
            return;
        };
        let ground = Rgb8::new(0, 0, 0);
        frame.fill(ground);
        let paint = Rgb8::new(255, 255, 0);
        let Some(ten) = make_ratio(10, 1) else { return };
        let Some(twenty_two) = make_ratio(22, 1) else {
            return;
        };
        let Some(sixteen) = make_ratio(16, 1) else {
            return;
        };
        let start = Point::new(ten, sixteen);
        let end = Point::new(twenty_two, sixteen);
        draw_line(&mut frame, start, end, paint);
        let Some(bounds) = line_extent(start, end) else {
            return;
        };
        assert_inside(bounds, &frame, ground, "line");
        assert!(
            !painted_centres(&frame, ground).is_empty(),
            "the test line must paint at least one pixel"
        );
    }

    #[test]
    fn test_cross_extent_contains_pixels() {
        let Some(mut frame) = large_frame() else {
            return;
        };
        let ground = Rgb8::new(0, 0, 0);
        frame.fill(ground);
        let paint = Rgb8::new(255, 0, 255);
        let Some(sixteen) = make_ratio(16, 1) else {
            return;
        };
        let Some(arm) = make_ratio(6, 1) else { return };
        let Some(thick) = make_ratio(2, 1) else {
            return;
        };
        let centre = Point::new(sixteen, sixteen);
        draw_cross(&mut frame, centre, arm, thick, paint);
        let Some(bounds) = cross_extent(centre, arm, thick) else {
            return;
        };
        assert_inside(bounds, &frame, ground, "cross");
        assert!(
            !painted_centres(&frame, ground).is_empty(),
            "the test cross must paint at least one pixel"
        );
        let Some(neg) = make_ratio(-1, 1) else { return };
        assert!(
            cross_extent(centre, neg, thick).is_none(),
            "a negative arm must have no extent"
        );
    }

    #[test]
    fn test_straddling_shapes_clip_correctly() {
        let Some(mut frame) = black_8x8() else { return };
        let paint = Rgb8::new(200, 40, 40);
        let Some(neg_two) = make_ratio(-2, 1) else {
            return;
        };
        let Some(ten) = make_ratio(10, 1) else { return };
        let Some(four) = make_ratio(4, 1) else { return };
        let Some(three) = make_ratio(3, 1) else {
            return;
        };
        fill_disc(&mut frame, Point::new(four, four), ten, paint);
        assert_eq!(
            frame.pixel(0, 0),
            Some(paint),
            "a huge disc must cover the corner pixel"
        );
        assert_eq!(
            frame.pixel(7, 7),
            Some(paint),
            "a huge disc must cover the far corner pixel"
        );
        let corners = [
            Point::new(neg_two, neg_two),
            Point::new(ten, neg_two),
            Point::new(ten, ten),
            Point::new(neg_two, ten),
        ];
        let Some(huge) = corners.get(0..4) else {
            return;
        };
        fill_polygon(&mut frame, huge, paint);
        assert_eq!(
            frame.pixel(7, 7),
            Some(paint),
            "a huge polygon must cover the far corner pixel"
        );
        draw_line(
            &mut frame,
            Point::new(neg_two, neg_two),
            Point::new(ten, ten),
            paint,
        );
        assert_eq!(
            frame.pixel(4, 4),
            Some(paint),
            "a straddling line must cover the centre pixel"
        );
        draw_cross(&mut frame, Point::new(four, four), ten, three, paint);
        assert_eq!(
            frame.pixel(4, 0),
            Some(paint),
            "a straddling cross must cover the top-middle pixel"
        );
        assert_eq!(
            frame.pixel(0, 0),
            Some(paint),
            "a straddling cross must cover the corner through the disc"
        );
    }
}
