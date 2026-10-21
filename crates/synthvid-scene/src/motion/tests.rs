use super::*;
use crate::testutil::{At, Frac};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// Builds every motion variant over a shared origin for order tests.
fn every_motion(origin: Point) -> Result<Vec<Motion>, Fail> {
    let drift = Vector {
        across: Ratio::from_integer(Numerator(3)),
        down: Ratio::from_integer(Numerator(-1)),
    };
    let accel = Vector {
        across: Ratio::from_integer(Numerator(2)),
        down: Ratio::from_integer(Numerator(4)),
    };
    let radius = Ratio::from_integer(Numerator(4));
    let quarter = Frac(1, 4).ratio()?;
    let step = Ratio::from_integer(Numerator(2));
    let period = FrameCount::new(4).or_fail()?;
    let still = Motion::Fixed(origin);
    Ok(vec![
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
            phase: Ratio::from_integer(Numerator(0)),
        },
        Motion::Oscillating {
            origin,
            amplitude: drift,
            period,
            phase: Ratio::from_integer(Numerator(0)),
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
fn fixed_and_linear_basic() {
    assert_eq!(check_fixed_and_linear_basic(), Ok(()));
}

fn check_fixed_and_linear_basic() -> Result<(), Fail> {
    let spot = At(Frac(3, 2), Frac(-5, 4)).point()?;
    let still = Motion::Fixed(spot);
    for frame_number in 0..8_u32 {
        ensure_eq!(
            position_at(&still, FrameIndex::new(frame_number)),
            Ok(spot),
            "a fixed motion must hold its point on every frame"
        );
    }
    let origin = Point {
        across: Ratio::from_integer(Numerator(1)),
        down: Ratio::from_integer(Numerator(2)),
    };
    let velocity = Vector {
        across: Ratio::from_integer(Numerator(3)),
        down: Ratio::from_integer(Numerator(-1)),
    };
    let line = Motion::Linear { origin, velocity };
    let cases: [(u32, i64, i64); 3] = [(0, 1, 2), (1, 4, 1), (4, 13, -2)];
    for (frame_number, ex, ey) in cases {
        let want = Point {
            across: Ratio::from_integer(Numerator(ex)),
            down: Ratio::from_integer(Numerator(ey)),
        };
        ensure_eq!(
            position_at(&line, FrameIndex::new(frame_number)),
            Ok(want),
            "linear motion must equal origin plus velocity times frame"
        );
    }
    Ok(())
}

#[test]
fn ballistic_matches_hand_table() {
    let origin = Point {
        across: Ratio::from_integer(Numerator(1)),
        down: Ratio::from_integer(Numerator(2)),
    };
    let velocity = Vector {
        across: Ratio::from_integer(Numerator(3)),
        down: Ratio::from_integer(Numerator(-1)),
    };
    let acceleration = Vector {
        across: Ratio::from_integer(Numerator(2)),
        down: Ratio::from_integer(Numerator(4)),
    };
    let toss = Motion::Ballistic {
        origin,
        velocity,
        acceleration,
    };
    // Hand-computed from origin + velocity * n + acceleration * n^2 / 2.
    let cases: [(u32, i64, i64); 5] = [(0, 1, 2), (1, 5, 3), (2, 11, 8), (3, 19, 17), (4, 29, 30)];
    for (frame_number, ex, ey) in cases {
        let want = Point {
            across: Ratio::from_integer(Numerator(ex)),
            down: Ratio::from_integer(Numerator(ey)),
        };
        assert_eq!(
            position_at(&toss, FrameIndex::new(frame_number)),
            Ok(want),
            "ballistic motion must match the hand-computed table"
        );
    }
}

#[test]
fn circular_cardinal_frames() {
    assert_eq!(check_circular_cardinal_frames(), Ok(()));
}

fn check_circular_cardinal_frames() -> Result<(), Fail> {
    let centre = Point {
        across: Ratio::from_integer(Numerator(0)),
        down: Ratio::from_integer(Numerator(0)),
    };
    let radius = Ratio::from_integer(Numerator(4));
    let turns = Frac(1, 4).ratio()?;
    let phase = Ratio::from_integer(Numerator(0));
    let orbit = Motion::Circular {
        centre,
        radius,
        turns_per_frame: turns,
        phase,
    };
    let cases: [(u32, i64, i64); 5] = [(0, 4, 0), (1, 0, 4), (2, -4, 0), (3, 0, -4), (4, 4, 0)];
    for (frame_number, ex, ey) in cases {
        let want = Point {
            across: Ratio::from_integer(Numerator(ex)),
            down: Ratio::from_integer(Numerator(ey)),
        };
        ensure_eq!(
            position_at(&orbit, FrameIndex::new(frame_number)),
            Ok(want),
            "a quarter turn per frame must visit the cardinal points in order"
        );
    }
    Ok(())
}

#[test]
fn oscillating_peaks_and_midpoints() {
    assert_eq!(check_oscillating_peaks_and_midpoints(), Ok(()));
}

fn check_oscillating_peaks_and_midpoints() -> Result<(), Fail> {
    let origin = Point {
        across: Ratio::from_integer(Numerator(10)),
        down: Ratio::from_integer(Numerator(20)),
    };
    let amplitude = Vector {
        across: Ratio::from_integer(Numerator(4)),
        down: Ratio::from_integer(Numerator(-6)),
    };
    let period = FrameCount::new(4).or_fail()?;
    let phase = Ratio::from_integer(Numerator(0));
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
    for (frame_number, ex, ey) in cases {
        let want = Point {
            across: Ratio::from_integer(Numerator(ex)),
            down: Ratio::from_integer(Numerator(ey)),
        };
        ensure_eq!(
            position_at(&swing, FrameIndex::new(frame_number)),
            Ok(want),
            "oscillation must peak at quarter period and cross zero at half period"
        );
    }
    Ok(())
}

#[test]
fn walk_zero_determinism_and_bounded_steps() {
    assert_eq!(check_walk_zero_determinism_and_bounded_steps(), Ok(()));
}

fn check_walk_zero_determinism_and_bounded_steps() -> Result<(), Fail> {
    let origin = Point {
        across: Ratio::from_integer(Numerator(7)),
        down: Ratio::from_integer(Numerator(-3)),
    };
    let step = Ratio::from_integer(Numerator(2));
    let neg_step = Ratio::from_integer(Numerator(-2));
    let stroll = Motion::Walk {
        origin,
        step,
        seed: Seed::new(11),
    };
    ensure_eq!(
        position_at(&stroll, FrameIndex::new(0)),
        Ok(origin),
        "a walk at frame zero must sit exactly on its origin"
    );
    let far = position_at(&stroll, FrameIndex::new(900));
    ensure_eq!(
        position_at(&stroll, FrameIndex::new(900)),
        far,
        "re-evaluating a walk frame must give the identical point"
    );
    for frame_number in 0..32_u32 {
        let here = position_at(&stroll, FrameIndex::new(frame_number)).or_fail()?;
        ensure_eq!(
            position_at(&stroll, FrameIndex::new(frame_number)),
            Ok(here),
            "walk evaluation must be a pure function of the frame index"
        );
        let next = frame_number.checked_add(1).or_fail()?;
        let there = position_at(&stroll, FrameIndex::new(next)).or_fail()?;
        let dx = there.across.checked_sub(here.across).or_fail()?;
        let dy = there.down.checked_sub(here.down).or_fail()?;
        ensure!(
            dx >= neg_step && dx <= step,
            "one walk step in x must stay within the declared bound"
        );
        ensure!(
            dy >= neg_step && dy <= step,
            "one walk step in y must stay within the declared bound"
        );
    }
    Ok(())
}

#[test]
fn order_independence_forward_backward_shuffled() {
    assert_eq!(check_order_independence_forward_backward_shuffled(), Ok(()));
}

fn check_order_independence_forward_backward_shuffled() -> Result<(), Fail> {
    const COUNT: u32 = 25;
    let origin = Point {
        across: Ratio::from_integer(Numerator(1)),
        down: Ratio::from_integer(Numerator(2)),
    };
    let motions = every_motion(origin)?;
    for motion in &motions {
        let mut forward = Vec::new();
        for frame_number in 0..COUNT {
            forward.push(position_at(motion, FrameIndex::new(frame_number)));
        }
        let mut backward = Vec::new();
        for frame_number in (0..COUNT).rev() {
            backward.push(position_at(motion, FrameIndex::new(frame_number)));
        }
        backward.reverse();
        ensure_eq!(
            backward,
            forward,
            "evaluating frames backwards must match forwards order"
        );
        // A fixed stride coprime to COUNT visits every frame exactly once.
        let mut shuffled = Vec::new();
        for frame_number in 0..COUNT {
            let permuted = frame_number
                .checked_mul(7)
                .and_then(|scaled| scaled.checked_add(3))
                .or_fail()?;
            let slot = permuted.checked_rem(COUNT).or_fail()?;
            shuffled.push((slot, position_at(motion, FrameIndex::new(slot))));
        }
        shuffled.sort_by_key(|entry| entry.0);
        let back_in_order: Vec<Result<Point, Overflow>> =
            shuffled.iter().map(|entry| entry.1).collect();
        ensure_eq!(
            back_in_order,
            forward,
            "evaluating frames shuffled must match forwards order"
        );
    }
    Ok(())
}

#[test]
fn piecewise_boundary_continuous() {
    let first_origin = Point {
        across: Ratio::from_integer(Numerator(0)),
        down: Ratio::from_integer(Numerator(0)),
    };
    let first_velocity = Vector {
        across: Ratio::from_integer(Numerator(1)),
        down: Ratio::from_integer(Numerator(2)),
    };
    // Segments evaluate at the absolute frame number, so the second leg's
    // origin is chosen to pass through the shared position (10, 20) at
    // frame 10: 20 + (-1) * 10 = 10.
    let second_origin = Point {
        across: Ratio::from_integer(Numerator(20)),
        down: Ratio::from_integer(Numerator(20)),
    };
    let second_velocity = Vector {
        across: Ratio::from_integer(Numerator(-1)),
        down: Ratio::from_integer(Numerator(0)),
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
    let before = Point {
        across: Ratio::from_integer(Numerator(9)),
        down: Ratio::from_integer(Numerator(18)),
    };
    let at_edge = Point {
        across: Ratio::from_integer(Numerator(10)),
        down: Ratio::from_integer(Numerator(20)),
    };
    let after = Point {
        across: Ratio::from_integer(Numerator(9)),
        down: Ratio::from_integer(Numerator(20)),
    };
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
fn piecewise_first_entry_empty_and_nested() {
    let early = Point {
        across: Ratio::from_integer(Numerator(1)),
        down: Ratio::from_integer(Numerator(1)),
    };
    let late = Point {
        across: Ratio::from_integer(Numerator(9)),
        down: Ratio::from_integer(Numerator(9)),
    };
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
    let scene_origin = Point {
        across: Ratio::from_integer(Numerator(0)),
        down: Ratio::from_integer(Numerator(0)),
    };
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
fn overflow_returns_err() {
    assert_eq!(check_overflow_returns_err(), Ok(()));
}

fn check_overflow_returns_err() -> Result<(), Fail> {
    // Construct a linear motion with a large velocity that will overflow
    // when multiplied by a large frame number.
    let origin = Point {
        across: Ratio::from_integer(Numerator(0)),
        down: Ratio::from_integer(Numerator(0)),
    };
    let max_ratio = Ratio::from_integer(Numerator(i64::MAX));
    let velocity = Vector {
        across: max_ratio,
        down: max_ratio,
    };
    let overflow_motion = Motion::Linear { origin, velocity };

    // Frame 0 should succeed (multiplication by 0)
    ensure_eq!(
        position_at(&overflow_motion, FrameIndex::new(0)),
        Ok(origin),
        "overflow motion at frame 0 must succeed"
    );

    // Frame 2 should fail (max_ratio * 2 overflows)
    let result = position_at(&overflow_motion, FrameIndex::new(2));
    ensure!(
        result.is_err(),
        "overflow motion should return Err when arithmetic overflows"
    );
    Ok(())
}
