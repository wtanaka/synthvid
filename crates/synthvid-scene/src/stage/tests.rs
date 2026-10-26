use super::*;
use crate::color::Channel;
use crate::units::FrameIndex;
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

#[test]
fn coord_accepts_both_edges_of_the_world_bound() {
    assert_eq!(check_coord_accepts_both_edges_of_the_world_bound(), Ok(()));
}

fn check_coord_accepts_both_edges_of_the_world_bound() -> Result<(), Fail> {
    ensure_eq!(
        Coord::new(RawCoord(-0x0010_0000)).map(Coord::get),
        Ok(RawCoord(-0x0010_0000)),
        "the negative edge of the world bound is in range"
    );
    ensure_eq!(
        Coord::new(RawCoord(0x0010_0000)).map(Coord::get),
        Ok(RawCoord(0x0010_0000)),
        "the positive edge of the world bound is in range"
    );
    ensure_eq!(
        Coord::new(RawCoord(0x0010_0001)),
        Err(CoordError::OutOfRange),
        "one past the positive edge is out of range"
    );
    ensure_eq!(
        Coord::new(RawCoord(-0x0010_0001)),
        Err(CoordError::OutOfRange),
        "one past the negative edge is out of range"
    );
    Ok(())
}

#[test]
fn extent_accepts_the_world_bound_and_refuses_one_more() {
    assert_eq!(
        check_extent_accepts_the_world_bound_and_refuses_one_more(),
        Ok(())
    );
}

fn check_extent_accepts_the_world_bound_and_refuses_one_more() -> Result<(), Fail> {
    let largest = NonZeroU32::new(0x0010_0000).or_fail()?;
    let too_long = NonZeroU32::new(0x0010_0001).or_fail()?;
    ensure_eq!(
        Extent::new(largest).map(Extent::get),
        Ok(largest),
        "an extent equal to the world bound is accepted"
    );
    ensure_eq!(
        Extent::new(too_long),
        Err(ExtentError::TooLong),
        "an extent one past the world bound is refused"
    );
    Ok(())
}

#[test]
fn outline_keeps_vertices_in_declaration_order() {
    assert_eq!(check_outline_keeps_vertices_in_declaration_order(), Ok(()));
}

fn check_outline_keeps_vertices_in_declaration_order() -> Result<(), Fail> {
    let first = [
        LocalPoint {
            across: Coord::new(RawCoord(0)).or_fail()?,
            down: Coord::new(RawCoord(0)).or_fail()?,
        },
        LocalPoint {
            across: Coord::new(RawCoord(4)).or_fail()?,
            down: Coord::new(RawCoord(0)).or_fail()?,
        },
        LocalPoint {
            across: Coord::new(RawCoord(0)).or_fail()?,
            down: Coord::new(RawCoord(5)).or_fail()?,
        },
    ];
    let rest = vec![
        LocalPoint {
            across: Coord::new(RawCoord(-2)).or_fail()?,
            down: Coord::new(RawCoord(3)).or_fail()?,
        },
        LocalPoint {
            across: Coord::new(RawCoord(7)).or_fail()?,
            down: Coord::new(RawCoord(-1)).or_fail()?,
        },
    ];
    let outline = Outline::new(first, rest.clone());
    let mut expected: Vec<LocalPoint> = first.to_vec();
    expected.extend(rest);
    let collected: Vec<LocalPoint> = outline.vertices().copied().collect();
    ensure_eq!(
        collected,
        expected,
        "vertices must come out as the first three, then the rest, in order"
    );
    Ok(())
}

#[test]
fn outline_head_and_tail_split_the_vertices() {
    assert_eq!(check_outline_head_and_tail_split_the_vertices(), Ok(()));
}

fn check_outline_head_and_tail_split_the_vertices() -> Result<(), Fail> {
    let corner = |across: RawCoord, down: RawCoord| -> Result<LocalPoint, Fail> {
        Ok(LocalPoint {
            across: Coord::new(across).or_fail()?,
            down: Coord::new(down).or_fail()?,
        })
    };
    let first = [
        corner(RawCoord(0), RawCoord(0))?,
        corner(RawCoord(4), RawCoord(0))?,
        corner(RawCoord(0), RawCoord(5))?,
    ];
    let rest = vec![corner(RawCoord(-2), RawCoord(3))?];
    let outline = Outline::new(first, rest.clone());
    let (head, tail) = outline.head_and_tail();
    ensure_eq!(*head, first);
    ensure_eq!(tail, rest.as_slice());
    Ok(())
}

#[test]
fn stage_layers_keep_declaration_order_and_follow_their_tracks() {
    assert_eq!(
        check_stage_layers_keep_declaration_order_and_follow_their_tracks(),
        Ok(())
    );
}

/// Builds a world coordinate from its raw value, failing if it lies outside the world bound.
fn coord(raw: RawCoord) -> Result<Coord, Fail> {
    Coord::new(raw).or_fail()
}

/// A stage with a fixed disc at the origin and a rectangle moving linearly away from it.
fn example_stage() -> Result<Stage, Fail> {
    let visible = FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(10)).or_fail()?;
    let radius = Extent::new(NonZeroU32::new(5).or_fail()?).or_fail()?;
    Ok(Stage {
        backdrop: Rgb8 {
            red: Channel(0),
            green: Channel(0),
            blue: Channel(0),
        },
        layers: vec![
            Layer {
                cutout: Cutout::Disc { radius },
                fill: Rgb8 {
                    red: Channel(255),
                    green: Channel(0),
                    blue: Channel(0),
                },
                track: Track::Fixed(Position {
                    across: coord(RawCoord(0))?,
                    down: coord(RawCoord(0))?,
                    depth: coord(RawCoord(0))?,
                }),
                visible,
            },
            Layer {
                cutout: Cutout::Rect {
                    half_width: radius,
                    half_height: radius,
                },
                fill: Rgb8 {
                    red: Channel(0),
                    green: Channel(0),
                    blue: Channel(255),
                },
                track: Track::Linear {
                    origin: Position {
                        across: coord(RawCoord(-7))?,
                        down: coord(RawCoord(5))?,
                        depth: coord(RawCoord(2))?,
                    },
                    velocity: Velocity {
                        across: PerFrame(3),
                        down: PerFrame(-4),
                        depth: PerFrame(1),
                    },
                },
                visible,
            },
        ],
    })
}

fn check_stage_layers_keep_declaration_order_and_follow_their_tracks() -> Result<(), Fail> {
    let stage = example_stage()?;
    // At frame 3 the linear layer sits at across -7 + 3 * 3 = 2, down 5 + 3 * (-4) = -7,
    // and depth 2 + 3 * 1 = 5.
    let frame = FrameIndex::new(3);
    let mut positions = Vec::new();
    for layer in &stage.layers {
        positions.push(track_at(&layer.track, frame).or_fail()?);
    }
    let at_origin = Position {
        across: coord(RawCoord(0))?,
        down: coord(RawCoord(0))?,
        depth: coord(RawCoord(0))?,
    };
    let moved = Position {
        across: coord(RawCoord(2))?,
        down: coord(RawCoord(-7))?,
        depth: coord(RawCoord(5))?,
    };
    ensure_eq!(
        positions,
        vec![at_origin, moved],
        "each layer must report its own track, in declaration order"
    );
    Ok(())
}
