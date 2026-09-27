//! Per-frame placement of objects: the single source of truth for where an
//! object is, on screen and in the scene, at one frame.
//!
//! [`object_states`] reports for every object visible at a frame its scene
//! centre, its screen centre, and its screen-space extent. The renderer takes
//! each object's scene position from the same [`placement`] call, and the
//! extent is built from the same geometry helpers (`rect_corners`,
//! `shifted_vertices`, and the camera transform) that the drawing code uses.
//! The drawing code still rebuilds its own geometry rather than consuming the
//! mapped points computed here, so the two agree by sharing those helpers, not
//! by construction.

use crate::geom::Point;
use crate::raster::{disc_extent, polygon_extent, BoundsOrEmpty};
use crate::ratio::{int_ratio, Overflow};
use crate::scene::{Object, Scene, Shape};
use crate::units::{FrameIndex, ObjectIndex};

use super::shape::{rect_corners, shifted_vertices};
use super::{camera_frame, CameraFrame, RenderError};

/// Where one visible object is at one frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ObjectState {
    /// Position of the object in `Scene::objects`.
    pub index: ObjectIndex,
    /// The object's motion position, in scene space.
    pub centre_scene: Point,
    /// That position carried through the camera, in screen space.
    pub centre_screen: Point,
    /// The screen-space box the object is drawn within, or
    /// [`BoundsOrEmpty::Empty`] when it draws nothing.
    pub extent: BoundsOrEmpty,
}

/// The frame and object a placement is evaluated for.
#[derive(Copy, Clone, Debug)]
pub(super) struct Slot {
    /// Frame being evaluated.
    pub(super) frame: FrameIndex,
    /// Object being evaluated.
    pub(super) object: ObjectIndex,
}

impl Slot {
    /// The error reported when this slot's placement overflows.
    pub(super) const fn overflow(self) -> RenderError {
        RenderError::ObjectOverflow {
            frame: self.frame,
            object: self.object,
        }
    }
}

/// Names the object at `position` in `Scene::objects`.
///
/// # Errors
///
/// Returns [`RenderError::ObjectOverflow`] for a position beyond `u32`.
pub(super) fn object_index(frame: FrameIndex, position: usize) -> Result<ObjectIndex, RenderError> {
    u32::try_from(position)
        .map(ObjectIndex::new)
        .map_err(|_| RenderError::ObjectOverflow {
            frame,
            object: ObjectIndex::new(u32::MAX),
        })
}

/// Returns the screen-space extent of `shape` centred on `at`.
fn screen_extent(
    shape: &Shape,
    at: Point,
    camera: &CameraFrame,
) -> Result<BoundsOrEmpty, Overflow> {
    let map = |points: &[Point], out: &mut Vec<Point>| -> Result<(), Overflow> {
        for point in points {
            out.push(camera.transform.apply(*point)?);
        }
        Ok(())
    };
    let mut mapped = Vec::new();
    match shape {
        Shape::Disc { radius } => {
            let centre = camera.transform.apply(at)?;
            disc_extent(centre, radius.checked_mul(camera.zoom)?)
        }
        Shape::Rect {
            half_width,
            half_height,
        } => {
            map(&rect_corners(at, *half_width, *half_height)?, &mut mapped)?;
            polygon_extent(&mapped)
        }
        Shape::Polygon { vertices } => {
            map(&shifted_vertices(at, vertices)?, &mut mapped)?;
            polygon_extent(&mapped)
        }
        Shape::Cross { arm, thickness } => {
            if *arm < int_ratio(0) || *thickness <= int_ratio(0) {
                return Ok(BoundsOrEmpty::Empty);
            }
            let half = thickness.checked_div(int_ratio(2))?;
            map(&rect_corners(at, *arm, half)?, &mut mapped)?;
            map(&rect_corners(at, half, *arm)?, &mut mapped)?;
            polygon_extent(&mapped)
        }
    }
}

/// Evaluates where `object` is at `slot.frame` under `camera`.
///
/// # Errors
///
/// Returns [`RenderError::ObjectOverflow`] when any value overflows exact
/// arithmetic.
pub(super) fn placement(
    object: &Object,
    camera: &CameraFrame,
    slot: Slot,
) -> Result<ObjectState, RenderError> {
    let centre_scene =
        crate::motion::position_at(&object.motion, slot.frame).map_err(|_| slot.overflow())?;
    let centre_screen = camera
        .transform
        .apply(centre_scene)
        .map_err(|_| slot.overflow())?;
    let extent = screen_extent(&object.shape, centre_scene, camera).map_err(|_| slot.overflow())?;
    Ok(ObjectState {
        index: slot.object,
        centre_scene,
        centre_screen,
        extent,
    })
}

/// Returns the state of every object visible at `frame`, in declaration
/// order.
///
/// An object whose visibility span contains `frame` is reported even when it
/// draws nothing; its extent is then [`BoundsOrEmpty::Empty`].
/// [`render_into`](super::render_into) takes the same placements, and the
/// extent is built from the same geometry helpers as the drawing code.
///
/// # Errors
///
/// Returns [`RenderError::CameraOverflow`] or [`RenderError::ObjectOverflow`]
/// when a placement overflows exact arithmetic.
pub fn object_states(scene: &Scene, frame: FrameIndex) -> Result<Vec<ObjectState>, RenderError> {
    let camera = camera_frame(&scene.camera, frame)?;
    let mut states = Vec::new();
    for (position, object) in scene.objects.iter().enumerate() {
        if object.visible.contains(frame) {
            let slot = Slot {
                frame,
                object: object_index(frame, position)?,
            };
            states.push(placement(object, &camera, slot)?);
        }
    }
    Ok(states)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Camera, Magnification, Rotation, Turns, Zoom};
    use crate::color::Rgb8;
    use crate::raster::Bounds;
    use crate::scene::{Background, FrameSpan, Motion};
    use crate::testutil::make_ratio;
    use crate::units::{Dimensions, FrameCount, FrameRate, Height, Width};

    fn r(numer: i64, denom: i64) -> crate::ratio::Ratio {
        make_ratio(numer, denom).unwrap()
    }

    fn pt(x: (i64, i64), y: (i64, i64)) -> Point {
        Point::new(r(x.0, x.1), r(y.0, y.1))
    }

    fn bounds(min: Point, max: Point) -> BoundsOrEmpty {
        BoundsOrEmpty::Covering(Bounds::new(min.x, min.y, max.x, max.y).unwrap())
    }

    fn span(start: u32, end: u32) -> FrameSpan {
        FrameSpan::new(FrameIndex::new(start), FrameIndex::new(end)).unwrap()
    }

    /// Camera at (1, 1), a quarter turn, magnification 3/2.
    ///
    /// The scene-to-screen map is `spin * zoom * (w - centre)` with `spin`
    /// the rotation by the negated angle, i.e. by -1/4 turn, which sends
    /// `(x, y)` to `(y, -x)`. Hence `w -> (3/2) * (w.y - 1, -(w.x - 1))`.
    fn quarter_turn_scene(objects: Vec<Object>) -> Scene {
        let camera = Camera::new(
            Motion::Fixed(pt((1, 1), (1, 1))),
            Rotation::Fixed(Turns::new(r(1, 4))),
            Zoom::Fixed(Magnification::new(r(3, 2)).unwrap()),
        );
        Scene::new(
            Dimensions::new(Width::new(8).unwrap(), Height::new(8).unwrap()),
            FrameRate::from_fps(30).unwrap(),
            FrameCount::new(4).unwrap(),
            None,
            Background::Solid(Rgb8::new(0, 0, 0)),
            camera,
            objects,
        )
    }

    fn fixed(shape: Shape, at: Point, visible: FrameSpan) -> Object {
        Object::new(shape, Rgb8::new(255, 0, 0), Motion::Fixed(at), visible)
    }

    #[test]
    fn test_object_states_under_a_turned_zoomed_camera() {
        let scene = quarter_turn_scene(vec![
            // Declared first, visible only at frames 2 and 3.
            fixed(
                Shape::Disc { radius: r(1, 1) },
                pt((0, 1), (0, 1)),
                span(2, 4),
            ),
            // Disc radius 2 at (4, 2): offset from the camera (3, 1), which
            // turns to (1, -3) and magnifies to (3/2, -9/2). Radius 2 * 3/2
            // = 3, so the box is [-3/2, 9/2] x [-15/2, -3/2].
            fixed(
                Shape::Disc { radius: r(2, 1) },
                pt((4, 1), (2, 1)),
                span(0, 3),
            ),
            // Rect half-extents 2 x 1 at (4, 2): corners (2,1) (6,1) (6,3)
            // (2,3); offsets (1,0) (5,0) (5,2) (1,2); turned (0,-1) (0,-5)
            // (2,-5) (2,-1); magnified (0,-3/2) (0,-15/2) (3,-15/2)
            // (3,-3/2). Box [0, 3] x [-15/2, -3/2].
            fixed(
                Shape::Rect {
                    half_width: r(2, 1),
                    half_height: r(1, 1),
                },
                pt((4, 1), (2, 1)),
                span(0, 4),
            ),
        ]);

        let at_frame_1 = object_states(&scene, FrameIndex::new(1)).unwrap();
        assert_eq!(
            at_frame_1,
            vec![
                ObjectState {
                    index: ObjectIndex::new(1),
                    centre_scene: pt((4, 1), (2, 1)),
                    centre_screen: pt((3, 2), (-9, 2)),
                    extent: bounds(pt((-3, 2), (-15, 2)), pt((9, 2), (-3, 2))),
                },
                ObjectState {
                    index: ObjectIndex::new(2),
                    centre_scene: pt((4, 1), (2, 1)),
                    centre_screen: pt((3, 2), (-9, 2)),
                    extent: bounds(pt((0, 1), (-15, 2)), pt((3, 1), (-3, 2))),
                },
            ]
        );

        // At frame 2 the first object appears, in declaration order, and
        // the second is still visible; at frame 3 the second has gone.
        // Disc radius 1 at (0, 0): offset (-1, -1), turned (-1, 1),
        // magnified (-3/2, 3/2), radius 3/2: [-3, 0] x [0, 3].
        let at_frame_2 = object_states(&scene, FrameIndex::new(2)).unwrap();
        let indices: Vec<_> = at_frame_2.iter().map(|s| s.index).collect();
        assert_eq!(
            indices,
            vec![
                ObjectIndex::new(0),
                ObjectIndex::new(1),
                ObjectIndex::new(2)
            ]
        );
        assert_eq!(
            at_frame_2.first().map(|s| s.extent),
            Some(bounds(pt((-3, 1), (0, 1)), pt((0, 1), (3, 1))))
        );
        let at_frame_3 = object_states(&scene, FrameIndex::new(3)).unwrap();
        let remaining: Vec<_> = at_frame_3.iter().map(|s| s.index).collect();
        assert_eq!(remaining, vec![ObjectIndex::new(0), ObjectIndex::new(2)]);
    }

    #[test]
    fn test_degenerate_shape_is_visible_but_empty() {
        let scene = quarter_turn_scene(vec![fixed(
            Shape::Cross {
                arm: r(5, 1),
                thickness: r(0, 1),
            },
            pt((4, 1), (2, 1)),
            span(0, 4),
        )]);
        let states = object_states(&scene, FrameIndex::new(0)).unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states.first().map(|s| s.extent), Some(BoundsOrEmpty::Empty));
    }
}
