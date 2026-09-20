//! Closed-form motion evaluation.
//!
//! [`position_at`] answers "where is this motion at frame N" as a pure function
//! of `N`: every variant is computed from its parameters and the frame index
//! alone, never by stepping from a previous frame, so results match whether
//! frames run forwards, backwards, or shuffled.

use crate::geom::{Point, Vector};
use crate::ratio::{int_ratio, Ratio};
use crate::rng::Rng;
use crate::scene::Motion;
use crate::trig::{cos_turns, sin_turns};
use crate::units::{FrameCount, FrameIndex, Seed};

/// Stride mixed into a walk seed for each step index.
///
/// Successive walk steps use seeds `base + k * STRIDE` with wrapping
/// arithmetic, so every step draws from a distinct, reproducible stream. The
/// value differs deliberately from the generator's own increment so that the
/// tail of one step's stream cannot align with the head of the next.
const WALK_SEED_STRIDE: u64 = 0xBF58_476D_1CE4_E5B9;

/// Denominator of the grid a walk step is quantised to.
///
/// Each step draws integers uniform in `[-WALK_GRID, WALK_GRID]` and divides
/// by `WALK_GRID`, so every displacement has a tiny denominator. Drawing full
/// 62-bit unit ratios instead would force a denominator of `2^62` onto every
/// term, and adding such a term to any origin outside `(-8, 8)` overflows
/// `i64` numerators even though the true sum is small. A power of two keeps
/// accumulated denominators small no matter how many steps are summed.
const WALK_GRID: i64 = 1024;

/// Returns the frame index as an exact [`Ratio`].
///
/// Every `u32` fits in an `i64`, so the conversion cannot fail; the fallback
/// is unreachable and exists only because [`Ratio::from_integer`] returns
/// [`Option`].
#[must_use]
fn frame_ratio(frame: FrameIndex) -> Ratio {
    Ratio::from_integer(i64::from(frame.get())).unwrap_or_else(|| int_ratio(0))
}

/// Derives the deterministic seed for walk step `step_index` from the base seed.
///
/// `Seed(base.wrapping_add(index * WALK_SEED_STRIDE))`, so step 900 needs no
/// knowledge of steps 0 through 899 beyond its own index.
#[must_use]
fn walk_step_seed(base: Seed, step_index: u32) -> Seed {
    let scaled = u64::from(step_index).wrapping_mul(WALK_SEED_STRIDE);
    Seed::new(base.get().wrapping_add(scaled))
}

/// Sums the per-step walk displacements for frames `1..=frame`.
///
/// Step `k` draws two integers uniform in `[-WALK_GRID, WALK_GRID]` from the
/// generator seeded with [`walk_step_seed`], divides each by `WALK_GRID`, and
/// scales by `step`, so one step moves at most `step` per axis. Frame zero
/// yields the zero offset. Returns `None` when any intermediate value
/// overflows.
#[must_use]
fn walk_offset(step: Ratio, seed: Seed, frame: FrameIndex) -> Option<(Ratio, Ratio)> {
    let zero = int_ratio(0);
    let grid = int_ratio(WALK_GRID);
    let span = u64::try_from(WALK_GRID.checked_mul(2)?.checked_add(1)?).ok()?;
    let mut acc_x = zero;
    let mut acc_y = zero;
    for k in 1..=frame.get() {
        let mut rng = Rng::from_seed(walk_step_seed(seed, k));
        let raw_x = rng.next_bounded(span);
        let raw_y = rng.next_bounded(span);
        let num_x = i64::try_from(raw_x)
            .unwrap_or_default()
            .checked_sub(WALK_GRID)?;
        let num_y = i64::try_from(raw_y)
            .unwrap_or_default()
            .checked_sub(WALK_GRID)?;
        let dx = Ratio::from_integer(num_x)?
            .checked_div(grid)?
            .checked_mul(step)?;
        let dy = Ratio::from_integer(num_y)?
            .checked_div(grid)?
            .checked_mul(step)?;
        acc_x = acc_x.checked_add(dx)?;
        acc_y = acc_y.checked_add(dy)?;
    }
    Some((acc_x, acc_y))
}

/// Evaluates uniform straight-line motion: `origin + velocity * n`.
///
/// A coordinate whose intermediate value overflows falls back to the origin
/// coordinate, so the result stays total.
#[must_use]
fn linear_position(origin: &Point, velocity: &Vector, frame: FrameIndex) -> Point {
    let n = frame_ratio(frame);
    let zero = int_ratio(0);
    let dx = velocity.x.checked_mul(n).unwrap_or(zero);
    let dy = velocity.y.checked_mul(n).unwrap_or(zero);
    let x = origin.x.checked_add(dx).unwrap_or(origin.x);
    let y = origin.y.checked_add(dy).unwrap_or(origin.y);
    Point::new(x, y)
}

/// Evaluates constant-acceleration motion: `origin + velocity * n + a * n^2 / 2`.
///
/// A coordinate whose intermediate value overflows falls back to the origin
/// coordinate, so the result stays total.
#[must_use]
fn ballistic_position(origin: &Point, velocity: &Vector, acceleration: &Vector, n: Ratio) -> Point {
    let zero = int_ratio(0);
    let two = int_ratio(2);
    let n_squared = n.checked_mul(n).unwrap_or(zero);
    let linear_x = velocity.x.checked_mul(n).unwrap_or(zero);
    let linear_y = velocity.y.checked_mul(n).unwrap_or(zero);
    let quad_x = acceleration
        .x
        .checked_mul(n_squared)
        .and_then(|scaled| scaled.checked_div(two))
        .unwrap_or(zero);
    let quad_y = acceleration
        .y
        .checked_mul(n_squared)
        .and_then(|scaled| scaled.checked_div(two))
        .unwrap_or(zero);
    let x = origin
        .x
        .checked_add(linear_x)
        .and_then(|partial| partial.checked_add(quad_x))
        .unwrap_or(origin.x);
    let y = origin
        .y
        .checked_add(linear_y)
        .and_then(|partial| partial.checked_add(quad_y))
        .unwrap_or(origin.y);
    Point::new(x, y)
}

/// Evaluates uniform circular motion about `centre` at the given angle in turns.
///
/// A coordinate whose intermediate value overflows falls back to the centre
/// coordinate, so the result stays total.
#[must_use]
fn circular_position(centre: &Point, radius: &Ratio, angle: Ratio) -> Point {
    let cos = cos_turns(angle);
    let sin = sin_turns(angle);
    let x = radius
        .checked_mul(cos)
        .and_then(|dx| centre.x.checked_add(dx))
        .unwrap_or(centre.x);
    let y = radius
        .checked_mul(sin)
        .and_then(|dy| centre.y.checked_add(dy))
        .unwrap_or(centre.y);
    Point::new(x, y)
}

/// Evaluates sinusoidal motion: `origin + amplitude * sin(angle)`.
///
/// A coordinate whose intermediate value overflows falls back to the origin
/// coordinate, so the result stays total.
#[must_use]
fn oscillating_position(origin: &Point, amplitude: &Vector, swing: Ratio) -> Point {
    let x = amplitude
        .x
        .checked_mul(swing)
        .and_then(|dx| origin.x.checked_add(dx))
        .unwrap_or(origin.x);
    let y = amplitude
        .y
        .checked_mul(swing)
        .and_then(|dy| origin.y.checked_add(dy))
        .unwrap_or(origin.y);
    Point::new(x, y)
}

/// Returns the oscillation angle `phase + n / period` for a cycle length.
///
/// Falls back to `phase` when the division overflows, so the result stays total.
#[must_use]
fn oscillating_angle(n: Ratio, period: FrameCount, phase: &Ratio) -> Ratio {
    let one = int_ratio(1);
    let period_len = Ratio::from_integer(i64::from(period.get().get())).unwrap_or(one);
    n.checked_div(period_len)
        .and_then(|fraction| phase.checked_add(fraction))
        .unwrap_or(*phase)
}

/// Returns the circular angle `phase + turns_per_frame * n`.
///
/// Falls back to `phase` when the product overflows, so the result stays total.
#[must_use]
fn circular_angle(n: Ratio, turns_per_frame: &Ratio, phase: &Ratio) -> Ratio {
    turns_per_frame
        .checked_mul(n)
        .and_then(|turns| turns.checked_add(*phase))
        .unwrap_or(*phase)
}

/// Evaluates a deterministic walk: `origin` plus the [`walk_offset`] sum.
///
/// A coordinate whose accumulation overflows falls back to the origin
/// coordinate, so the result stays total.
#[must_use]
fn walk_position(origin: &Point, step: &Ratio, seed: Seed, frame: FrameIndex) -> Point {
    let zero = int_ratio(0);
    let offset = walk_offset(*step, seed, frame).unwrap_or((zero, zero));
    let x = origin.x.checked_add(offset.0).unwrap_or(origin.x);
    let y = origin.y.checked_add(offset.1).unwrap_or(origin.y);
    Point::new(x, y)
}

/// Evaluates the segment with the greatest key at or before `frame`.
///
/// The active segment evaluates at the absolute frame index, so a leg that
/// continues its predecessor passes through the shared position at the
/// boundary frame. Frames before the first key use the first entry. An empty
/// list has no motion to evaluate and yields the scene origin `(0, 0)` instead.
#[must_use]
fn piecewise_position(segments: &[(FrameIndex, Motion)], frame: FrameIndex) -> Point {
    let Some(first) = segments.first() else {
        return Point::new(int_ratio(0), int_ratio(0));
    };
    let mut active: &Motion = &first.1;
    for entry in segments {
        if entry.0.get() <= frame.get() {
            active = &entry.1;
        } else {
            break;
        }
    }
    position_at(active, frame)
}

/// Evaluates a [`Motion`] at a frame index, in closed form.
///
/// - [`Motion::Fixed`] holds its point on every frame.
/// - `Linear` yields `origin + velocity * n`.
/// - `Ballistic` yields `origin + velocity * n + acceleration * n^2 / 2`.
/// - `Circular` yields `centre + radius * (cos(angle), sin(angle))` with
///   `angle = phase + turns_per_frame * n`, using [`cos_turns`] and
///   [`sin_turns`].
/// - `Oscillating` yields `origin + amplitude * sin(angle)` with
///   `angle = phase + n / period`, where `period` is the cycle length, using
///   [`sin_turns`].
/// - `Walk` yields `origin` plus the re-seeded per-step sum, so frame 900
///   never requires frame 899.
/// - `Piecewise` evaluates the entry with the greatest key less than or equal
///   to the frame; frames before the first key use the first entry.
///
/// The function is total: arithmetic is checked, and a coordinate whose
/// intermediate value overflows falls back to its anchor (`origin` or
/// `centre`), which lies far outside any renderable scene. An empty segment
/// list has no anchor and yields the scene origin `(0, 0)`.
///
/// Every variant is a pure function of the frame index, so results are
/// identical whether frames run forwards, backwards, or shuffled.
#[must_use]
pub fn position_at(motion: &Motion, frame: FrameIndex) -> Point {
    match motion {
        Motion::Fixed(point) => *point,
        Motion::Linear { origin, velocity } => linear_position(origin, velocity, frame),
        Motion::Ballistic {
            origin,
            velocity,
            acceleration,
        } => ballistic_position(origin, velocity, acceleration, frame_ratio(frame)),
        Motion::Circular {
            centre,
            radius,
            turns_per_frame,
            phase,
        } => circular_position(
            centre,
            radius,
            circular_angle(frame_ratio(frame), turns_per_frame, phase),
        ),
        Motion::Oscillating {
            origin,
            amplitude,
            period,
            phase,
        } => oscillating_position(
            origin,
            amplitude,
            sin_turns(oscillating_angle(frame_ratio(frame), *period, phase)),
        ),
        Motion::Walk { origin, step, seed } => walk_position(origin, step, *seed, frame),
        Motion::Piecewise(segments) => piecewise_position(segments, frame),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Vector;
    use crate::testutil::make_ratio;
    use crate::units::FrameCount;

    /// Builds the point `(xn / xd, yn / yd)` for motion tests.
    fn point_ratio(xn: i64, xd: i64, yn: i64, yd: i64) -> Option<Point> {
        let x = make_ratio(xn, xd)?;
        let y = make_ratio(yn, yd)?;
        Some(Point::new(x, y))
    }

    /// Builds a velocity-style [`Vector`] from integer components.
    fn int_vector(x: i64, y: i64) -> Option<Vector> {
        let vx = make_ratio(x, 1)?;
        let vy = make_ratio(y, 1)?;
        Some(Vector::new(vx, vy))
    }

    /// Builds every motion variant over a shared origin for order tests.
    fn every_motion(origin: Point) -> Option<Vec<Motion>> {
        let drift = int_vector(3, -1)?;
        let accel = int_vector(2, 4)?;
        let radius = make_ratio(4, 1)?;
        let quarter = make_ratio(1, 4)?;
        let step = make_ratio(2, 1)?;
        let period = FrameCount::new(4)?;
        let still = Motion::Fixed(origin);
        Some(vec![
            still.clone(),
            Motion::Linear {
                origin,
                velocity: drift,
            },
            Motion::Ballistic {
                origin,
                velocity: drift,
                acceleration: accel,
            },
            Motion::Circular {
                centre: origin,
                radius,
                turns_per_frame: quarter,
                phase: make_ratio(0, 1)?,
            },
            Motion::Oscillating {
                origin,
                amplitude: drift,
                period,
                phase: make_ratio(0, 1)?,
            },
            Motion::Walk {
                origin,
                step,
                seed: Seed::new(11),
            },
            Motion::Piecewise(vec![
                (
                    FrameIndex::new(0),
                    Motion::Linear {
                        origin,
                        velocity: drift,
                    },
                ),
                (FrameIndex::new(10), still),
            ]),
        ])
    }

    #[test]
    fn test_fixed_and_linear_basic() {
        let Some(spot) = point_ratio(3, 2, -5, 4) else {
            return;
        };
        let still = Motion::Fixed(spot);
        for n in 0..8_u32 {
            assert_eq!(
                position_at(&still, FrameIndex::new(n)),
                spot,
                "a fixed motion must hold its point on every frame"
            );
        }
        let Some(origin) = point_ratio(1, 1, 2, 1) else {
            return;
        };
        let Some(velocity) = int_vector(3, -1) else {
            return;
        };
        let line = Motion::Linear { origin, velocity };
        let cases: [(u32, i64, i64); 3] = [(0, 1, 2), (1, 4, 1), (4, 13, -2)];
        for (n, ex, ey) in cases {
            let Some(want) = point_ratio(ex, 1, ey, 1) else {
                return;
            };
            assert_eq!(
                position_at(&line, FrameIndex::new(n)),
                want,
                "linear motion must equal origin plus velocity times frame"
            );
        }
    }

    #[test]
    fn test_ballistic_matches_hand_table() {
        let Some(origin) = point_ratio(1, 1, 2, 1) else {
            return;
        };
        let Some(velocity) = int_vector(3, -1) else {
            return;
        };
        let Some(acceleration) = int_vector(2, 4) else {
            return;
        };
        let toss = Motion::Ballistic {
            origin,
            velocity,
            acceleration,
        };
        // Hand-computed from origin + velocity * n + acceleration * n^2 / 2.
        let cases: [(u32, i64, i64); 5] =
            [(0, 1, 2), (1, 5, 3), (2, 11, 8), (3, 19, 17), (4, 29, 30)];
        for (n, ex, ey) in cases {
            let Some(want) = point_ratio(ex, 1, ey, 1) else {
                return;
            };
            assert_eq!(
                position_at(&toss, FrameIndex::new(n)),
                want,
                "ballistic motion must match the hand-computed table"
            );
        }
    }

    #[test]
    fn test_circular_cardinal_frames() {
        let Some(centre) = point_ratio(0, 1, 0, 1) else {
            return;
        };
        let Some(radius) = make_ratio(4, 1) else {
            return;
        };
        let Some(turns) = make_ratio(1, 4) else {
            return;
        };
        let Some(phase) = make_ratio(0, 1) else {
            return;
        };
        let orbit = Motion::Circular {
            centre,
            radius,
            turns_per_frame: turns,
            phase,
        };
        let cases: [(u32, i64, i64); 5] = [(0, 4, 0), (1, 0, 4), (2, -4, 0), (3, 0, -4), (4, 4, 0)];
        for (n, ex, ey) in cases {
            let Some(want) = point_ratio(ex, 1, ey, 1) else {
                return;
            };
            assert_eq!(
                position_at(&orbit, FrameIndex::new(n)),
                want,
                "a quarter turn per frame must visit the cardinal points in order"
            );
        }
    }

    #[test]
    fn test_oscillating_peaks_and_midpoints() {
        let Some(origin) = point_ratio(10, 1, 20, 1) else {
            return;
        };
        let Some(amplitude) = int_vector(4, -6) else {
            return;
        };
        let Some(period) = FrameCount::new(4) else {
            return;
        };
        let Some(phase) = make_ratio(0, 1) else {
            return;
        };
        let swing = Motion::Oscillating {
            origin,
            amplitude,
            period,
            phase,
        };
        let cases: [(u32, i64, i64); 5] = [
            (0, 10, 20),
            (1, 14, 14),
            (2, 10, 20),
            (3, 6, 26),
            (4, 10, 20),
        ];
        for (n, ex, ey) in cases {
            let Some(want) = point_ratio(ex, 1, ey, 1) else {
                return;
            };
            assert_eq!(
                position_at(&swing, FrameIndex::new(n)),
                want,
                "oscillation must peak at quarter period and cross zero at half period"
            );
        }
    }

    #[test]
    fn test_walk_zero_determinism_and_bounded_steps() {
        let Some(origin) = point_ratio(7, 1, -3, 1) else {
            return;
        };
        let Some(step) = make_ratio(2, 1) else {
            return;
        };
        let Some(neg_step) = make_ratio(-2, 1) else {
            return;
        };
        let stroll = Motion::Walk {
            origin,
            step,
            seed: Seed::new(11),
        };
        assert_eq!(
            position_at(&stroll, FrameIndex::new(0)),
            origin,
            "a walk at frame zero must sit exactly on its origin"
        );
        let far = position_at(&stroll, FrameIndex::new(900));
        assert_eq!(
            position_at(&stroll, FrameIndex::new(900)),
            far,
            "re-evaluating a walk frame must give the identical point"
        );
        for n in 0..32_u32 {
            let here = position_at(&stroll, FrameIndex::new(n));
            assert_eq!(
                position_at(&stroll, FrameIndex::new(n)),
                here,
                "walk evaluation must be a pure function of the frame index"
            );
            let Some(next) = n.checked_add(1) else {
                return;
            };
            let there = position_at(&stroll, FrameIndex::new(next));
            let Some(dx) = there.x.checked_sub(here.x) else {
                return;
            };
            let Some(dy) = there.y.checked_sub(here.y) else {
                return;
            };
            assert!(
                dx >= neg_step && dx <= step,
                "one walk step in x must stay within the declared bound"
            );
            assert!(
                dy >= neg_step && dy <= step,
                "one walk step in y must stay within the declared bound"
            );
        }
    }

    #[test]
    fn test_order_independence_forward_backward_shuffled() {
        const COUNT: u32 = 25;
        let Some(origin) = point_ratio(1, 1, 2, 1) else {
            return;
        };
        let Some(motions) = every_motion(origin) else {
            return;
        };
        for motion in &motions {
            let mut forward = Vec::new();
            for n in 0..COUNT {
                forward.push(position_at(motion, FrameIndex::new(n)));
            }
            let mut backward = Vec::new();
            for n in (0..COUNT).rev() {
                backward.push(position_at(motion, FrameIndex::new(n)));
            }
            backward.reverse();
            assert_eq!(
                backward, forward,
                "evaluating frames backwards must match forwards order"
            );
            // A fixed stride coprime to COUNT visits every frame exactly once.
            let mut shuffled = Vec::new();
            for n in 0..COUNT {
                let permuted = n.wrapping_mul(7).wrapping_add(3);
                let j = permuted.checked_rem(COUNT).unwrap_or_default();
                shuffled.push((j, position_at(motion, FrameIndex::new(j))));
            }
            shuffled.sort_by_key(|entry| entry.0);
            let back_in_order: Vec<Point> = shuffled.iter().map(|entry| entry.1).collect();
            assert_eq!(
                back_in_order, forward,
                "evaluating frames shuffled must match forwards order"
            );
        }
    }

    #[test]
    fn test_piecewise_boundary_continuous() {
        let Some(first_origin) = point_ratio(0, 1, 0, 1) else {
            return;
        };
        let Some(first_velocity) = int_vector(1, 2) else {
            return;
        };
        // Segments evaluate at the absolute frame number, so the second leg's
        // origin is chosen to pass through the shared position (10, 20) at
        // frame 10: 20 + (-1) * 10 = 10.
        let Some(second_origin) = point_ratio(20, 1, 20, 1) else {
            return;
        };
        let Some(second_velocity) = int_vector(-1, 0) else {
            return;
        };
        let legs = Motion::Piecewise(vec![
            (
                FrameIndex::new(0),
                Motion::Linear {
                    origin: first_origin,
                    velocity: first_velocity,
                },
            ),
            (
                FrameIndex::new(10),
                Motion::Linear {
                    origin: second_origin,
                    velocity: second_velocity,
                },
            ),
        ]);
        let Some(before) = point_ratio(9, 1, 18, 1) else {
            return;
        };
        let Some(at_edge) = point_ratio(10, 1, 20, 1) else {
            return;
        };
        let Some(after) = point_ratio(9, 1, 20, 1) else {
            return;
        };
        assert_eq!(
            position_at(&legs, FrameIndex::new(9)),
            before,
            "frames before the boundary must follow the first leg"
        );
        assert_eq!(
            position_at(&legs, FrameIndex::new(10)),
            at_edge,
            "the boundary frame must sit exactly on the shared position"
        );
        assert_eq!(
            position_at(&legs, FrameIndex::new(11)),
            after,
            "frames after the boundary must follow the second leg"
        );
    }

    #[test]
    fn test_piecewise_first_entry_empty_and_nested() {
        let Some(early) = point_ratio(1, 1, 1, 1) else {
            return;
        };
        let Some(late) = point_ratio(9, 1, 9, 1) else {
            return;
        };
        let late_motion = Motion::Fixed(late);
        let legs = Motion::Piecewise(vec![
            (FrameIndex::new(5), Motion::Fixed(early)),
            (FrameIndex::new(8), late_motion.clone()),
        ]);
        assert_eq!(
            position_at(&legs, FrameIndex::new(2)),
            early,
            "frames before the first key must hold the first entry"
        );
        assert_eq!(
            position_at(&legs, FrameIndex::new(8)),
            late,
            "frames at a later key must use that key's motion"
        );
        let empty = Motion::Piecewise(Vec::new());
        let Some(scene_origin) = point_ratio(0, 1, 0, 1) else {
            return;
        };
        assert_eq!(
            position_at(&empty, FrameIndex::new(3)),
            scene_origin,
            "an empty segment list must yield the scene origin"
        );
        let nested = Motion::Piecewise(vec![
            (FrameIndex::new(0), legs),
            (FrameIndex::new(20), late_motion),
        ]);
        assert_eq!(
            position_at(&nested, FrameIndex::new(6)),
            early,
            "a nested piecewise must delegate to the active inner motion"
        );
        assert_eq!(
            position_at(&nested, FrameIndex::new(20)),
            late,
            "a nested piecewise must switch to the outer motion at its key"
        );
    }
}
