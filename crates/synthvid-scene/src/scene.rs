//! Scene description.
//!
//! Plain data and nothing else. [`Background`], [`Shape`], [`Object`],
//! [`Camera`], and [`Scene`] describe what is in the frame; evaluation of
//! motion and drawing of pixels belong to later concerns and live elsewhere.
//! Every type here is [`Clone`] and [`PartialEq`], so a future canonical text
//! form can round-trip a scene and compare the result with [`Eq`].

use core::fmt;
use core::num::NonZeroU16;

use crate::camera::Camera;
use crate::color::Rgb8;
use crate::geom::{Point, Vector};
use crate::ratio::Ratio;
use crate::units::{Dimensions, FrameCount, FrameIndex, FrameRate, Seed};

/// Direction of a linear gradient across the frame.
///
/// The gradient interpolates from `from` at one edge to `to` at the opposite
/// edge along the named axis.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Direction {
    /// Gradient varies from left (`from`) to right (`to`).
    Horizontal,
    /// Gradient varies from top (`from`) to bottom (`to`).
    Vertical,
}

/// Error returned when a [`Scale`] cannot be constructed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ScaleError {
    /// Scale must be strictly positive.
    NonPositive,
}

impl fmt::Display for ScaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositive => write!(f, "scale must be strictly positive"),
        }
    }
}

impl core::error::Error for ScaleError {}

/// Declares how many scene units correspond to one world unit.
///
/// A scene is laid out in scene units, which coincide with pixel units. The
/// optional [`Scene::scale`] gives the layout an exact physical
/// interpretation, so a manifest can report real distances rather than only
/// pixels. The value must be strictly positive.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Scale(Ratio);

impl Scale {
    /// Creates a scale from a [`Ratio`], rejecting zero and negative values.
    ///
    /// Returns `None` when `units_per_world_unit` is zero or negative.
    #[must_use]
    pub const fn new(units_per_world_unit: Ratio) -> Option<Self> {
        if units_per_world_unit.numer() > 0 {
            Some(Self(units_per_world_unit))
        } else {
            None
        }
    }

    /// Creates a scale from a [`Ratio`], rejecting zero and negative values.
    ///
    /// Returns `None` when `units_per_world_unit` is zero or negative.
    #[must_use]
    pub const fn from_ratio(units_per_world_unit: Ratio) -> Option<Self> {
        Self::new(units_per_world_unit)
    }

    /// Returns the underlying [`Ratio`] of scene units per world unit.
    #[must_use]
    pub const fn get(self) -> Ratio {
        self.0
    }

    /// Returns the underlying [`Ratio`] of scene units per world unit.
    #[must_use]
    pub const fn ratio(self) -> Ratio {
        self.0
    }
}

impl TryFrom<Ratio> for Scale {
    type Error = ScaleError;

    fn try_from(units_per_world_unit: Ratio) -> Result<Self, Self::Error> {
        Self::new(units_per_world_unit).ok_or(ScaleError::NonPositive)
    }
}

/// Inclusive-start, exclusive-end span of frames during which an object is visible.
///
/// Every span covers at least one frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FrameSpan {
    /// First visible frame index, inclusive.
    pub start: FrameIndex,
    /// One past the last visible frame index, exclusive.
    pub end: FrameIndex,
}

impl FrameSpan {
    /// Creates a frame span from `start` (inclusive) to `end` (exclusive).
    ///
    /// Returns `None` when `start` lies at or after `end`.
    #[must_use]
    pub const fn new(start: FrameIndex, end: FrameIndex) -> Option<Self> {
        if start.get() >= end.get() {
            None
        } else {
            Some(Self { start, end })
        }
    }

    /// Returns `true` when `frame` lies in `[start, end)`.
    #[must_use]
    pub const fn contains(self, frame: FrameIndex) -> bool {
        frame.get() >= self.start.get() && frame.get() < self.end.get()
    }
}

/// Position of an object or camera property as a closed-form function of frame.
///
/// Every variant evaluates at a frame index without touching any other frame,
/// so results are identical whether frames run forwards, backwards, or
/// shuffled. `Circular` and `Oscillating` evaluate with the in-crate
/// trigonometry; `Walk` re-seeds the in-crate generator from its seed and the
/// frame index rather than accumulating step by step.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Motion {
    /// Fixed point, identical on every frame.
    Fixed(Point),
    /// Uniform straight-line motion from an origin along a velocity per frame.
    Linear {
        /// Position at frame zero.
        origin: Point,
        /// Displacement added per frame.
        velocity: Vector,
    },
    /// Constant-acceleration motion from an origin with an initial velocity.
    Ballistic {
        /// Position at frame zero.
        origin: Point,
        /// Displacement per frame at frame zero.
        velocity: Vector,
        /// Change in velocity added per frame.
        acceleration: Vector,
    },
    /// Uniform circular motion about a centre.
    Circular {
        /// Centre of the circle.
        centre: Point,
        /// Radius of the circle.
        radius: Ratio,
        /// Revolutions completed per frame, in turns.
        turns_per_frame: Ratio,
        /// Angular offset at frame zero, in turns.
        phase: Ratio,
    },
    /// Sinusoidal motion about an origin along an amplitude vector.
    Oscillating {
        /// Midpoint of the oscillation.
        origin: Point,
        /// Peak displacement from the origin.
        amplitude: Vector,
        /// Number of frames per full cycle, strictly positive by construction.
        period: FrameCount,
        /// Phase offset at frame zero, in turns.
        phase: Ratio,
    },
    /// Deterministic pseudo-random walk from an origin.
    Walk {
        /// Position at frame zero.
        origin: Point,
        /// Maximum displacement per frame in scene units.
        step: Ratio,
        /// Base seed; the stream for frame N derives from this seed and N.
        seed: Seed,
    },
    /// Abrupt behaviour changes at frame boundaries.
    ///
    /// Entries must be sorted by frame index with strictly increasing keys.
    /// The motion in force at frame N is the entry with the greatest key less
    /// than or equal to N; frames before the first key hold the first entry.
    Piecewise(Vec<(FrameIndex, Self)>),
}

/// Background of a scene.
///
/// Uniform backgrounds give feature-tracking code nothing to lock onto;
/// [`Background::Blobs`] and [`Background::Grid`] give it structure. Both
/// cases must be producible.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Background {
    /// Uniform fill of a single colour.
    Solid(Rgb8),
    /// Alternating squares of side `cell` pixels, colour `a` at the origin.
    Checker {
        /// Side length of one square in pixels, never zero.
        cell: NonZeroU16,
        /// Colour of the square at the origin.
        a: Rgb8,
        /// Colour of the square adjacent to the origin.
        b: Rgb8,
    },
    /// Linear interpolation from `from` to `to` along `direction`.
    Gradient {
        /// Colour at the start edge.
        from: Rgb8,
        /// Colour at the far edge.
        to: Rgb8,
        /// Axis the interpolation runs along.
        direction: Direction,
    },
    /// Deterministically placed discs over a black ground.
    ///
    /// Centres, radii, and colours derive from `seed`, so the same seed
    /// yields the same discs on every platform. `count` is the number of
    /// discs; radii lie in `[min_radius, max_radius]`.
    Blobs {
        /// Seed for the deterministic disc layout.
        seed: Seed,
        /// Number of discs to place.
        count: u16,
        /// Smallest disc radius in pixels.
        min_radius: u16,
        /// Largest disc radius in pixels.
        max_radius: u16,
    },
    /// Axis-aligned lines of width one over a uniform ground.
    Grid {
        /// Distance between adjacent lines in pixels, never zero.
        spacing: NonZeroU16,
        /// Colour of the lines.
        line: Rgb8,
        /// Colour between the lines.
        ground: Rgb8,
    },
}

/// Shape of an object, centred on its motion position.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Shape {
    /// Disc of the given radius about the centre.
    Disc {
        /// Radius in scene units.
        radius: Ratio,
    },
    /// Axis-aligned rectangle extending `half_width` and `half_height` about the centre.
    Rect {
        /// Half the width in scene units.
        half_width: Ratio,
        /// Half the height in scene units.
        half_height: Ratio,
    },
    /// Closed polygon through the given vertices in scene units.
    Polygon {
        /// Vertices of the polygon in order.
        vertices: Vec<Point>,
    },
    /// Axis-aligned cross centred on the position.
    Cross {
        /// Half-length from the centre to the tip of each bar.
        arm: Ratio,
        /// Full width of each bar.
        thickness: Ratio,
    },
}

/// A single drawable object in a scene.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Object {
    /// Geometry of the object, centred on its motion position.
    pub shape: Shape,
    /// Fill colour of the object.
    pub fill: Rgb8,
    /// Closed-form position as a function of frame.
    pub motion: Motion,
    /// Frames during which the object is drawn.
    pub visible: FrameSpan,
}

impl Object {
    /// Creates an object from its shape, fill, motion, and visibility.
    #[must_use]
    pub const fn new(shape: Shape, fill: Rgb8, motion: Motion, visible: FrameSpan) -> Self {
        Self {
            shape,
            fill,
            motion,
            visible,
        }
    }
}

/// Declarative description of a video sequence.
///
/// Plain data: dimensions, rate, length, an optional physical [`Scale`], a
/// [`Background`], a [`Camera`], and the [`Object`]s drawn in declaration
/// order. Later concerns evaluate motion and draw pixels; this type only
/// holds what they evaluate.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Scene {
    /// Frame dimensions in pixels.
    pub dimensions: Dimensions,
    /// Exact frame rate in frames per second, never stored as a float.
    pub frame_rate: FrameRate,
    /// Number of frames in the sequence.
    pub frame_count: FrameCount,
    /// Optional physical interpretation of scene units.
    pub scale: Option<Scale>,
    /// Backdrop drawn before any object.
    pub background: Background,
    /// Camera transform applied to the whole frame.
    pub camera: Camera,
    /// Objects drawn in declaration order.
    pub objects: Vec<Object>,
}

impl Scene {
    /// Creates a scene from its dimensions, rate, length, scale, backdrop, camera, and objects.
    #[must_use]
    pub const fn new(
        dimensions: Dimensions,
        frame_rate: FrameRate,
        frame_count: FrameCount,
        scale: Option<Scale>,
        background: Background,
        camera: Camera,
        objects: Vec<Object>,
    ) -> Self {
        Self {
            dimensions,
            frame_rate,
            frame_count,
            scale,
            background,
            camera,
            objects,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Magnification, Rotation, Turns, Zoom};
    use crate::testutil::make_ratio;
    use crate::units::{Height, Width};

    /// Builds the fixed point at the origin for motion tests.
    fn origin_point() -> Option<Point> {
        let zero = make_ratio(0, 1)?;
        Some(Point::new(zero, zero))
    }

    /// Builds the identity camera: still at the origin with unit magnification.
    fn fixed_camera() -> Option<Camera> {
        let origin = origin_point()?;
        let angle = Turns::new(make_ratio(0, 1)?);
        let unit = Zoom::Fixed(Magnification::new(make_ratio(1, 1)?)?);
        Some(Camera::new(
            Motion::Fixed(origin),
            Rotation::Fixed(angle),
            unit,
        ))
    }

    /// Builds a one-frame-per-side span from `start` to `end`.
    fn make_span(start: u32, end: u32) -> Option<FrameSpan> {
        FrameSpan::new(FrameIndex::new(start), FrameIndex::new(end))
    }

    #[test]
    fn test_scale_rejects_non_positive() {
        let Some(zero) = make_ratio(0, 1) else { return };
        let Some(neg) = make_ratio(-3, 2) else { return };
        let Some(pos) = make_ratio(3, 2) else { return };
        assert!(Scale::new(zero).is_none(), "zero scale must be rejected");
        assert!(Scale::new(neg).is_none(), "negative scale must be rejected");
        let Some(scale) = Scale::new(pos) else { return };
        assert_eq!(scale.get(), pos, "positive scale must round-trip");
        assert_eq!(scale.ratio(), pos, "scale ratio must round-trip");
        assert_eq!(
            Scale::try_from(pos),
            Ok(scale),
            "TryFrom a positive ratio must succeed"
        );
        assert_eq!(
            Scale::try_from(neg),
            Err(ScaleError::NonPositive),
            "TryFrom a negative ratio must fail"
        );
        assert_eq!(
            Scale::from_ratio(pos),
            Some(scale),
            "from_ratio must match new"
        );
    }

    #[test]
    fn test_frame_span_contains_and_rejects_inverted() {
        let Some(span) = make_span(2, 5) else { return };
        assert!(span.contains(FrameIndex::new(2)), "span must contain start");
        assert!(
            span.contains(FrameIndex::new(4)),
            "span must contain end - 1"
        );
        assert!(!span.contains(FrameIndex::new(5)), "span must exclude end");
        assert!(
            !span.contains(FrameIndex::new(1)),
            "span must exclude before start"
        );
        assert!(make_span(5, 2).is_none(), "inverted span must be rejected");
        assert!(make_span(5, 4).is_none(), "inverted span must be rejected");
        assert!(make_span(5, 5).is_none(), "empty span must be rejected");
        let Some(unit) = make_span(5, 6) else { return };
        assert!(
            unit.contains(FrameIndex::new(5)),
            "unit span must contain start"
        );
        assert!(
            !unit.contains(FrameIndex::new(6)),
            "unit span must exclude end"
        );
    }

    #[test]
    fn test_scene_clone_compares_equal() {
        let Some(width) = Width::new(64) else { return };
        let Some(height) = Height::new(48) else {
            return;
        };
        let dimensions = Dimensions::new(width, height);
        let Some(rate) = FrameRate::from_fps(30) else {
            return;
        };
        let Some(count) = FrameCount::new(4) else {
            return;
        };
        let Some(unit) = make_ratio(2, 1) else { return };
        let Some(scale) = Scale::new(unit) else {
            return;
        };
        let Some(camera) = fixed_camera() else { return };
        let Some(origin) = origin_point() else { return };
        let Some(radius) = make_ratio(5, 1) else {
            return;
        };
        let Some(span) = make_span(0, 4) else { return };
        let red = Rgb8::new(200, 30, 30);
        let disc = Object::new(Shape::Disc { radius }, red, Motion::Fixed(origin), span);
        let scene = Scene::new(
            dimensions,
            rate,
            count,
            Some(scale),
            Background::Solid(red),
            camera,
            vec![disc],
        );
        let cloned = scene.clone();
        assert_eq!(cloned, scene, "a scene must equal its clone");
    }

    /// Builds the shared object list covering every shape variant.
    fn every_shape_objects(green: Rgb8, span: FrameSpan) -> Option<Vec<Object>> {
        let origin = origin_point()?;
        let one = make_ratio(1, 1)?;
        let two = make_ratio(2, 1)?;
        let drift = Motion::Linear {
            origin,
            velocity: Vector::new(one, one),
        };
        let shapes = [
            Shape::Disc { radius: two },
            Shape::Rect {
                half_width: two,
                half_height: one,
            },
            Shape::Polygon {
                vertices: vec![origin, Point::new(one, origin.y), Point::new(one, one)],
            },
            Shape::Cross {
                arm: two,
                thickness: one,
            },
        ];
        let mut objects = Vec::new();
        for shape in shapes {
            objects.push(Object::new(shape, green, drift.clone(), span));
        }
        Some(objects)
    }

    /// Builds every background variant in declaration order.
    fn every_background() -> Option<[Background; 6]> {
        let red = Rgb8::new(255, 0, 0);
        let blue = Rgb8::new(0, 0, 255);
        let white = Rgb8::new(255, 255, 255);
        let black = Rgb8::new(0, 0, 0);
        let cell = NonZeroU16::new(4)?;
        let spacing = NonZeroU16::new(8)?;
        Some([
            Background::Solid(red),
            Background::Checker {
                cell,
                a: red,
                b: blue,
            },
            Background::Gradient {
                from: red,
                to: blue,
                direction: Direction::Horizontal,
            },
            Background::Gradient {
                from: black,
                to: white,
                direction: Direction::Vertical,
            },
            Background::Blobs {
                seed: Seed::new(7),
                count: 5,
                min_radius: 1,
                max_radius: 4,
            },
            Background::Grid {
                spacing,
                line: white,
                ground: black,
            },
        ])
    }

    /// Builds a small scene with the given backdrop and objects.
    fn small_scene(background: Background, objects: Vec<Object>) -> Option<Scene> {
        let width = Width::new(32)?;
        let height = Height::new(32)?;
        let dimensions = Dimensions::new(width, height);
        let rate = FrameRate::from_fps(60)?;
        let count = FrameCount::new(8)?;
        let camera = fixed_camera()?;
        Some(Scene::new(
            dimensions, rate, count, None, background, camera, objects,
        ))
    }

    #[test]
    fn test_scene_backgrounds_round_trip() {
        let Some(backgrounds) = every_background() else {
            return;
        };
        let Some(span) = make_span(0, 8) else { return };
        let green = Rgb8::new(0, 255, 0);
        let Some(objects) = every_shape_objects(green, span) else {
            return;
        };
        for background in backgrounds {
            let Some(scene) = small_scene(background, objects.clone()) else {
                return;
            };
            let cloned = scene.clone();
            assert_eq!(
                cloned, scene,
                "every background variant must round-trip through clone"
            );
        }
    }

    #[test]
    fn test_scene_shapes_round_trip() {
        let Some(span) = make_span(0, 8) else { return };
        let green = Rgb8::new(0, 255, 0);
        let Some(objects) = every_shape_objects(green, span) else {
            return;
        };
        let Some(slice) = objects.get(0..4) else {
            return;
        };
        assert_eq!(slice.len(), 4, "one object per shape variant must exist");
        let Some(backgrounds) = every_background() else {
            return;
        };
        let Some(first) = backgrounds.first().copied() else {
            return;
        };
        let Some(scene) = small_scene(first, objects) else {
            return;
        };
        let cloned = scene.clone();
        assert_eq!(
            cloned, scene,
            "every shape variant must round-trip through clone"
        );
    }

    #[test]
    fn test_scene_inequality() {
        let Some(backgrounds) = every_background() else {
            return;
        };
        let Some(first) = backgrounds.first().copied() else {
            return;
        };
        let Some(span) = make_span(0, 8) else { return };
        let green = Rgb8::new(0, 255, 0);
        let red = Rgb8::new(255, 0, 0);
        let Some(objects) = every_shape_objects(green, span) else {
            return;
        };
        let Some(scene) = small_scene(first, objects) else {
            return;
        };
        let mut altered = scene.clone();
        altered.background = Background::Solid(green);
        assert!(
            altered != scene,
            "changing the background must compare unequal"
        );
        let Some(one) = make_ratio(1, 1) else { return };
        let Some(two) = make_ratio(2, 1) else { return };
        let Some(origin) = origin_point() else { return };
        let still = Motion::Fixed(origin);
        let still_object = Object::new(Shape::Disc { radius: one }, red, still, span);
        assert!(
            still_object.shape != Shape::Disc { radius: two },
            "distinct shapes must compare unequal"
        );
    }

    #[test]
    fn test_motion_variants_clone_equal() {
        let Some(origin) = origin_point() else { return };
        let Some(one) = make_ratio(1, 1) else { return };
        let Some(quarter) = make_ratio(1, 4) else {
            return;
        };
        let Some(count) = FrameCount::new(24) else {
            return;
        };
        let vector = Vector::new(one, one);
        let motions = [
            Motion::Fixed(origin),
            Motion::Linear {
                origin,
                velocity: vector,
            },
            Motion::Ballistic {
                origin,
                velocity: vector,
                acceleration: vector,
            },
            Motion::Circular {
                centre: origin,
                radius: one,
                turns_per_frame: quarter,
                phase: one,
            },
            Motion::Oscillating {
                origin,
                amplitude: vector,
                period: count,
                phase: quarter,
            },
            Motion::Walk {
                origin,
                step: one,
                seed: Seed::new(11),
            },
            Motion::Piecewise(vec![(FrameIndex::new(0), Motion::Fixed(origin))]),
        ];
        for motion in motions {
            let cloned = motion.clone();
            assert_eq!(cloned, motion, "every motion variant must equal its clone");
        }
    }
}
