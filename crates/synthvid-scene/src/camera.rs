//! Camera rotation and magnification as closed-form functions of the frame number.
//!
//! Translation is a position over time and reuses [`Motion`]. Rotation and magnification are not
//! positions: an angle is a single scalar in turns, and a magnification is a
//! strictly positive scalar. Giving each its own type keeps half of a rotation
//! from being silently ignored and makes a non-positive magnification
//! unrepresentable instead of clamped at render time.

use core::num::NonZeroI64;

use crate::ratio::{int_ratio, Ratio};
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
    /// Returns `None` when `factor` is zero or negative.
    #[must_use]
    pub const fn new(factor: Ratio) -> Option<Self> {
        if factor.numer() > 0 {
            Some(Self(factor))
        } else {
            None
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
/// Every `u32` fits in an `i64`, so the conversion cannot fail; the fallback
/// is unreachable and exists only because [`Ratio::from_integer`] returns
/// [`Option`].
#[must_use]
fn frame_ratio(frame: FrameIndex) -> Ratio {
    Ratio::from_integer(i64::from(frame.get())).unwrap_or_else(|| int_ratio(0))
}

/// Evaluates a [`Rotation`] at a frame index, in closed form.
///
/// - [`Rotation::Fixed`] holds its angle on every frame.
/// - `Linear` yields `start + per_frame * n`, where `n` is the frame index.
///
/// Returns `None` when exact arithmetic overflows. The result is a pure
/// function of the frame index, so evaluating frames in any order gives
/// identical values.
#[must_use]
pub fn rotation_at(rotation: &Rotation, frame: FrameIndex) -> Option<Turns> {
    match rotation {
        Rotation::Fixed(angle) => Some(*angle),
        Rotation::Linear { start, per_frame } => {
            let step = per_frame.get().checked_mul(frame_ratio(frame))?;
            start.get().checked_add(step).map(Turns::new)
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
/// Returns `None` when exact arithmetic overflows. The result is a pure
/// function of the frame index, so evaluating frames in any order gives
/// identical values.
#[must_use]
pub fn zoom_at(zoom: &Zoom, frame: FrameIndex) -> Option<Magnification> {
    match zoom {
        Zoom::Fixed(magnification) => Some(*magnification),
        Zoom::Ramp { start, end, over } => {
            let span = i64::from(over.get().get());
            let at = i64::from(frame.get().min(over.get().get()));
            let denom = NonZeroI64::new(span)?;
            let t = Ratio::new(at, denom)?;
            let spread = end.get().checked_sub(start.get())?;
            let grown = t.checked_mul(spread)?;
            Magnification::new(start.get().checked_add(grown)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::make_ratio;

    /// Builds an angle in turns from a numerator and denominator.
    fn make_turns(numer: i64, denom: i64) -> Option<Turns> {
        Some(Turns::new(make_ratio(numer, denom)?))
    }

    /// Builds a magnification from a numerator and denominator.
    fn make_magnification(numer: i64, denom: i64) -> Option<Magnification> {
        Magnification::new(make_ratio(numer, denom)?)
    }

    /// Builds a ramp from `start` to `end` over `over` frames.
    fn make_ramp(start: Magnification, end: Magnification, over: u32) -> Option<Zoom> {
        let span = FrameCount::new(over)?;
        Some(Zoom::Ramp {
            start,
            end,
            over: span,
        })
    }

    #[test]
    fn test_magnification_rejects_zero_and_negative() {
        let Some(zero) = make_ratio(0, 1) else { return };
        let Some(negative) = make_ratio(-3, 2) else {
            return;
        };
        let Some(positive) = make_ratio(3, 2) else {
            return;
        };
        assert!(
            Magnification::new(zero).is_none(),
            "zero magnification must be rejected"
        );
        assert!(
            Magnification::new(negative).is_none(),
            "negative magnification must be rejected"
        );
        let Some(magnification) = Magnification::new(positive) else {
            return;
        };
        assert_eq!(
            magnification.get(),
            positive,
            "positive magnification must round-trip"
        );
    }

    #[test]
    fn test_zoom_ramp_stays_positive_before_inside_after() {
        let Some(start) = make_magnification(1, 1) else {
            return;
        };
        let Some(end) = make_magnification(3, 1) else {
            return;
        };
        let Some(ramp) = make_ramp(start, end, 4) else {
            return;
        };
        let Some(zero) = make_ratio(0, 1) else { return };
        for n in [0_u32, 2, 9] {
            let Some(at) = zoom_at(&ramp, FrameIndex::new(n)) else {
                return;
            };
            assert!(
                at.get() > zero,
                "ramp magnification at frame {n} must stay strictly positive"
            );
        }
    }

    #[test]
    fn test_zoom_ramp_endpoints_exact() {
        let Some(start) = make_magnification(1, 2) else {
            return;
        };
        let Some(end) = make_magnification(5, 1) else {
            return;
        };
        let Some(ramp) = make_ramp(start, end, 6) else {
            return;
        };
        assert_eq!(
            zoom_at(&ramp, FrameIndex::new(0)),
            Some(start),
            "ramp at frame zero must equal its start exactly"
        );
        assert_eq!(
            zoom_at(&ramp, FrameIndex::new(6)),
            Some(end),
            "ramp at its span must equal its end exactly"
        );
        assert_eq!(
            zoom_at(&ramp, FrameIndex::new(60)),
            Some(end),
            "ramp past its span must hold its end exactly"
        );
    }

    #[test]
    fn test_zoom_ramp_midpoint_matches_hand_value() {
        let Some(start) = make_magnification(1, 1) else {
            return;
        };
        let Some(end) = make_magnification(3, 1) else {
            return;
        };
        let Some(ramp) = make_ramp(start, end, 4) else {
            return;
        };
        // t = 2 / 4 = 1 / 2, so the value is 1 + (1 / 2) * (3 - 1) = 2.
        let Some(want) = make_magnification(2, 1) else {
            return;
        };
        assert_eq!(
            zoom_at(&ramp, FrameIndex::new(2)),
            Some(want),
            "ramp halfway through its span must equal the midpoint"
        );
    }

    #[test]
    fn test_rotation_linear_matches_hand_table() {
        let Some(start) = make_turns(1, 4) else {
            return;
        };
        let Some(per_frame) = make_turns(1, 8) else {
            return;
        };
        let spin = Rotation::Linear { start, per_frame };
        // Hand-computed from start + per_frame * n.
        let cases: [(u32, i64, i64); 3] = [(0, 1, 4), (2, 1, 2), (4, 3, 4)];
        for (n, numer, denom) in cases {
            let Some(want) = make_turns(numer, denom) else {
                return;
            };
            assert_eq!(
                rotation_at(&spin, FrameIndex::new(n)),
                Some(want),
                "linear rotation at frame {n} must equal start plus rate times frame"
            );
        }
        let Some(fixed_angle) = make_turns(-3, 2) else {
            return;
        };
        let still = Rotation::Fixed(fixed_angle);
        for n in 0..4_u32 {
            assert_eq!(
                rotation_at(&still, FrameIndex::new(n)),
                Some(fixed_angle),
                "a fixed rotation must hold its angle on every frame"
            );
        }
    }

    #[test]
    fn test_rotation_linear_order_independence() {
        const COUNT: u32 = 25;
        let Some(start) = make_turns(1, 4) else {
            return;
        };
        let Some(per_frame) = make_turns(1, 8) else {
            return;
        };
        let spin = Rotation::Linear { start, per_frame };
        let mut forward = Vec::new();
        for n in 0..COUNT {
            forward.push(rotation_at(&spin, FrameIndex::new(n)));
        }
        // A fixed stride coprime to COUNT visits every frame exactly once.
        let mut shuffled = Vec::new();
        for n in 0..COUNT {
            let permuted = n.wrapping_mul(7).wrapping_add(3);
            let j = permuted.checked_rem(COUNT).unwrap_or_default();
            shuffled.push((j, rotation_at(&spin, FrameIndex::new(j))));
        }
        shuffled.sort_by_key(|entry| entry.0);
        let back_in_order: Vec<Option<Turns>> = shuffled.iter().map(|entry| entry.1).collect();
        assert_eq!(
            back_in_order, forward,
            "evaluating rotation shuffled must match forwards order"
        );
    }
}
