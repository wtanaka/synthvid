use super::*;
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// Validates a raw coordinate, failing the check if it lies outside the world bound.
fn coord(raw: RawCoord) -> Result<Coord, Fail> {
    Coord::new(raw).or_fail()
}

/// A ballistic track starting at depth 1000 with velocity `(3, -2, -7)` and
/// half acceleration `(0, 1, 0)`.
fn falling_track() -> Result<Track, Fail> {
    Ok(Track::Ballistic {
        origin: Position {
            across: coord(RawCoord(0))?,
            down: coord(RawCoord(0))?,
            depth: coord(RawCoord(1000))?,
        },
        velocity: Velocity {
            across: PerFrame(3),
            down: PerFrame(-2),
            depth: PerFrame(-7),
        },
        half_acceleration: HalfAcceleration {
            across: PerFrameSquared(0),
            down: PerFrameSquared(1),
            depth: PerFrameSquared(0),
        },
    })
}

#[test]
fn ballistic_matches_hand_computed_vector() {
    assert_eq!(check_ballistic_matches_hand_computed_vector(), Ok(()));
}

fn check_ballistic_matches_hand_computed_vector() -> Result<(), Fail> {
    let track = falling_track()?;
    // Across: 0 + 3 * 5 + 0 * 25 = 15.
    // Down: 0 + (-2) * 5 + 1 * 25 = 15.
    // Depth: 1000 + (-7) * 5 + 0 * 25 = 965.
    let expected = Position {
        across: coord(RawCoord(15))?,
        down: coord(RawCoord(15))?,
        depth: coord(RawCoord(965))?,
    };
    ensure_eq!(
        track_at(&track, FrameIndex::new(5)),
        Ok(expected),
        "a ballistic track must follow its closed-form polynomial"
    );
    Ok(())
}

#[test]
fn ballistic_is_closed_form_across_frame_order() {
    assert_eq!(check_ballistic_is_closed_form_across_frame_order(), Ok(()));
}

fn check_ballistic_is_closed_form_across_frame_order() -> Result<(), Fail> {
    let track = falling_track()?;
    let first = track_at(&track, FrameIndex::new(3));
    let later = track_at(&track, FrameIndex::new(9));
    ensure!(
        later.is_ok(),
        "a later frame must evaluate without disturbing frame 3"
    );
    let again = track_at(&track, FrameIndex::new(3));
    // Across: 3 * 3 = 9. Down: -2 * 3 + 1 * 9 = 3. Depth: 1000 + (-7) * 3 = 979.
    let expected = Position {
        across: coord(RawCoord(9))?,
        down: coord(RawCoord(3))?,
        depth: coord(RawCoord(979))?,
    };
    ensure_eq!(first, Ok(expected), "frame 3 must hold its own value");
    ensure_eq!(
        first,
        again,
        "a frame evaluated after another frame must match its first evaluation"
    );
    Ok(())
}

#[test]
fn linear_reaches_world_bound_then_leaves_it() {
    assert_eq!(check_linear_reaches_world_bound_then_leaves_it(), Ok(()));
}

fn check_linear_reaches_world_bound_then_leaves_it() -> Result<(), Fail> {
    let track = Track::Linear {
        origin: Position {
            across: coord(RawCoord(0x000F_FFFA))?,
            down: coord(RawCoord(0))?,
            depth: coord(RawCoord(0))?,
        },
        velocity: Velocity {
            across: PerFrame(1),
            down: PerFrame(0),
            depth: PerFrame(0),
        },
    };
    let on_bound = Position {
        across: coord(RawCoord(0x0010_0000))?,
        down: coord(RawCoord(0))?,
        depth: coord(RawCoord(0))?,
    };
    ensure_eq!(
        track_at(&track, FrameIndex::new(6)),
        Ok(on_bound),
        "across 1_048_570 + 1 * 6 is exactly the world bound"
    );
    ensure_eq!(
        track_at(&track, FrameIndex::new(7)),
        Err(TrackError::LeftWorld),
        "across 1_048_570 + 1 * 7 is 1_048_577, beyond the world bound"
    );
    Ok(())
}

#[test]
fn fixed_holds_its_position_at_any_frame() {
    assert_eq!(check_fixed_holds_its_position_at_any_frame(), Ok(()));
}

fn check_fixed_holds_its_position_at_any_frame() -> Result<(), Fail> {
    let spot = Position {
        across: coord(RawCoord(1))?,
        down: coord(RawCoord(2))?,
        depth: coord(RawCoord(3))?,
    };
    let track = Track::Fixed(spot);
    ensure_eq!(
        track_at(&track, FrameIndex::new(0)),
        Ok(spot),
        "a fixed track must hold its position at frame zero"
    );
    ensure_eq!(
        track_at(&track, FrameIndex::new(4_000_000_000)),
        Ok(spot),
        "a fixed track must hold its position at a large frame"
    );
    Ok(())
}

#[test]
fn depth_velocity_at_i32_max_leaves_the_world() {
    assert_eq!(check_depth_velocity_at_i32_max_leaves_the_world(), Ok(()));
}

fn check_depth_velocity_at_i32_max_leaves_the_world() -> Result<(), Fail> {
    let still = Position {
        across: coord(RawCoord(0))?,
        down: coord(RawCoord(0))?,
        depth: coord(RawCoord(0))?,
    };
    let track = Track::Ballistic {
        origin: still,
        velocity: Velocity {
            across: PerFrame(0),
            down: PerFrame(0),
            depth: PerFrame(i32::MAX),
        },
        half_acceleration: HalfAcceleration {
            across: PerFrameSquared(0),
            down: PerFrameSquared(0),
            depth: PerFrameSquared(0),
        },
    };
    ensure_eq!(
        track_at(&track, FrameIndex::new(4_000_000_000)),
        Err(TrackError::LeftWorld),
        "i32::MAX * 4_000_000_000 fits in i128 but leaves the world bound"
    );
    Ok(())
}

#[test]
fn most_negative_terms_at_the_largest_frame_leave_the_world() {
    assert_eq!(
        check_most_negative_terms_at_the_largest_frame_leave_the_world(),
        Ok(())
    );
}

fn check_most_negative_terms_at_the_largest_frame_leave_the_world() -> Result<(), Fail> {
    // Frame F = 2^32 - 1, velocity V = -2^31 and half acceleration A = -2^31, from an origin of
    // -2^20. The velocity term is V F = -2^63 + 2^31. The acceleration term is A F^2, which is
    // -2^95 + 2^64 - 2^31. The sum is -2^95 + 2^63 - 2^20, far below the i32 range, so every axis
    // leaves the world.
    let origin = Position {
        across: coord(RawCoord(-0x0010_0000))?,
        down: coord(RawCoord(-0x0010_0000))?,
        depth: coord(RawCoord(-0x0010_0000))?,
    };
    let track = Track::Ballistic {
        origin,
        velocity: Velocity {
            across: PerFrame(i32::MIN),
            down: PerFrame(i32::MIN),
            depth: PerFrame(i32::MIN),
        },
        half_acceleration: HalfAcceleration {
            across: PerFrameSquared(i32::MIN),
            down: PerFrameSquared(i32::MIN),
            depth: PerFrameSquared(i32::MIN),
        },
    };
    ensure_eq!(
        track_at(&track, FrameIndex::new(u32::MAX)),
        Err(TrackError::LeftWorld),
        "every term negative at the largest frame must leave the world bound"
    );
    Ok(())
}

#[test]
fn most_positive_terms_at_the_largest_frame_leave_the_world() {
    assert_eq!(
        check_most_positive_terms_at_the_largest_frame_leave_the_world(),
        Ok(())
    );
}

fn check_most_positive_terms_at_the_largest_frame_leave_the_world() -> Result<(), Fail> {
    // Frame F = 2^32 - 1, velocity V = 2^31 - 1 and half acceleration A = 2^31 - 1, from an origin
    // of 2^20. The velocity term is V F = 2^63 - 2^31 - 2^32 + 1, and the acceleration term A F^2
    // is about 2^95. The sum is far above the i32 range, so every axis leaves the world.
    let origin = Position {
        across: coord(RawCoord(0x0010_0000))?,
        down: coord(RawCoord(0x0010_0000))?,
        depth: coord(RawCoord(0x0010_0000))?,
    };
    let track = Track::Ballistic {
        origin,
        velocity: Velocity {
            across: PerFrame(i32::MAX),
            down: PerFrame(i32::MAX),
            depth: PerFrame(i32::MAX),
        },
        half_acceleration: HalfAcceleration {
            across: PerFrameSquared(i32::MAX),
            down: PerFrameSquared(i32::MAX),
            depth: PerFrameSquared(i32::MAX),
        },
    };
    ensure_eq!(
        track_at(&track, FrameIndex::new(u32::MAX)),
        Err(TrackError::LeftWorld),
        "every term positive at the largest frame must leave the world bound"
    );
    Ok(())
}
