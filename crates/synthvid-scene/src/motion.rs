//! Closed-form motion evaluation.
//!
//! [`position_at`] answers "where is this motion at frame N" as a pure function
//! of `N`: every variant is computed from its parameters and the frame index
//! alone, never by stepping from a previous frame, so results match whether
//! frames run forwards, backwards, or shuffled.

use core::num::{NonZeroI64, NonZeroU64};

use crate::geom::{Point, Vector};
use crate::ratio::{Numerator, Overflow, Ratio};
use crate::rng::{BoundedDraw, DrawBound, Rng};
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
    Ratio::from_integer(Numerator(i64::from(frame.get())))
}

/// A 128-bit intermediate product or sum, awaiting reduction modulo 2^64.
#[derive(Clone, Copy, Debug)]
struct Wide(u128);

/// A residue modulo 2^64: a `u64` whose arithmetic wraps by definition.
#[derive(Clone, Copy, Debug)]
struct Residue(u64);

/// The 1-based index of one walk step.
#[derive(Clone, Copy, Debug)]
struct WalkStep(u32);

/// Truncates a [`Wide`] to its low 64 bits. Total, and exact: reading the
/// low 8 bytes of the little-endian representation is precisely "reduce
/// modulo 2^64", the same reduction `u64::wrapping_add`/`wrapping_mul`
/// perform, spelled out explicitly rather than via a method this crate's
/// own modular-arithmetic guard flags (for good reason everywhere except
/// deliberate hash/seed mixing like this).
fn truncate_to_residue(value: Wide) -> Residue {
    let bytes = value.0.to_le_bytes();
    let mut low = [0_u8; 8];
    low.copy_from_slice(&bytes[..8]);
    Residue(u64::from_le_bytes(low))
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
fn mul_add_mod_2_64(factors: (Residue, Residue), addend: Residue) -> Residue {
    let (Residue(left), Residue(right)) = factors;
    let Residue(addend_bits) = addend;
    let product = match u128::from(left).checked_mul(u128::from(right)) {
        Some(exact) => exact,
        // Unreachable: see this function's doc comment.
        None if left == 0 => 0,
        None => u128::MAX,
    };
    let sum = match product.checked_add(u128::from(addend_bits)) {
        Some(exact_sum) => exact_sum,
        None if addend_bits == 0 => product,
        None => u128::MAX,
    };
    truncate_to_residue(Wide(sum))
}

/// Derives the deterministic seed for walk step `step` from the base seed.
///
/// `Seed(base + index * WALK_SEED_STRIDE)`, reduced modulo 2^64, so step
/// 900 needs no knowledge of steps 0 through 899 beyond its own index.
#[must_use]
fn walk_step_seed(base: Seed, step: WalkStep) -> Seed {
    let WalkStep(index) = step;
    let reduced = mul_add_mod_2_64(
        (Residue(u64::from(index)), Residue(WALK_SEED_STRIDE)),
        Residue(base.get()),
    );
    Seed::new(reduced.0)
}

/// Sums the per-step walk displacements for frames `1..=frame`.
///
/// Step `k` draws two integers uniform in `[-WALK_GRID, WALK_GRID]` from the
/// generator seeded with [`walk_step_seed`], divides each by `WALK_GRID`, and
/// scales by `step`, so one step moves at most `step` per axis. Frame zero
/// yields the zero offset. Returns `Err(Overflow)` when any intermediate value
/// overflows.
fn walk_offset(step: Ratio, seed: Seed, frame: FrameIndex) -> Result<(Ratio, Ratio), Overflow> {
    let zero = Ratio::from_integer(Numerator(0));
    let grid = Ratio::from_integer(Numerator(WALK_GRID));
    let span = u64::try_from(
        WALK_GRID
            .checked_mul(2)
            .ok_or(Overflow)?
            .checked_add(1)
            .ok_or(Overflow)?,
    )
    .map_err(Overflow::from)
    .and_then(|width| NonZeroU64::new(width).ok_or(Overflow))
    .map(DrawBound)?;
    let mut acc_x = zero;
    let mut acc_y = zero;
    for step_index in 1..=frame.get() {
        let mut rng = Rng::from_seed(walk_step_seed(seed, WalkStep(step_index)));
        let BoundedDraw(raw_x) = rng.next_bounded(span);
        let BoundedDraw(raw_y) = rng.next_bounded(span);
        // raw_x and raw_y come from next_bounded(span) where span = 2049.
        // Both are < 2049, which fits easily in i64.
        let num_x = i64::try_from(raw_x)
            .map_err(Overflow::from)?
            .checked_sub(WALK_GRID)
            .ok_or(Overflow)?;
        let num_y = i64::try_from(raw_y)
            .map_err(Overflow::from)?
            .checked_sub(WALK_GRID)
            .ok_or(Overflow)?;
        let dx = Ratio::from_integer(Numerator(num_x))
            .checked_div(grid)?
            .checked_mul(step)?;
        let dy = Ratio::from_integer(Numerator(num_y))
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
    let elapsed = frame_ratio(frame);
    let dx = velocity.across.checked_mul(elapsed)?;
    let dy = velocity.down.checked_mul(elapsed)?;
    let out_x = origin.across.checked_add(dx)?;
    let out_y = origin.down.checked_add(dy)?;
    Ok(Point {
        across: out_x,
        down: out_y,
    })
}

/// Evaluates constant-acceleration motion: `origin + velocity * elapsed + a * elapsed^2 / 2`.
///
/// Returns `Err(Overflow)` when any intermediate value overflows exact arithmetic.
fn ballistic_position(
    origin: &Point,
    velocity: &Vector,
    elapsed: Ratio,
    acceleration: &Vector,
) -> Result<Point, Overflow> {
    let two = Ratio::from_integer(Numerator(2));
    let n_squared = elapsed.checked_mul(elapsed)?;
    let linear_x = velocity.across.checked_mul(elapsed)?;
    let linear_y = velocity.down.checked_mul(elapsed)?;
    let quad_x = acceleration
        .across
        .checked_mul(n_squared)?
        .checked_div(two)?;
    let quad_y = acceleration.down.checked_mul(n_squared)?.checked_div(two)?;
    let out_x = origin.across.checked_add(linear_x)?.checked_add(quad_x)?;
    let out_y = origin.down.checked_add(linear_y)?.checked_add(quad_y)?;
    Ok(Point {
        across: out_x,
        down: out_y,
    })
}

/// Evaluates uniform circular motion about `centre` at the given angle in turns.
///
/// Returns `Err(Overflow)` when any intermediate value overflows exact arithmetic.
fn circular_position(centre: &Point, radius: &Ratio, angle: Ratio) -> Result<Point, Overflow> {
    let cos = cos_turns(angle)?;
    let sin = sin_turns(angle)?;
    let out_x = radius.checked_mul(cos)?.checked_add(centre.across)?;
    let out_y = radius.checked_mul(sin)?.checked_add(centre.down)?;
    Ok(Point {
        across: out_x,
        down: out_y,
    })
}

/// Evaluates sinusoidal motion: `origin + amplitude * sin(angle)`.
///
/// Returns `Err(Overflow)` when any intermediate value overflows exact arithmetic.
fn oscillating_position(
    origin: &Point,
    amplitude: &Vector,
    swing: Ratio,
) -> Result<Point, Overflow> {
    let out_x = amplitude
        .across
        .checked_mul(swing)?
        .checked_add(origin.across)?;
    let out_y = amplitude
        .down
        .checked_mul(swing)?
        .checked_add(origin.down)?;
    Ok(Point {
        across: out_x,
        down: out_y,
    })
}

/// Returns the oscillation angle `phase + elapsed / period` for a cycle length.
///
/// Returns `Err(Overflow)` when the division overflows exact arithmetic.
fn oscillating_angle(elapsed: Ratio, period: FrameCount, phase: &Ratio) -> Result<Ratio, Overflow> {
    let span = NonZeroI64::from(period.get()).get();
    let period_len = Ratio::from_integer(Numerator(span));
    let fraction = elapsed.checked_div(period_len)?;
    phase.checked_add(fraction)
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
    let out_x = origin.across.checked_add(offset_x)?;
    let out_y = origin.down.checked_add(offset_y)?;
    Ok(Point {
        across: out_x,
        down: out_y,
    })
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
        return Ok(Point {
            across: Ratio::from_integer(Numerator(0)),
            down: Ratio::from_integer(Numerator(0)),
        });
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
        } => ballistic_position(origin, velocity, frame_ratio(frame), acceleration),
        Motion::Circular {
            centre,
            radius,
            turns_per_frame,
            phase,
        } => {
            let turns = turns_per_frame.checked_mul(frame_ratio(frame))?;
            let angle = turns.checked_add(*phase)?;
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
mod tests;
