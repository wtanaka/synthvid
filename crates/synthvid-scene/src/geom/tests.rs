use super::*;
use crate::testutil::Frac;
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

#[test]
fn affine_apply_compose_inverse() {
    assert_eq!(check_affine_apply_compose_inverse(), Ok(()));
}

fn check_affine_apply_compose_inverse() -> Result<(), Fail> {
    let probe = Point {
        across: Frac(3, 2).ratio()?,
        down: Frac(-5, 4).ratio()?,
    };
    let same = Affine::identity().apply(probe);
    ensure_eq!(same, Ok(probe), "identity must map a point to itself");
    let dx = Ratio::from_integer(Numerator(2));
    let dy = Ratio::from_integer(Numerator(3));
    let shift = Affine::translation(Vector {
        across: dx,
        down: dy,
    });
    let moved = shift.apply(probe).or_fail()?;
    let expect_x = probe.across.checked_add(dx).or_fail()?;
    let expect_y = probe.down.checked_add(dy).or_fail()?;
    ensure_eq!(
        moved,
        Point {
            across: expect_x,
            down: expect_y
        },
        "translation must add the displacement"
    );
    let back = shift.inverse().or_fail()?;
    let there = shift.apply(probe).or_fail()?;
    let roundtrip = back.apply(there).or_fail()?;
    ensure_eq!(
        roundtrip,
        probe,
        "inverse must undo the translation exactly"
    );
    let there_and_back = shift.then(back).or_fail()?;
    ensure_eq!(
        there_and_back,
        Affine::identity(),
        "a transform followed by its inverse must be identity"
    );
    check_affine_vectors(probe, &shift)
}

/// Checks that a scale multiplies the components of `probe` taken as a
/// vector, and that the translation `shift` leaves that vector alone.
fn check_affine_vectors(probe: Point, shift: &Affine) -> Result<(), Fail> {
    let probe_vector = Vector {
        across: probe.across,
        down: probe.down,
    };
    let sx = Ratio::from_integer(Numerator(2));
    let sy = Ratio::from_integer(Numerator(3));
    let scale = Affine {
        xx: sx,
        xy: Ratio::from_integer(Numerator(0)),
        tx: Ratio::from_integer(Numerator(0)),
        yx: Ratio::from_integer(Numerator(0)),
        yy: sy,
        ty: Ratio::from_integer(Numerator(0)),
    };
    let scaled_x = probe.across.checked_mul(sx).or_fail()?;
    let scaled_y = probe.down.checked_mul(sy).or_fail()?;
    let scaled = scale.apply_vector(probe_vector).or_fail()?;
    ensure_eq!(
        scaled,
        Vector {
            across: scaled_x,
            down: scaled_y
        },
        "scaling must multiply vector components"
    );
    let vec_moved = shift.apply_vector(probe_vector).or_fail()?;
    ensure_eq!(
        vec_moved,
        probe_vector,
        "translation must not affect vectors"
    );
    Ok(())
}

#[test]
fn affine_quarter_turns() {
    assert_eq!(check_affine_quarter_turns(), Ok(()));
}

fn check_affine_quarter_turns() -> Result<(), Fail> {
    let one = Ratio::from_integer(Numerator(1));
    let zero = Ratio::from_integer(Numerator(0));
    let neg_one = Ratio::from_integer(Numerator(-1));
    let probe = Point {
        across: one,
        down: zero,
    };
    let first = Affine::quarter_turns(QuarterTurn::One)
        .apply(probe)
        .or_fail()?;
    ensure_eq!(
        first,
        Point {
            across: zero,
            down: one
        },
        "one quarter turn must map (1, 0) to (0, 1)"
    );
    let second = Affine::quarter_turns(QuarterTurn::Two)
        .apply(probe)
        .or_fail()?;
    ensure_eq!(
        second,
        Point {
            across: neg_one,
            down: zero
        },
        "two quarter turns must map (1, 0) to (-1, 0)"
    );
    let third = Affine::quarter_turns(QuarterTurn::Three)
        .apply(probe)
        .or_fail()?;
    ensure_eq!(
        third,
        Point {
            across: zero,
            down: neg_one
        },
        "three quarter turns must map (1, 0) to (0, -1)"
    );
    let full = Affine::quarter_turns(QuarterTurn::Zero)
        .apply(probe)
        .or_fail()?;
    ensure_eq!(full, probe, "zero quarter turns must be identity");
    let backward = Affine::quarter_turns(QuarterTurn::One)
        .then(Affine::quarter_turns(QuarterTurn::Three))
        .or_fail()?;
    ensure_eq!(
        backward,
        Affine::identity(),
        "one forward and three forward turns must cancel to identity"
    );
    Ok(())
}

#[test]
fn similarity_roundtrip_and_affine() {
    assert_eq!(check_similarity_roundtrip_and_affine(), Ok(()));
}

fn check_similarity_roundtrip_and_affine() -> Result<(), Fail> {
    let one = Ratio::from_integer(Numerator(1));
    let zero = Ratio::from_integer(Numerator(0));
    let two = Ratio::from_integer(Numerator(2));
    let three = Ratio::from_integer(Numerator(3));
    let probe = Point {
        across: two,
        down: three,
    };
    let shift = Similarity::translation(Vector {
        across: two,
        down: three,
    });
    let moved = shift.apply(probe).or_fail()?;
    let back = shift.inverse().or_fail()?;
    let home = back.apply(moved).or_fail()?;
    ensure_eq!(home, probe, "inverse must undo a translation exactly");
    let spun = Similarity::quarter_turns(QuarterTurn::One)
        .apply(Point {
            across: one,
            down: zero,
        })
        .or_fail()?;
    ensure_eq!(
        spun,
        Point {
            across: zero,
            down: one
        },
        "a similarity quarter turn must match the affine one"
    );
    let scaled = Similarity::uniform_scale(two);
    let wide = scaled
        .apply_vector(Vector {
            across: one,
            down: zero,
        })
        .or_fail()?;
    ensure_eq!(
        wide,
        Vector {
            across: two,
            down: zero
        },
        "uniform scale must multiply both axes equally"
    );
    let as_affine = scaled.to_affine().or_fail()?;
    ensure_eq!(
        as_affine.yy,
        as_affine.xx,
        "a similarity affine must scale uniformly"
    );
    Ok(())
}
