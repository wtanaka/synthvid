//! Extent boxes for the rasterisers.
//!
//! Each drawing entry point in the parent module is paired here with a pure
//! function returning the exact rational box it would cover, without touching
//! a [`Frame`](crate::Frame). The manifest reports the box the renderer drew;
//! were it to compute its own, the repository would hold two implementations
//! of one piece of geometry and the ground truth could drift from the pixels
//! quietly. A drawing function returning `()` cannot be asked what it did;
//! these can.

use core::fmt;

use crate::geom::Point;
use crate::ratio::{half_ratio, int_ratio, Overflow, Ratio};

/// Error returned when attempting to construct an invalid [`Bounds`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BoundsError {
    /// Bounds must have maximum coordinates at or after minimum coordinates.
    Inverted,
}

impl fmt::Display for BoundsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inverted => {
                write!(
                    f,
                    "maximum coordinates must be at or after minimum coordinates"
                )
            }
        }
    }
}

impl core::error::Error for BoundsError {}

/// A bounding box or an indication that nothing was drawn.
///
/// An empty result and an arithmetic failure are different outcomes and must
/// not share a representation. A shape with invalid parameters (non-positive
/// radius, non-positive thickness, inverted box, fewer than three vertices)
/// draws nothing and that is the correct answer. An extent that overflows exact
/// arithmetic is a failure the caller must hear about. Collapsing both into
/// `None` is what let an overflow be silently rendered as a missing shape.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BoundsOrEmpty {
    /// Nothing is drawn; this is the correct outcome.
    Empty,
    /// The bounding box that was computed.
    Covering(Bounds),
}

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
    /// # Errors
    ///
    /// Returns `Err(BoundsError::Inverted)` when `max_x` lies strictly below `min_x`
    /// or `max_y` lies strictly below `min_y`.
    pub fn new(
        min_x: Ratio,
        min_y: Ratio,
        max_x: Ratio,
        max_y: Ratio,
    ) -> Result<Self, BoundsError> {
        if max_x < min_x {
            return Err(BoundsError::Inverted);
        }
        if max_y < min_y {
            return Err(BoundsError::Inverted);
        }
        Ok(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    /// Returns the overlap of two boxes.
    ///
    /// # Errors
    ///
    /// Returns `Err(BoundsError::Inverted)` when the boxes are disjoint. Touching at an edge
    /// or a corner still overlaps: the result is the degenerate box along that edge or point.
    pub fn intersect(self, other: Self) -> Result<Self, BoundsError> {
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
    /// Computes `(max_x - min_x) * (max_y - min_y)` exactly.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` when any intermediate or final value overflows.
    /// An inverted box is treated as empty and returns `Ok(Ratio::zero())`.
    pub fn area(self) -> Result<Ratio, Overflow> {
        if self.max_x < self.min_x {
            return Ok(int_ratio(0));
        }
        if self.max_y < self.min_y {
            return Ok(int_ratio(0));
        }
        let width = self.max_x.checked_sub(self.min_x)?;
        let height = self.max_y.checked_sub(self.min_y)?;
        width.checked_mul(height)
    }
}

/// Returns the box covered by [`fill_disc`](super::fill_disc).
///
/// The disc is `{ p : |p - centre|^2 <= radius^2 }`, whose tight box is
/// `[centre - radius, centre + radius]` on each axis.
///
/// # Errors
///
/// Returns `Err(Overflow)` only when the conversion overflows exact arithmetic.
/// A non-positive radius yields [`BoundsOrEmpty::Empty`].
pub fn disc_extent(centre: Point, radius: Ratio) -> Result<BoundsOrEmpty, Overflow> {
    if radius <= int_ratio(0) {
        return Ok(BoundsOrEmpty::Empty);
    }
    let min_x = centre.x.checked_sub(radius)?;
    let max_x = centre.x.checked_add(radius)?;
    let min_y = centre.y.checked_sub(radius)?;
    let max_y = centre.y.checked_add(radius)?;
    Ok(Bounds::new(min_x, min_y, max_x, max_y)
        .map_or(BoundsOrEmpty::Empty, BoundsOrEmpty::Covering))
}

/// Returns the box covered by [`fill_polygon`](super::fill_polygon).
///
/// The box is the minimum and maximum vertex coordinates on each axis, which
/// is tight for the inclusive edge rule: every covered centre lies on an
/// edge or strictly inside, hence within the vertex ranges.
///
/// # Errors
///
/// Returns `Err(Overflow)` only when the conversion overflows exact arithmetic.
/// Fewer than three vertices yields [`BoundsOrEmpty::Empty`].
pub fn polygon_extent(vertices: &[Point]) -> Result<BoundsOrEmpty, Overflow> {
    if vertices.len() < 3 {
        return Ok(BoundsOrEmpty::Empty);
    }
    let mut iter = vertices.iter();
    let first = iter.next().ok_or(Overflow)?;
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
    Ok(Bounds::new(min_x, min_y, max_x, max_y)
        .map_or(BoundsOrEmpty::Empty, BoundsOrEmpty::Covering))
}

/// Returns the box covered by [`fill_rect`](super::fill_rect).
///
/// The rectangle is `{ p : min.x <= p.x <= max.x, min.y <= p.y <= max.y }`,
/// so its own corners are already the box.
///
/// # Errors
///
/// Returns `Err(Overflow)` only when the conversion overflows exact arithmetic.
/// When `max` lies strictly below `min` on either axis, yields [`BoundsOrEmpty::Empty`].
pub fn rect_extent(min: Point, max: Point) -> Result<BoundsOrEmpty, Overflow> {
    Ok(Bounds::new(min.x, min.y, max.x, max.y)
        .map_or(BoundsOrEmpty::Empty, BoundsOrEmpty::Covering))
}

/// Returns the box covered by [`draw_line`](super::draw_line).
///
/// A 1-unit-thick line is the set of points within `1 / 2` of the segment,
/// so the box is the segment range expanded by half a unit on every side.
/// This also covers a zero-length segment, which draws the disc of radius
/// `1 / 2` around the point.
///
/// # Errors
///
/// Returns `Err(Overflow)` when any intermediate or final value overflows.
pub fn line_extent(start: Point, end: Point) -> Result<BoundsOrEmpty, Overflow> {
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
    Ok(Bounds::new(min_x, min_y, max_x, max_y)
        .map_or(BoundsOrEmpty::Empty, BoundsOrEmpty::Covering))
}

/// Returns the box covered by [`draw_cross`](super::draw_cross).
///
/// The cross is the union of its horizontal and vertical bars, so the box is
/// `[centre - reach, centre + reach]` on each axis where `reach` is the
/// larger of `arm` and `thickness / 2`.
///
/// # Errors
///
/// Returns `Err(Overflow)` only when the conversion overflows exact arithmetic.
/// A negative `arm` or a non-positive `thickness` yields [`BoundsOrEmpty::Empty`].
pub fn cross_extent(
    centre: Point,
    arm: Ratio,
    thickness: Ratio,
) -> Result<BoundsOrEmpty, Overflow> {
    if arm < int_ratio(0) {
        return Ok(BoundsOrEmpty::Empty);
    }
    if thickness <= int_ratio(0) {
        return Ok(BoundsOrEmpty::Empty);
    }
    let half = thickness.checked_div(int_ratio(2))?;
    let reach = if half < arm { arm } else { half };
    let min_x = centre.x.checked_sub(reach)?;
    let max_x = centre.x.checked_add(reach)?;
    let min_y = centre.y.checked_sub(reach)?;
    let max_y = centre.y.checked_add(reach)?;
    Ok(Bounds::new(min_x, min_y, max_x, max_y)
        .map_or(BoundsOrEmpty::Empty, BoundsOrEmpty::Covering))
}

#[cfg(test)]
mod tests {
    use super::super::coverage::pixel_centre;
    use super::*;
    use crate::color::Rgb8;
    use crate::frame::{Frame, PixelCoord};
    use crate::raster::{draw_cross, draw_line, fill_disc, fill_polygon, fill_rect};
    use crate::testutil::{black_8x8, make_ratio};
    use crate::units::{Dimensions, Height, Width};

    /// Builds a frame large enough to hold every test shape whole.
    fn large_frame() -> Frame {
        let width = Width::new(32).unwrap();
        let height = Height::new(32).unwrap();
        Frame::zeroed(Dimensions::new(width, height)).unwrap()
    }

    /// Collects the centres of all non-background pixels in a frame.
    fn painted_centres(frame: &Frame, ground: Rgb8) -> Vec<Point> {
        let width = frame.width().get().get();
        let height = frame.height().get().get();
        let mut found = Vec::new();
        for y in 0..height {
            for x in 0..width {
                if frame.pixel(PixelCoord::new(x, y)) != Some(ground) {
                    found.push(pixel_centre(x, y));
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
        let a_min_x = make_ratio(0, 1).unwrap();
        let a_min_y = make_ratio(0, 1).unwrap();
        let a_max_x = make_ratio(4, 1).unwrap();
        let a_max_y = make_ratio(4, 1).unwrap();
        let first = Bounds::new(a_min_x, a_min_y, a_max_x, a_max_y).unwrap();
        let b_min_x = make_ratio(2, 1).unwrap();
        let b_min_y = make_ratio(2, 1).unwrap();
        let b_max_x = make_ratio(6, 1).unwrap();
        let b_max_y = make_ratio(6, 1).unwrap();
        let second = Bounds::new(b_min_x, b_min_y, b_max_x, b_max_y).unwrap();
        let overlap = first.intersect(second).unwrap();
        assert_eq!(overlap.min_x, b_min_x, "overlap must start at 2");
        assert_eq!(overlap.max_x, a_max_x, "overlap must end at 4");
        let two = make_ratio(2, 1).unwrap();
        let expected = two.checked_mul(two).unwrap();
        assert_eq!(overlap.area(), Ok(expected), "2x2 overlap must have area 4");
        let sixteen = make_ratio(16, 1).unwrap();
        assert_eq!(first.area(), Ok(sixteen), "4x4 box must have area 16");
        let far_min = make_ratio(10, 1).unwrap();
        let far_max = make_ratio(12, 1).unwrap();
        let far = Bounds::new(far_min, far_min, far_max, far_max).unwrap();
        assert!(
            first.intersect(far).is_err(),
            "disjoint boxes must not intersect"
        );
        assert!(
            Bounds::new(a_max_x, a_min_y, a_min_x, a_max_y).is_err(),
            "an inverted box must be rejected"
        );
    }

    #[test]
    fn test_disc_extent_contains_pixels() {
        let mut frame = large_frame();
        let ground = Rgb8::new(0, 0, 0);
        frame.fill(ground);
        let paint = Rgb8::new(255, 0, 0);
        let cx = make_ratio(16, 1).unwrap();
        let cy = make_ratio(16, 1).unwrap();
        let radius = make_ratio(5, 1).unwrap();
        let centre = Point::new(cx, cy);
        fill_disc(&mut frame, centre, radius, paint)
            .expect("drawing a test fixture must not overflow");
        let bounds = match disc_extent(centre, radius).unwrap() {
            BoundsOrEmpty::Covering(b) => b,
            BoundsOrEmpty::Empty => panic!("disc extent should not be empty"),
        };
        assert_inside(bounds, &frame, ground, "disc");
        assert!(
            !painted_centres(&frame, ground).is_empty(),
            "the test disc must paint at least one pixel"
        );
        let zero = make_ratio(0, 1).unwrap();
        assert!(
            matches!(disc_extent(centre, zero).unwrap(), BoundsOrEmpty::Empty),
            "a zero radius must have no extent"
        );
    }

    #[test]
    fn test_polygon_extent_contains_pixels() {
        let mut frame = large_frame();
        let ground = Rgb8::new(0, 0, 0);
        frame.fill(ground);
        let paint = Rgb8::new(0, 255, 0);
        let twelve = make_ratio(12, 1).unwrap();
        let twenty = make_ratio(20, 1).unwrap();
        let sixteen = make_ratio(16, 1).unwrap();
        let vertices = [
            Point::new(twelve, twelve),
            Point::new(twenty, twelve),
            Point::new(sixteen, twenty),
        ];
        let slice = &vertices[0..3];
        fill_polygon(&mut frame, slice, paint).expect("drawing a test fixture must not overflow");
        let bounds = match polygon_extent(slice).unwrap() {
            BoundsOrEmpty::Covering(b) => b,
            BoundsOrEmpty::Empty => panic!("polygon extent should not be empty"),
        };
        assert_inside(bounds, &frame, ground, "polygon");
        assert!(
            !painted_centres(&frame, ground).is_empty(),
            "the test triangle must paint at least one pixel"
        );
        let pair = &vertices[0..2];
        assert!(
            matches!(polygon_extent(pair).unwrap(), BoundsOrEmpty::Empty),
            "two vertices must have no extent"
        );
    }

    #[test]
    fn test_rect_extent_contains_pixels() {
        let mut frame = large_frame();
        let ground = Rgb8::new(0, 0, 0);
        frame.fill(ground);
        let paint = Rgb8::new(0, 0, 255);
        let ten = make_ratio(10, 1).unwrap();
        let twenty = make_ratio(20, 1).unwrap();
        let lower = Point::new(ten, ten);
        let upper = Point::new(twenty, twenty);
        fill_rect(&mut frame, lower, upper, paint)
            .expect("drawing a test fixture must not overflow");
        let bounds = match rect_extent(lower, upper).unwrap() {
            BoundsOrEmpty::Covering(b) => b,
            BoundsOrEmpty::Empty => panic!("rect extent should not be empty"),
        };
        assert_inside(bounds, &frame, ground, "rect");
        assert!(
            matches!(rect_extent(upper, lower).unwrap(), BoundsOrEmpty::Empty),
            "an inverted rectangle must have no extent"
        );
    }

    #[test]
    fn test_line_extent_contains_pixels() {
        let mut frame = large_frame();
        let ground = Rgb8::new(0, 0, 0);
        frame.fill(ground);
        let paint = Rgb8::new(255, 255, 0);
        let ten = make_ratio(10, 1).unwrap();
        let twenty_two = make_ratio(22, 1).unwrap();
        let sixteen = make_ratio(16, 1).unwrap();
        let start = Point::new(ten, sixteen);
        let end = Point::new(twenty_two, sixteen);
        draw_line(&mut frame, start, end, paint).expect("drawing a test fixture must not overflow");
        let bounds = match line_extent(start, end).unwrap() {
            BoundsOrEmpty::Covering(b) => b,
            BoundsOrEmpty::Empty => panic!("line extent should not be empty"),
        };
        assert_inside(bounds, &frame, ground, "line");
        assert!(
            !painted_centres(&frame, ground).is_empty(),
            "the test line must paint at least one pixel"
        );
    }

    #[test]
    fn test_cross_extent_contains_pixels() {
        let mut frame = large_frame();
        let ground = Rgb8::new(0, 0, 0);
        frame.fill(ground);
        let paint = Rgb8::new(255, 0, 255);
        let sixteen = make_ratio(16, 1).unwrap();
        let arm = make_ratio(6, 1).unwrap();
        let thick = make_ratio(2, 1).unwrap();
        let centre = Point::new(sixteen, sixteen);
        draw_cross(&mut frame, centre, arm, thick, paint)
            .expect("drawing a test fixture must not overflow");
        let bounds = match cross_extent(centre, arm, thick).unwrap() {
            BoundsOrEmpty::Covering(b) => b,
            BoundsOrEmpty::Empty => panic!("cross extent should not be empty"),
        };
        assert_inside(bounds, &frame, ground, "cross");
        assert!(
            !painted_centres(&frame, ground).is_empty(),
            "the test cross must paint at least one pixel"
        );
        let neg = make_ratio(-1, 1).unwrap();
        assert!(
            matches!(
                cross_extent(centre, neg, thick).unwrap(),
                BoundsOrEmpty::Empty
            ),
            "a negative arm must have no extent"
        );
    }

    #[test]
    fn test_straddling_shapes_clip_correctly() {
        let mut frame = black_8x8().unwrap();
        let paint = Rgb8::new(200, 40, 40);
        let neg_two = make_ratio(-2, 1).unwrap();
        let ten = make_ratio(10, 1).unwrap();
        let four = make_ratio(4, 1).unwrap();
        let three = make_ratio(3, 1).unwrap();
        fill_disc(&mut frame, Point::new(four, four), ten, paint)
            .expect("drawing a test fixture must not overflow");
        assert_eq!(
            frame.pixel(PixelCoord::new(0, 0)),
            Some(paint),
            "a huge disc must cover the corner pixel"
        );
        assert_eq!(
            frame.pixel(PixelCoord::new(7, 7)),
            Some(paint),
            "a huge disc must cover the far corner pixel"
        );
        let corners = [
            Point::new(neg_two, neg_two),
            Point::new(ten, neg_two),
            Point::new(ten, ten),
            Point::new(neg_two, ten),
        ];
        let huge = &corners[0..4];
        fill_polygon(&mut frame, huge, paint).expect("drawing a test fixture must not overflow");
        assert_eq!(
            frame.pixel(PixelCoord::new(7, 7)),
            Some(paint),
            "a huge polygon must cover the far corner pixel"
        );
        draw_line(
            &mut frame,
            Point::new(neg_two, neg_two),
            Point::new(ten, ten),
            paint,
        )
        .expect("drawing a test fixture must not overflow");
        assert_eq!(
            frame.pixel(PixelCoord::new(4, 4)),
            Some(paint),
            "a straddling line must cover the centre pixel"
        );
        draw_cross(&mut frame, Point::new(four, four), ten, three, paint)
            .expect("drawing a test fixture must not overflow");
        assert_eq!(
            frame.pixel(PixelCoord::new(4, 0)),
            Some(paint),
            "a straddling cross must cover the top-middle pixel"
        );
        assert_eq!(
            frame.pixel(PixelCoord::new(0, 0)),
            Some(paint),
            "a straddling cross must cover the corner through the disc"
        );
    }
}
