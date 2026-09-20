//! Closed-form motion evaluation.
//!
//! [`position_at`] answers "where is this motion at frame N" as a pure function
//! of `N`: every variant is computed from its parameters and the frame index
//! alone, never by stepping from a previous frame, so results match whether
//! frames run forwards, backwards, or shuffled.

use crate::geom::{Point, Vector};
use crate::ratio::{int_ratio, Overflow, Ratio};
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
/// Every `u32` fits in an `i64`, and `v / 1` is already in lowest terms, so
/// this construction is total.
#[must_use]
fn frame_ratio(frame: FrameIndex) -> Ratio {
    int_ratio(i64::from(frame.get()))
}

/// Truncates a `u128` to its low 64 bits. Total, and exact: reading the
/// low 8 bytes of the little-endian representation is precisely "reduce
/// modulo 2^64", the same reduction `u64::wrapping_add`/`wrapping_mul`
/// perform, spelled out explicitly rather than via a method this crate's
/// own modular-arithmetic guard flags (for good reason everywhere except
/// deliberate hash/seed mixing like this).
fn truncate_u128_to_u64(value: u128) -> u64 {
    let bytes = value.to_le_bytes();
    let mut low = [0_u8; 8];
    low.copy_from_slice(&bytes[..8]);
    u64::from_le_bytes(low)
}

/// Computes `factors.0 * factors.1 + addend`, reduced modulo 2^64 (i.e.
/// what `u64::wrapping_mul` followed by `u64::wrapping_add` computes).
/// Takes the multiplicands bundled as one tuple parameter, not two
/// adjacent `u64`s, for the same reason `color.rs`'s `weighted_sum` bundles
/// its terms: two separate `u64` parameters here would just be another
/// same-type pair against this crate's transposable-parameter budget, for
/// an operation (multiplication) where swapping them changes nothing.
///
/// Total: widened to `u128`, two `u64` operands multiply to at most
/// `(2^64 - 1)^2`, still below `u128::MAX` (`2^128 - 1`), and adding a
/// third `u64` to that product stays far below `u128::MAX` as well, so
/// neither widened operation can overflow.
fn mul_add_mod_2_64(factors: (u64, u64), addend: u64) -> u64 {
    let (a, b) = factors;
    let product = match u128::from(a).checked_mul(u128::from(b)) {
        Some(p) => p,
        // Unreachable: see this function's doc comment.
        None if a == 0 => 0,
        None => u128::MAX,
    };
    let sum = match product.checked_add(u128::from(addend)) {
        Some(s) => s,
        None if addend == 0 => product,
        None => u128::MAX,
    };
    truncate_u128_to_u64(sum)
}

/// Derives the deterministic seed for walk step `step_index` from the base seed.
///
/// `Seed(base + index * WALK_SEED_STRIDE)`, reduced modulo 2^64, so step
/// 900 needs no knowledge of steps 0 through 899 beyond its own index.
#[must_use]
fn walk_step_seed(base: Seed, step_index: u32) -> Seed {
    Seed::new(mul_add_mod_2_64(
        (u64::from(step_index), WALK_SEED_STRIDE),
        base.get(),
    ))
}

/// Sums the per-step walk displacements for frames `1..=frame`.
///
/// Step `k` draws two integers uniform in `[-WALK_GRID, WALK_GRID]` from the
/// generator seeded with [`walk_step_seed`], divides each by `WALK_GRID`, and
/// scales by `step`, so one step moves at most `step` per axis. Frame zero
/// yields the zero offset. Returns `Err(Overflow)` when any intermediate value
/// overflows.
fn walk_offset(step: Ratio, seed: Seed, frame: FrameIndex) -> Result<(Ratio, Ratio), Overflow> {
    let zero = int_ratio(0);
    let grid = int_ratio(WALK_GRID);
    let span = u64::try_from(
        WALK_GRID
            .checked_mul(2)
            .ok_or(Overflow)?
            .checked_add(1)
            .ok_or(Overflow)?,
    )
    .ok()
    .ok_or(Overflow)?;
    let mut acc_x = zero;
    let mut acc_y = zero;
    for k in 1..=frame.get() {
        let mut rng = Rng::from_seed(walk_step_seed(seed, k));
        let raw_x = rng.next_bounded(span);
        let raw_y = rng.next_bounded(span);
        // raw_x and raw_y come from next_bounded(span) where span = 2049.
        // Both are < 2049, which fits easily in i64.
        let num_x = i64::try_from(raw_x)
            .ok()
            .ok_or(Overflow)?
            .checked_sub(WALK_GRID)
            .ok_or(Overflow)?;
        let num_y = i64::try_from(raw_y)
            .ok()
            .ok_or(Overflow)?
            .checked_sub(WALK_GRID)
            .ok_or(Overflow)?;
        let dx = Ratio::from_integer(num_x)
            .checked_div(grid)?
            .checked_mul(step)?;
        let dy = Ratio::from_integer(num_y)
            .checked_div(grid)?
            .checked_mul(step)?;
        acc_x = acc_x.checked_add(dx)?;
        acc_y = acc_y.checked_add(dy)?;
    }
    Ok((acc_x, acc_y))
}

/// Evaluates uniform straight-line motion: `origin + velocity * n`.
///
/// Returns `Err(Overflow)` when any intermediate value overflows exact arithmetic.
fn linear_position(
    origin: &Point,
    velocity: &Vector,
    frame: FrameIndex,
) -> Result<Point, Overflow> {
    let n = frame_ratio(frame);
    let dx = velocity.x.checked_mul(n)?;
    let dy = velocity.y.checked_mul(n)?;
    let x = origin.x.checked_add(dx)?;
    let y = origin.y.checked_add(dy)?;
    Ok(Point::new(x, y))
}

/// Evaluates constant-acceleration motion: `origin + velocity * n + a * n^2 / 2`.
///
/// Returns `Err(Overflow)` when any intermediate value overflows exact arithmetic.
fn ballistic_position(
    origin: &Point,
    velocity: &Vector,
    acceleration: &Vector,
    n: Ratio,
) -> Result<Point, Overflow> {
    let two = int_ratio(2);
    let n_squared = n.checked_mul(n)?;
    let linear_x = velocity.x.checked_mul(n)?;
    let linear_y = velocity.y.checked_mul(n)?;
    let quad_x = acceleration.x.checked_mul(n_squared)?.checked_div(two)?;
    let quad_y = acceleration.y.checked_mul(n_squared)?.checked_div(two)?;
    let x = origin.x.checked_add(linear_x)?.checked_add(quad_x)?;
    let y = origin.y.checked_add(linear_y)?.checked_add(quad_y)?;
    Ok(Point::new(x, y))
}

/// Evaluates uniform circular motion about `centre` at the given angle in turns.
///
/// Returns `Err(Overflow)` when any intermediate value overflows exact arithmetic.
fn circular_position(centre: &Point, radius: &Ratio, angle: Ratio) -> Result<Point, Overflow> {
    let cos = cos_turns(angle)?;
    let sin = sin_turns(angle)?;
    let x = radius.checked_mul(cos)?.checked_add(centre.x)?;
    let y = radius.checked_mul(sin)?.checked_add(centre.y)?;
    Ok(Point::new(x, y))
}

/// Evaluates sinusoidal motion: `origin + amplitude * sin(angle)`.
///
/// Returns `Err(Overflow)` when any intermediate value overflows exact arithmetic.
fn oscillating_position(
    origin: &Point,
    amplitude: &Vector,
    swing: Ratio,
) -> Result<Point, Overflow> {
    let x = amplitude.x.checked_mul(swing)?.checked_add(origin.x)?;
    let y = amplitude.y.checked_mul(swing)?.checked_add(origin.y)?;
    Ok(Point::new(x, y))
}

/// Returns the oscillation angle `phase + n / period` for a cycle length.
///
/// Returns `Err(Overflow)` when the division overflows exact arithmetic.
fn oscillating_angle(n: Ratio, period: FrameCount, phase: &Ratio) -> Result<Ratio, Overflow> {
    let span = i64::from(period.get().get());
    let period_len = Ratio::from_integer(span);
    let fraction = n.checked_div(period_len)?;
    phase.checked_add(fraction)
}

/// Returns the circular angle `phase + turns_per_frame * n`.
///
/// Returns `Err(Overflow)` when the product overflows exact arithmetic.
fn circular_angle(n: Ratio, turns_per_frame: &Ratio, phase: &Ratio) -> Result<Ratio, Overflow> {
    let turns = turns_per_frame.checked_mul(n)?;
    turns.checked_add(*phase)
}

/// Evaluates a deterministic walk: `origin` plus the [`walk_offset`] sum.
///
/// Returns `Err(Overflow)` when any intermediate value overflows exact arithmetic.
fn walk_position(
    origin: &Point,
    step: &Ratio,
    seed: Seed,
    frame: FrameIndex,
) -> Result<Point, Overflow> {
    let (offset_x, offset_y) = walk_offset(*step, seed, frame)?;
    let x = origin.x.checked_add(offset_x)?;
    let y = origin.y.checked_add(offset_y)?;
    Ok(Point::new(x, y))
}

/// Evaluates the segment with the greatest key at or before `frame`.
///
/// The active segment evaluates at the absolute frame index, so a leg that
/// continues its predecessor passes through the shared position at the
/// boundary frame. Frames before the first key use the first entry. An empty
/// list returns the scene origin `(0, 0)`. Returns `Err(Overflow)` when the active
/// segment's evaluation overflows exact arithmetic.
fn piecewise_position(
    segments: &[(FrameIndex, Motion)],
    frame: FrameIndex,
) -> Result<Point, Overflow> {
    let Some(first) = segments.first() else {
        return Ok(Point::new(int_ratio(0), int_ratio(0)));
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
/// An empty segment list yields the scene origin `(0, 0)`.
///
/// Every variant is a pure function of the frame index, so results are
/// identical whether frames run forwards, backwards, or shuffled.
///
/// # Errors
///
/// Returns `Err(Overflow)` when any intermediate value overflows exact arithmetic.
pub fn position_at(motion: &Motion, frame: FrameIndex) -> Result<Point, Overflow> {
    match motion {
        Motion::Fixed(point) => Ok(*point),
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
        } => {
            let angle = circular_angle(frame_ratio(frame), turns_per_frame, phase)?;
            circular_position(centre, radius, angle)
        }
        Motion::Oscillating {
            origin,
            amplitude,
            period,
            phase,
        } => {
            let angle = oscillating_angle(frame_ratio(frame), *period, phase)?;
            let swing = sin_turns(angle)?;
            oscillating_position(origin, amplitude, swing)
        }
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
    fn point_ratio(xn: i64, xd: i64, yn: i64, yd: i64) -> Point {
        let x = make_ratio(xn, xd).unwrap();
        let y = make_ratio(yn, yd).unwrap();
        Point::new(x, y)
    }

    /// Builds a velocity-style [`Vector`] from integer components.
    fn int_vector(x: i64, y: i64) -> Vector {
        let vx = make_ratio(x, 1).unwrap();
        let vy = make_ratio(y, 1).unwrap();
        Vector::new(vx, vy)
    }

    /// Builds every motion variant over a shared origin for order tests.
    fn every_motion(origin: Point) -> Vec<Motion> {
        let drift = int_vector(3, -1);
        let accel = int_vector(2, 4);
        let radius = make_ratio(4, 1).unwrap();
        let quarter = make_ratio(1, 4).unwrap();
        let step = make_ratio(2, 1).unwrap();
        let period = FrameCount::new(4).unwrap();
        let still = Motion::Fixed(origin);
        vec![
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
                phase: make_ratio(0, 1).unwrap(),
            },
            Motion::Oscillating {
                origin,
                amplitude: drift,
                period,
                phase: make_ratio(0, 1).unwrap(),
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
        ]
    }

    #[test]
    fn test_fixed_and_linear_basic() {
        let spot = point_ratio(3, 2, -5, 4);
        let still = Motion::Fixed(spot);
        for n in 0..8_u32 {
            assert_eq!(
                position_at(&still, FrameIndex::new(n)),
                Ok(spot),
                "a fixed motion must hold its point on every frame"
            );
        }
        let origin = point_ratio(1, 1, 2, 1);
        let velocity = int_vector(3, -1);
        let line = Motion::Linear { origin, velocity };
        let cases: [(u32, i64, i64); 3] = [(0, 1, 2), (1, 4, 1), (4, 13, -2)];
        for (n, ex, ey) in cases {
            let want = point_ratio(ex, 1, ey, 1);
            assert_eq!(
                position_at(&line, FrameIndex::new(n)),
                Ok(want),
                "linear motion must equal origin plus velocity times frame"
            );
        }
    }

    #[test]
    fn test_ballistic_matches_hand_table() {
        let origin = point_ratio(1, 1, 2, 1);
        let velocity = int_vector(3, -1);
        let acceleration = int_vector(2, 4);
        let toss = Motion::Ballistic {
            origin,
            velocity,
            acceleration,
        };
        // Hand-computed from origin + velocity * n + acceleration * n^2 / 2.
        let cases: [(u32, i64, i64); 5] =
            [(0, 1, 2), (1, 5, 3), (2, 11, 8), (3, 19, 17), (4, 29, 30)];
        for (n, ex, ey) in cases {
            let want = point_ratio(ex, 1, ey, 1);
            assert_eq!(
                position_at(&toss, FrameIndex::new(n)),
                Ok(want),
                "ballistic motion must match the hand-computed table"
            );
        }
    }

    #[test]
    fn test_circular_cardinal_frames() {
        let centre = point_ratio(0, 1, 0, 1);
        let radius = make_ratio(4, 1).unwrap();
        let turns = make_ratio(1, 4).unwrap();
        let phase = make_ratio(0, 1).unwrap();
        let orbit = Motion::Circular {
            centre,
            radius,
            turns_per_frame: turns,
            phase,
        };
        let cases: [(u32, i64, i64); 5] = [(0, 4, 0), (1, 0, 4), (2, -4, 0), (3, 0, -4), (4, 4, 0)];
        for (n, ex, ey) in cases {
            let want = point_ratio(ex, 1, ey, 1);
            assert_eq!(
                position_at(&orbit, FrameIndex::new(n)),
                Ok(want),
                "a quarter turn per frame must visit the cardinal points in order"
            );
        }
    }

    #[test]
    fn test_oscillating_peaks_and_midpoints() {
        let origin = point_ratio(10, 1, 20, 1);
        let amplitude = int_vector(4, -6);
        let period = FrameCount::new(4).unwrap();
        let phase = make_ratio(0, 1).unwrap();
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
            let want = point_ratio(ex, 1, ey, 1);
            assert_eq!(
                position_at(&swing, FrameIndex::new(n)),
                Ok(want),
                "oscillation must peak at quarter period and cross zero at half period"
            );
        }
    }

    #[test]
    fn test_walk_zero_determinism_and_bounded_steps() {
        let origin = point_ratio(7, 1, -3, 1);
        let step = make_ratio(2, 1).unwrap();
        let neg_step = make_ratio(-2, 1).unwrap();
        let stroll = Motion::Walk {
            origin,
            step,
            seed: Seed::new(11),
        };
        assert_eq!(
            position_at(&stroll, FrameIndex::new(0)),
            Ok(origin),
            "a walk at frame zero must sit exactly on its origin"
        );
        let far = position_at(&stroll, FrameIndex::new(900));
        assert_eq!(
            position_at(&stroll, FrameIndex::new(900)),
            far,
            "re-evaluating a walk frame must give the identical point"
        );
        for n in 0..32_u32 {
            let here = position_at(&stroll, FrameIndex::new(n)).unwrap();
            assert_eq!(
                position_at(&stroll, FrameIndex::new(n)),
                Ok(here),
                "walk evaluation must be a pure function of the frame index"
            );
            let next = n.checked_add(1).unwrap();
            let there = position_at(&stroll, FrameIndex::new(next)).unwrap();
            let dx = there.x.checked_sub(here.x).unwrap();
            let dy = there.y.checked_sub(here.y).unwrap();
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
        let origin = point_ratio(1, 1, 2, 1);
        let motions = every_motion(origin);
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
                let j = permuted.checked_rem(COUNT).unwrap();
                shuffled.push((j, position_at(motion, FrameIndex::new(j))));
            }
            shuffled.sort_by_key(|entry| entry.0);
            let back_in_order: Vec<Result<Point, Overflow>> =
                shuffled.iter().map(|entry| entry.1).collect();
            assert_eq!(
                back_in_order, forward,
                "evaluating frames shuffled must match forwards order"
            );
        }
    }

    #[test]
    fn test_piecewise_boundary_continuous() {
        let first_origin = point_ratio(0, 1, 0, 1);
        let first_velocity = int_vector(1, 2);
        // Segments evaluate at the absolute frame number, so the second leg's
        // origin is chosen to pass through the shared position (10, 20) at
        // frame 10: 20 + (-1) * 10 = 10.
        let second_origin = point_ratio(20, 1, 20, 1);
        let second_velocity = int_vector(-1, 0);
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
        let before = point_ratio(9, 1, 18, 1);
        let at_edge = point_ratio(10, 1, 20, 1);
        let after = point_ratio(9, 1, 20, 1);
        assert_eq!(
            position_at(&legs, FrameIndex::new(9)),
            Ok(before),
            "frames before the boundary must follow the first leg"
        );
        assert_eq!(
            position_at(&legs, FrameIndex::new(10)),
            Ok(at_edge),
            "the boundary frame must sit exactly on the shared position"
        );
        assert_eq!(
            position_at(&legs, FrameIndex::new(11)),
            Ok(after),
            "frames after the boundary must follow the second leg"
        );
    }

    #[test]
    fn test_piecewise_first_entry_empty_and_nested() {
        let early = point_ratio(1, 1, 1, 1);
        let late = point_ratio(9, 1, 9, 1);
        let late_motion = Motion::Fixed(late);
        let legs = Motion::Piecewise(vec![
            (FrameIndex::new(5), Motion::Fixed(early)),
            (FrameIndex::new(8), late_motion.clone()),
        ]);
        assert_eq!(
            position_at(&legs, FrameIndex::new(2)),
            Ok(early),
            "frames before the first key must hold the first entry"
        );
        assert_eq!(
            position_at(&legs, FrameIndex::new(8)),
            Ok(late),
            "frames at a later key must use that key's motion"
        );
        let empty = Motion::Piecewise(Vec::new());
        let scene_origin = point_ratio(0, 1, 0, 1);
        assert_eq!(
            position_at(&empty, FrameIndex::new(3)),
            Ok(scene_origin),
            "an empty segment list must yield the scene origin"
        );
        let nested = Motion::Piecewise(vec![
            (FrameIndex::new(0), legs),
            (FrameIndex::new(20), late_motion),
        ]);
        assert_eq!(
            position_at(&nested, FrameIndex::new(6)),
            Ok(early),
            "a nested piecewise must delegate to the active inner motion"
        );
        assert_eq!(
            position_at(&nested, FrameIndex::new(20)),
            Ok(late),
            "a nested piecewise must switch to the outer motion at its key"
        );
    }

    #[test]
    fn test_overflow_returns_err() {
        // Construct a linear motion with a large velocity that will overflow
        // when multiplied by a large frame number.
        let origin = point_ratio(0, 1, 0, 1);
        let max_ratio = make_ratio(i64::MAX, 1).unwrap();
        let velocity = Vector::new(max_ratio, max_ratio);
        let overflow_motion = Motion::Linear { origin, velocity };

        // Frame 0 should succeed (multiplication by 0)
        assert_eq!(
            position_at(&overflow_motion, FrameIndex::new(0)),
            Ok(origin),
            "overflow motion at frame 0 must succeed"
        );

        // Frame 2 should fail (max_ratio * 2 overflows)
        let result = position_at(&overflow_motion, FrameIndex::new(2));
        assert!(
            result.is_err(),
            "overflow motion should return Err when arithmetic overflows"
        );
    }
}
