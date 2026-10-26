use super::*;
use crate::testutil::Frac;
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// Builds an angle in turns from a fraction.
fn turns(frac: Frac) -> Result<Turns, Fail> {
    Ok(Turns::new(frac.ratio()?))
}

/// Builds a magnification from a fraction.
fn magnification(frac: Frac) -> Result<Magnification, Fail> {
    Magnification::new(frac.ratio()?).or_fail()
}

#[test]
fn magnification_rejects_zero_and_negative() {
    assert_eq!(check_magnification_rejects_zero_and_negative(), Ok(()));
}

fn check_magnification_rejects_zero_and_negative() -> Result<(), Fail> {
    let zero = Ratio::from_integer(Numerator(0));
    let negative = Frac(-3, 2).ratio()?;
    let positive = Frac(3, 2).ratio()?;
    ensure!(
        Magnification::new(zero).is_err(),
        "zero magnification must be rejected"
    );
    ensure!(
        Magnification::new(negative).is_err(),
        "negative magnification must be rejected"
    );
    let magnification = Magnification::new(positive).or_fail()?;
    ensure_eq!(
        magnification.get(),
        positive,
        "positive magnification must round-trip"
    );
    Ok(())
}

#[test]
fn zoom_ramp_stays_positive_before_inside_after() {
    assert_eq!(check_zoom_ramp_stays_positive_before_inside_after(), Ok(()));
}

fn check_zoom_ramp_stays_positive_before_inside_after() -> Result<(), Fail> {
    let start = magnification(Frac(1, 1))?;
    let end = magnification(Frac(3, 1))?;
    let ramp = Zoom::Ramp {
        start,
        end,
        over: FrameCount::new(4).or_fail()?,
    };
    let zero = Ratio::from_integer(Numerator(0));
    for frame_number in [0_u32, 2, 9] {
        let at = zoom_at(&ramp, FrameIndex::new(frame_number)).or_fail()?;
        ensure!(
            at.get() > zero,
            "ramp magnification at frame {frame_number} must stay strictly positive"
        );
    }
    Ok(())
}

#[test]
fn zoom_ramp_endpoints_exact() {
    assert_eq!(check_zoom_ramp_endpoints_exact(), Ok(()));
}

fn check_zoom_ramp_endpoints_exact() -> Result<(), Fail> {
    let start = magnification(Frac(1, 2))?;
    let end = magnification(Frac(5, 1))?;
    let ramp = Zoom::Ramp {
        start,
        end,
        over: FrameCount::new(6).or_fail()?,
    };
    ensure_eq!(
        zoom_at(&ramp, FrameIndex::new(0)),
        Ok(start),
        "ramp at frame zero must equal its start exactly"
    );
    ensure_eq!(
        zoom_at(&ramp, FrameIndex::new(6)),
        Ok(end),
        "ramp at its span must equal its end exactly"
    );
    ensure_eq!(
        zoom_at(&ramp, FrameIndex::new(60)),
        Ok(end),
        "ramp past its span must hold its end exactly"
    );
    Ok(())
}

#[test]
fn zoom_ramp_midpoint_matches_hand_value() {
    assert_eq!(check_zoom_ramp_midpoint_matches_hand_value(), Ok(()));
}

fn check_zoom_ramp_midpoint_matches_hand_value() -> Result<(), Fail> {
    let start = magnification(Frac(1, 1))?;
    let end = magnification(Frac(3, 1))?;
    let ramp = Zoom::Ramp {
        start,
        end,
        over: FrameCount::new(4).or_fail()?,
    };
    // t = 2 / 4 = 1 / 2, so the value is 1 + (1 / 2) * (3 - 1) = 2.
    let want = magnification(Frac(2, 1))?;
    ensure_eq!(
        zoom_at(&ramp, FrameIndex::new(2)),
        Ok(want),
        "ramp halfway through its span must equal the midpoint"
    );
    Ok(())
}

#[test]
fn rotation_linear_matches_hand_table() {
    assert_eq!(check_rotation_linear_matches_hand_table(), Ok(()));
}

fn check_rotation_linear_matches_hand_table() -> Result<(), Fail> {
    let start = turns(Frac(1, 4))?;
    let per_frame = turns(Frac(1, 8))?;
    let spin = Rotation::Linear { start, per_frame };
    // Hand-computed from start + per_frame * n.
    let cases: [(u32, Frac); 3] = [(0, Frac(1, 4)), (2, Frac(1, 2)), (4, Frac(3, 4))];
    for (frame_number, frac) in cases {
        let want = turns(frac)?;
        ensure_eq!(
            rotation_at(&spin, FrameIndex::new(frame_number)),
            Ok(want),
            "linear rotation at frame {frame_number} must equal start plus rate times frame"
        );
    }
    let fixed_angle = turns(Frac(-3, 2))?;
    let still = Rotation::Fixed(fixed_angle);
    for frame_number in 0..4_u32 {
        ensure_eq!(
            rotation_at(&still, FrameIndex::new(frame_number)),
            Ok(fixed_angle),
            "a fixed rotation must hold its angle on every frame"
        );
    }
    Ok(())
}

#[test]
fn rotation_linear_order_independence() {
    assert_eq!(check_rotation_linear_order_independence(), Ok(()));
}

fn check_rotation_linear_order_independence() -> Result<(), Fail> {
    const COUNT: u32 = 25;
    let start = turns(Frac(1, 4))?;
    let per_frame = turns(Frac(1, 8))?;
    let spin = Rotation::Linear { start, per_frame };
    let mut forward = Vec::new();
    for frame_number in 0..COUNT {
        forward.push(rotation_at(&spin, FrameIndex::new(frame_number)));
    }
    // A fixed stride coprime to COUNT visits every frame exactly once.
    let mut shuffled = Vec::new();
    for frame_number in 0..COUNT {
        let permuted = frame_number
            .checked_mul(7)
            .and_then(|scaled| scaled.checked_add(3))
            .or_fail()?;
        let slot = permuted.checked_rem(COUNT).or_fail()?;
        shuffled.push((slot, rotation_at(&spin, FrameIndex::new(slot))));
    }
    shuffled.sort_by_key(|entry| entry.0);
    let back_in_order: Vec<Result<Turns, Overflow>> =
        shuffled.iter().map(|entry| entry.1).collect();
    ensure_eq!(
        back_in_order,
        forward,
        "evaluating rotation shuffled must match forwards order"
    );
    Ok(())
}
