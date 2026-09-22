//! Camera rotation and magnification as closed-form functions of the frame number.
//!
//! Translation is a position over time and reuses [`Motion`]. Rotation and magnification are not
//! positions: an angle is a single scalar in turns, and a magnification is a
//! strictly positive scalar. Giving each its own type keeps half of a rotation
//! from being silently ignored and makes a non-positive magnification
//! unrepresentable instead of clamped at render time.

use core::fmt;
use core::num::NonZeroI64;

use crate::ratio::{int_ratio, Overflow, Ratio};
use crate::scene::Motion;
use crate::units::{FrameCount, FrameIndex};

/// A rotation angle in turns.
///
/// Every value is meaningful, so construction cannot fail: there is no angle
/// that must be rejected.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Turns(Ratio);

impl Turns {
    /// Creates an angle from its value in turns.
    #[must_use]
    pub const fn new(angle: Ratio) -> Self {
        Self(angle)
    }

    /// Returns the angle in turns.
    #[must_use]
    pub const fn get(self) -> Ratio {
        self.0
    }
}

/// Error returned when attempting to construct a non-positive [`Magnification`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MagnificationError {
    /// Magnification must be strictly positive.
    NonPositive,
}

impl fmt::Display for MagnificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositive => write!(f, "magnification must be strictly positive"),
        }
    }
}

impl core::error::Error for MagnificationError {}

/// A camera magnification.
///
/// The value is strictly positive by construction: [`Magnification::new`]
/// rejects zero and negative ratios, and every [`Zoom`] variant only holds
/// values built that way, so no magnification check is needed downstream.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Magnification(Ratio);

impl Magnification {
    /// Creates a magnification from a [`Ratio`], rejecting zero and negative values.
    ///
    /// # Errors
    ///
    /// Returns `Err(MagnificationError::NonPositive)` when `factor` is zero or negative.
    pub const fn new(factor: Ratio) -> Result<Self, MagnificationError> {
        if factor.numer() > 0 {
            Ok(Self(factor))
        } else {
            Err(MagnificationError::NonPositive)
        }
    }

    /// Returns the magnification factor.
    #[must_use]
    pub const fn get(self) -> Ratio {
        self.0
    }
}

/// Camera rotation as a closed-form function of the frame number.
///
/// Every variant evaluates at a frame index without touching any other frame,
/// so results are identical whether frames run forwards, backwards, or
/// shuffled. Unlike a two-dimensional position, an angle is a single scalar,
/// so nothing is ignored at evaluation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Rotation {
    /// Fixed angle, identical on every frame.
    Fixed(Turns),
    /// Uniform rotation from a starting angle at a constant rate per frame.
    Linear {
        /// Angle at frame zero, in turns.
        start: Turns,
        /// Angle added per frame, in turns.
        per_frame: Turns,
    },
}

/// Camera magnification as a closed-form function of the frame number.
///
/// Every variant evaluates at a frame index without touching any other frame,
/// so results are identical whether frames run forwards, backwards, or
/// shuffled. Both variants hold only [`Magnification`], which is strictly
/// positive by construction, so no [`Zoom`] value can describe a zero or
/// negative magnification at any frame; a reviewer can confirm this from the
/// type alone.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Zoom {
    /// Fixed magnification, identical on every frame.
    Fixed(Magnification),
    /// Linear ramp from one magnification to another over a frame span.
    Ramp {
        /// Magnification at frame zero.
        start: Magnification,
        /// Magnification held at every frame at or beyond `over`.
        end: Magnification,
        /// Number of frames the ramp runs over; never zero by construction.
        over: FrameCount,
    },
}

/// Camera transform of a scene.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Camera {
    /// Translation of the camera centre as a function of frame.
    pub motion: Motion,
    /// Rotation of the camera as a function of frame.
    pub rotation: Rotation,
    /// Magnification of the camera as a function of frame.
    pub zoom: Zoom,
}

impl Camera {
    /// Creates a camera from its translation, rotation, and magnification.
    #[must_use]
    pub const fn new(motion: Motion, rotation: Rotation, zoom: Zoom) -> Self {
        Self {
            motion,
            rotation,
            zoom,
        }
    }
}

/// Returns the frame index as an exact [`Ratio`].
///
/// Every `u32` fits in an `i64`, and `v / 1` is already in lowest terms, so
/// this construction is total.
#[must_use]
fn frame_ratio(frame: FrameIndex) -> Ratio {
    int_ratio(i64::from(frame.get()))
}

/// Evaluates a [`Rotation`] at a frame index, in closed form.
///
/// - [`Rotation::Fixed`] holds its angle on every frame.
/// - `Linear` yields `start + per_frame * n`, where `n` is the frame index.
///
/// The result is a pure function of the frame index, so evaluating frames in any order gives
/// identical values.
///
/// # Errors
///
/// Returns `Err(Overflow)` when exact arithmetic overflows.
pub fn rotation_at(rotation: &Rotation, frame: FrameIndex) -> Result<Turns, Overflow> {
    match rotation {
        Rotation::Fixed(angle) => Ok(*angle),
        Rotation::Linear { start, per_frame } => {
            let step = per_frame.get().checked_mul(frame_ratio(frame))?;
            let result = start.get().checked_add(step)?;
            Ok(Turns::new(result))
        }
    }
}

/// Evaluates a [`Zoom`] at a frame index, in closed form.
///
/// - [`Zoom::Fixed`] holds its magnification on every frame.
/// - `Ramp` yields `start + t * (end - start)` with
///   `t = min(frame, over) / over`.
///
/// The frame count is clamped to `over`, so `t` lies in `[0, 1]` and the
/// result is a convex combination of two strictly positive magnifications,
/// hence strictly positive. `over` is a [`FrameCount`], which is never zero,
/// so the division cannot fail. Because the type only admits positive values,
/// the renderer needs no magnification check downstream.
///
/// The result is a pure function of the frame index, so evaluating frames in any order gives
/// identical values.
///
/// # Errors
///
/// Returns `Err(Overflow)` when exact arithmetic overflows.
pub fn zoom_at(zoom: &Zoom, frame: FrameIndex) -> Result<Magnification, Overflow> {
    match zoom {
        Zoom::Fixed(magnification) => Ok(*magnification),
        Zoom::Ramp { start, end, over } => {
            let span = i64::from(over.get().get());
            let at = i64::from(frame.get().min(over.get().get()));
            let denom = NonZeroI64::new(span).ok_or(Overflow)?;
            let t = Ratio::new(at, denom)?;
            let spread = end.get().checked_sub(start.get())?;
            let grown = t.checked_mul(spread)?;
            let result = start.get().checked_add(grown)?;
            Magnification::new(result).map_err(|_| Overflow)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::make_ratio;

    /// Builds an angle in turns from a numerator and denominator.
    fn make_turns(numer: i64, denom: i64) -> Turns {
        Turns::new(make_ratio(numer, denom).unwrap())
    }

    /// Builds a magnification from a numerator and denominator.
    fn make_magnification(numer: i64, denom: i64) -> Magnification {
        Magnification::new(make_ratio(numer, denom).unwrap()).expect("test magnification")
    }

    /// Builds a ramp from `start` to `end` over `over` frames.
    fn make_ramp(start: Magnification, end: Magnification, over: u32) -> Zoom {
        let span = FrameCount::new(over).unwrap();
        Zoom::Ramp {
            start,
            end,
            over: span,
        }
    }

    #[test]
    fn test_magnification_rejects_zero_and_negative() {
        let zero = make_ratio(0, 1).unwrap();
        let negative = make_ratio(-3, 2).unwrap();
        let positive = make_ratio(3, 2).unwrap();
        assert!(
            Magnification::new(zero).is_err(),
            "zero magnification must be rejected"
        );
        assert!(
            Magnification::new(negative).is_err(),
            "negative magnification must be rejected"
        );
        let magnification = Magnification::new(positive).unwrap();
        assert_eq!(
            magnification.get(),
            positive,
            "positive magnification must round-trip"
        );
    }

    #[test]
    fn test_zoom_ramp_stays_positive_before_inside_after() {
        let start = make_magnification(1, 1);
        let end = make_magnification(3, 1);
        let ramp = make_ramp(start, end, 4);
        let zero = make_ratio(0, 1).unwrap();
        for n in [0_u32, 2, 9] {
            let at = zoom_at(&ramp, FrameIndex::new(n)).expect("zoom should not overflow");
            assert!(
                at.get() > zero,
                "ramp magnification at frame {n} must stay strictly positive"
            );
        }
    }

    #[test]
    fn test_zoom_ramp_endpoints_exact() {
        let start = make_magnification(1, 2);
        let end = make_magnification(5, 1);
        let ramp = make_ramp(start, end, 6);
        assert_eq!(
            zoom_at(&ramp, FrameIndex::new(0)),
            Ok(start),
            "ramp at frame zero must equal its start exactly"
        );
        assert_eq!(
            zoom_at(&ramp, FrameIndex::new(6)),
            Ok(end),
            "ramp at its span must equal its end exactly"
        );
        assert_eq!(
            zoom_at(&ramp, FrameIndex::new(60)),
            Ok(end),
            "ramp past its span must hold its end exactly"
        );
    }

    #[test]
    fn test_zoom_ramp_midpoint_matches_hand_value() {
        let start = make_magnification(1, 1);
        let end = make_magnification(3, 1);
        let ramp = make_ramp(start, end, 4);
        // t = 2 / 4 = 1 / 2, so the value is 1 + (1 / 2) * (3 - 1) = 2.
        let want = make_magnification(2, 1);
        assert_eq!(
            zoom_at(&ramp, FrameIndex::new(2)),
            Ok(want),
            "ramp halfway through its span must equal the midpoint"
        );
    }

    #[test]
    fn test_rotation_linear_matches_hand_table() {
        let start = make_turns(1, 4);
        let per_frame = make_turns(1, 8);
        let spin = Rotation::Linear { start, per_frame };
        // Hand-computed from start + per_frame * n.
        let cases: [(u32, i64, i64); 3] = [(0, 1, 4), (2, 1, 2), (4, 3, 4)];
        for (n, numer, denom) in cases {
            let want = make_turns(numer, denom);
            assert_eq!(
                rotation_at(&spin, FrameIndex::new(n)),
                Ok(want),
                "linear rotation at frame {n} must equal start plus rate times frame"
            );
        }
        let fixed_angle = make_turns(-3, 2);
        let still = Rotation::Fixed(fixed_angle);
        for n in 0..4_u32 {
            assert_eq!(
                rotation_at(&still, FrameIndex::new(n)),
                Ok(fixed_angle),
                "a fixed rotation must hold its angle on every frame"
            );
        }
    }

    #[test]
    fn test_rotation_linear_order_independence() {
        const COUNT: u32 = 25;
        let start = make_turns(1, 4);
        let per_frame = make_turns(1, 8);
        let spin = Rotation::Linear { start, per_frame };
        let mut forward = Vec::new();
        for n in 0..COUNT {
            forward.push(rotation_at(&spin, FrameIndex::new(n)));
        }
        // A fixed stride coprime to COUNT visits every frame exactly once.
        let mut shuffled = Vec::new();
        for n in 0..COUNT {
            let permuted = n.wrapping_mul(7).wrapping_add(3);
            let j = permuted.checked_rem(COUNT).unwrap();
            shuffled.push((j, rotation_at(&spin, FrameIndex::new(j))));
        }
        shuffled.sort_by_key(|entry| entry.0);
        let back_in_order: Vec<Result<Turns, Overflow>> =
            shuffled.iter().map(|entry| entry.1).collect();
        assert_eq!(
            back_in_order, forward,
            "evaluating rotation shuffled must match forwards order"
        );
    }
}
