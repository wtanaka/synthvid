//! Scene description.
//!
//! Plain data and nothing else. [`Background`], [`Shape`], [`Object`], and
//! [`Scene`] describe the world; evaluation of motion and drawing of pixels
//! belong to later concerns and live elsewhere.
//! Every type here is [`Clone`] and [`PartialEq`], so a future canonical text
//! form can round-trip a scene and compare the result with [`Eq`].

mod blobs;
mod membership;
mod scale;

use core::error::Error;
use core::fmt;
use core::num::NonZeroU16;
use core::ops::Range;

use crate::color::Rgb8;
use crate::geom::{Point, Vector};
use crate::ratio::Ratio;
use crate::units::{FrameCount, FrameIndex, Seed};

pub use blobs::{BlobCount, BlobRadii, BlobRadius};
pub use membership::Membership;
pub use scale::{Scale, ScaleError};

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

/// Error returned when attempting to construct an invalid [`FrameSpan`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FrameSpanError {
    /// Start frame must be strictly before the end frame.
    EndBeforeStart,
}

impl fmt::Display for FrameSpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EndBeforeStart => write!(f, "start frame must be strictly before the end frame"),
        }
    }
}

impl Error for FrameSpanError {}

/// Inclusive-start, exclusive-end span of frames during which an object is visible.
///
/// Every span covers at least one frame. The bounds are private, so the only way to
/// build a span is [`FrameSpan::new`], which refuses an empty or inverted one.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FrameSpan {
    /// First visible frame index, inclusive.
    start: FrameIndex,
    /// One past the last visible frame index, exclusive.
    end: FrameIndex,
}

impl FrameSpan {
    /// Creates a frame span covering `frames`, from its start (inclusive) to
    /// its end (exclusive).
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameSpanError::EndBeforeStart)` when the start lies at or after the end.
    pub const fn new(frames: Range<FrameIndex>) -> Result<Self, FrameSpanError> {
        let Range { start, end } = frames;
        if start.get() >= end.get() {
            Err(FrameSpanError::EndBeforeStart)
        } else {
            Ok(Self { start, end })
        }
    }

    /// The first visible frame index, inclusive.
    #[must_use]
    pub const fn start(self) -> FrameIndex {
        self.start
    }

    /// One past the last visible frame index, exclusive.
    #[must_use]
    pub const fn end(self) -> FrameIndex {
        self.end
    }

    /// Returns [`Membership::Inside`] when `frame` lies in `[start, end)`.
    #[must_use]
    pub const fn contains(self, frame: FrameIndex) -> Membership {
        if frame.get() >= self.start.get() && frame.get() < self.end.get() {
            Membership::Inside
        } else {
            Membership::Outside
        }
    }
}

/// Position of an object or view property as a closed-form function of frame.
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
    /// Abrupt behavior changes at frame boundaries.
    ///
    /// Entries must be sorted by frame index with strictly increasing keys.
    /// The motion in force at frame N is the entry with the greatest key less
    /// than or equal to N; frames before the first key hold the first entry.
    Piecewise(Vec<(FrameIndex, Self)>),
}

/// Background of a scene.
///
/// Uniform backgrounds carry no spatial structure; [`Background::Blobs`]
/// and [`Background::Grid`] carry a great deal. Both cases must be
/// producible.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Background {
    /// Uniform fill of a single color.
    Solid(Rgb8),
    /// Alternating squares of side `cell` pixels, color `even` at the origin.
    Checker {
        /// Side length of one square in pixels, never zero.
        cell: NonZeroU16,
        /// Color of the square at the origin.
        even: Rgb8,
        /// Color of the square adjacent to the origin.
        odd: Rgb8,
    },
    /// Linear interpolation from `from` to `to` along `direction`.
    Gradient {
        /// Color at the start edge.
        from: Rgb8,
        /// Color at the far edge.
        to: Rgb8,
        /// Axis the interpolation runs along.
        direction: Direction,
    },
    /// Deterministically placed discs over a black ground.
    ///
    /// Centers, radii, and colors derive from `seed`, so the same seed
    /// yields the same discs on every platform. `count` is the number of
    /// discs; radii lie in the inclusive range `radii`.
    Blobs {
        /// Seed for the deterministic disc layout.
        seed: Seed,
        /// Number of discs to place.
        count: BlobCount,
        /// Inclusive range of disc radii in pixels.
        radii: BlobRadii,
    },
    /// Axis-aligned lines of width one over a uniform ground.
    Grid {
        /// Distance between adjacent lines in pixels, never zero.
        spacing: NonZeroU16,
        /// Color of the lines.
        line: Rgb8,
        /// Color between the lines.
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
    /// Fill color of the object.
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

/// Declarative description of the world a video shows.
///
/// Plain data: an optional physical [`Scale`], a [`Background`], and the
/// [`Object`]s drawn in declaration order. The view, the frame dimensions, and
/// the timing belong to the recording, not the world, so they are supplied
/// when a frame is rendered. Later concerns evaluate motion and draw pixels;
/// this type only holds what they evaluate. A Scene describes sensible content
/// and nothing here validates that; output that is deliberately nonsensical
/// belongs in the video description, not the scene.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Scene {
    /// Optional physical interpretation of scene units.
    pub scale: Option<Scale>,
    /// Backdrop drawn before any object.
    pub background: Background,
    /// Objects drawn in declaration order.
    pub objects: Vec<Object>,
}

#[cfg(test)]
mod tests;
