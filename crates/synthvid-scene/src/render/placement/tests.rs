use super::*;
use crate::camera::{Camera, Magnification, Rotation, Turns, Zoom};
use crate::color::{Channel, Rgb8};
use crate::raster::{Bounds, Corners};
use crate::scene::{Background, FrameSpan, Motion};
use crate::testutil::{At, Frac};
use crate::units::{Dimensions, FrameCount, FrameRate, Height, Width};
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

fn bounds(corners: Corners) -> Result<BoundsOrEmpty, Fail> {
    Ok(BoundsOrEmpty::Covering(Bounds::new(corners).or_fail()?))
}

/// Camera at (1, 1), a quarter turn, magnification 3/2.
///
/// The scene-to-screen map is `spin * zoom * (w - centre)` with `spin`
/// the rotation by the negated angle, i.e. by -1/4 turn, which sends
/// `(x, y)` to `(y, -x)`. Hence `w -> (3/2) * (w.y - 1, -(w.x - 1))`.
fn quarter_turn_scene(objects: Vec<Object>) -> Result<Scene, Fail> {
    let camera = Camera::new(
        Motion::Fixed(At(Frac(1, 1), Frac(1, 1)).point()?),
        Rotation::Fixed(Turns::new(Frac(1, 4).ratio()?)),
        Zoom::Fixed(Magnification::new(Frac(3, 2).ratio()?).or_fail()?),
    );
    Ok(Scene {
        dimensions: Dimensions::new(Width::new(8).or_fail()?, Height::new(8).or_fail()?),
        frame_rate: FrameRate::from_fps(30).or_fail()?,
        frame_count: FrameCount::new(4).or_fail()?,
        scale: None,
        background: Background::Solid(Rgb8 {
            red: Channel(0),
            green: Channel(0),
            blue: Channel(0),
        }),
        camera,
        objects,
    })
}

fn fixed(shape: Shape, at: Point, visible: FrameSpan) -> Object {
    Object::new(
        shape,
        Rgb8 {
            red: Channel(255),
            green: Channel(0),
            blue: Channel(0),
        },
        Motion::Fixed(at),
        visible,
    )
}

#[test]
fn object_states_under_a_turned_zoomed_camera() {
    assert_eq!(check_object_states_under_a_turned_zoomed_camera(), Ok(()));
}

/// Builds the three-object scene, under the quarter-turned and magnified
/// camera, that [`check_object_states_under_a_turned_zoomed_camera`] probes.
fn turned_zoomed_scene() -> Result<Scene, Fail> {
    quarter_turn_scene(vec![
        // Declared first, visible only at frames 2 and 3.
        fixed(
            Shape::Disc {
                radius: Frac(1, 1).ratio()?,
            },
            At(Frac(0, 1), Frac(0, 1)).point()?,
            FrameSpan::new(FrameIndex::new(2)..FrameIndex::new(4)).or_fail()?,
        ),
        // Disc radius 2 at (4, 2): offset from the camera (3, 1), which
        // turns to (1, -3) and magnifies to (3/2, -9/2). Radius 2 * 3/2
        // = 3, so the box is [-3/2, 9/2] x [-15/2, -3/2].
        fixed(
            Shape::Disc {
                radius: Frac(2, 1).ratio()?,
            },
            At(Frac(4, 1), Frac(2, 1)).point()?,
            FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(3)).or_fail()?,
        ),
        // Rect half-extents 2 x 1 at (4, 2): corners (2,1) (6,1) (6,3)
        // (2,3); offsets (1,0) (5,0) (5,2) (1,2); turned (0,-1) (0,-5)
        // (2,-5) (2,-1); magnified (0,-3/2) (0,-15/2) (3,-15/2)
        // (3,-3/2). Box [0, 3] x [-15/2, -3/2].
        fixed(
            Shape::Rect {
                half_width: Frac(2, 1).ratio()?,
                half_height: Frac(1, 1).ratio()?,
            },
            At(Frac(4, 1), Frac(2, 1)).point()?,
            FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(4)).or_fail()?,
        ),
    ])
}

fn check_object_states_under_a_turned_zoomed_camera() -> Result<(), Fail> {
    let scene = turned_zoomed_scene()?;
    let at_frame_1 = object_states(&scene, FrameIndex::new(1)).or_fail()?;
    ensure_eq!(
        at_frame_1,
        vec![
            ObjectState {
                index: ObjectIndex::new(1),
                centre_scene: At(Frac(4, 1), Frac(2, 1)).point()?,
                centre_screen: At(Frac(3, 2), Frac(-9, 2)).point()?,
                extent: bounds(Corners {
                    min: At(Frac(-3, 2), Frac(-15, 2)).point()?,
                    max: At(Frac(9, 2), Frac(-3, 2)).point()?,
                })?,
            },
            ObjectState {
                index: ObjectIndex::new(2),
                centre_scene: At(Frac(4, 1), Frac(2, 1)).point()?,
                centre_screen: At(Frac(3, 2), Frac(-9, 2)).point()?,
                extent: bounds(Corners {
                    min: At(Frac(0, 1), Frac(-15, 2)).point()?,
                    max: At(Frac(3, 1), Frac(-3, 2)).point()?,
                })?,
            },
        ]
    );

    // At frame 2 the first object appears, in declaration order, and
    // the second is still visible; at frame 3 the second has gone.
    // Disc radius 1 at (0, 0): offset (-1, -1), turned (-1, 1),
    // magnified (-3/2, 3/2), radius 3/2: [-3, 0] x [0, 3].
    let at_frame_2 = object_states(&scene, FrameIndex::new(2)).or_fail()?;
    let indices: Vec<_> = at_frame_2.iter().map(|state| state.index).collect();
    ensure_eq!(
        indices,
        vec![
            ObjectIndex::new(0),
            ObjectIndex::new(1),
            ObjectIndex::new(2)
        ]
    );
    ensure_eq!(
        at_frame_2.first().map(|state| state.extent),
        Some(bounds(Corners {
            min: At(Frac(-3, 1), Frac(0, 1)).point()?,
            max: At(Frac(0, 1), Frac(3, 1)).point()?,
        })?)
    );
    let at_frame_3 = object_states(&scene, FrameIndex::new(3)).or_fail()?;
    let remaining: Vec<_> = at_frame_3.iter().map(|state| state.index).collect();
    ensure_eq!(remaining, vec![ObjectIndex::new(0), ObjectIndex::new(2)]);
    Ok(())
}

#[test]
fn degenerate_shape_is_visible_but_empty() {
    assert_eq!(check_degenerate_shape_is_visible_but_empty(), Ok(()));
}

fn check_degenerate_shape_is_visible_but_empty() -> Result<(), Fail> {
    let scene = quarter_turn_scene(vec![fixed(
        Shape::Cross {
            arm: Frac(5, 1).ratio()?,
            thickness: Frac(0, 1).ratio()?,
        },
        At(Frac(4, 1), Frac(2, 1)).point()?,
        FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(4)).or_fail()?,
    )])?;
    let states = object_states(&scene, FrameIndex::new(0)).or_fail()?;
    ensure_eq!(states.len(), 1);
    ensure_eq!(
        states.first().map(|state| state.extent),
        Some(BoundsOrEmpty::Empty)
    );
    Ok(())
}
