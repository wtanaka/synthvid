//! Per-frame placement of objects: the single source of truth for where an
//! object is, on screen and in the scene, at one frame.
//!
//! [`object_states`] reports for every object visible at a frame its scene
//! centre, its screen centre, and its screen-space extent. The renderer takes
//! each object's scene position from the same [`placement`] call, and the
//! extent is built from the same geometry helpers (`rect_corners`,
//! `shifted_vertices`, and the view transform) that the drawing code uses.
//! The drawing code still rebuilds its own geometry rather than consuming the
//! mapped points computed here, so the two agree by sharing those helpers, not
//! by construction.

use crate::geom::Point;
use crate::motion::position_at;
use crate::raster::{disc_extent, polygon_extent, BoundsOrEmpty, HalfSize};
use crate::ratio::{Numerator, Overflow, Ratio};
use crate::scene::{Membership, Object, Scene, Shape};
use crate::units::{FrameIndex, ObjectIndex};
use crate::view::View;

use super::shape::{rect_corners, shifted_vertices};
use super::{view_frame, RenderError, ViewFrame};

/// Where one visible object is at one frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ObjectState {
    /// Position of the object in `Scene::objects`.
    pub index: ObjectIndex,
    /// The object's motion position, in scene space.
    pub centre_scene: Point,
    /// That position carried through the view, in screen space.
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

/// The position of an object in `Scene::objects`, counted from zero in
/// declaration order.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct ObjectPosition(pub(super) usize);

/// Names the object at `position` in `Scene::objects`.
///
/// # Errors
///
/// Returns [`RenderError::ObjectOverflow`] for a position beyond `u32`.
pub(super) fn object_index(
    frame: FrameIndex,
    position: ObjectPosition,
) -> Result<ObjectIndex, RenderError> {
    u32::try_from(position.0)
        .map(ObjectIndex::new)
        .map_err(Overflow::from)
        .map_err(|Overflow| RenderError::ObjectOverflow {
            frame,
            object: ObjectIndex::new(u32::MAX),
        })
}

/// Returns the screen-space extent of `shape` centred on `at`.
fn screen_extent(shape: &Shape, at: Point, view: &ViewFrame) -> Result<BoundsOrEmpty, Overflow> {
    let map = |points: &[Point], out: &mut Vec<Point>| -> Result<(), Overflow> {
        for point in points {
            out.push(view.transform.apply(*point)?);
        }
        Ok(())
    };
    let mut mapped = Vec::new();
    match shape {
        Shape::Disc { radius } => {
            let centre = view.transform.apply(at)?;
            disc_extent(centre, radius.checked_mul(view.zoom)?)
        }
        Shape::Rect {
            half_width,
            half_height,
        } => {
            let half = HalfSize {
                width: *half_width,
                height: *half_height,
            };
            map(&rect_corners(at, half)?, &mut mapped)?;
            polygon_extent(&mapped)
        }
        Shape::Polygon { vertices } => {
            map(&shifted_vertices(at, vertices)?, &mut mapped)?;
            polygon_extent(&mapped)
        }
        Shape::Cross { arm, thickness } => {
            if *arm < Ratio::from_integer(Numerator(0))
                || *thickness <= Ratio::from_integer(Numerator(0))
            {
                return Ok(BoundsOrEmpty::Empty);
            }
            let half = thickness.checked_div(Ratio::from_integer(Numerator(2)))?;
            map(
                &rect_corners(
                    at,
                    HalfSize {
                        width: *arm,
                        height: half,
                    },
                )?,
                &mut mapped,
            )?;
            map(
                &rect_corners(
                    at,
                    HalfSize {
                        width: half,
                        height: *arm,
                    },
                )?,
                &mut mapped,
            )?;
            polygon_extent(&mapped)
        }
    }
}

/// Evaluates where `object` is at `slot.frame` under `view`.
///
/// # Errors
///
/// Returns [`RenderError::ObjectOverflow`] when any value overflows exact
/// arithmetic.
pub(super) fn placement(
    object: &Object,
    view: &ViewFrame,
    slot: Slot,
) -> Result<ObjectState, RenderError> {
    let centre_scene =
        position_at(&object.motion, slot.frame).map_err(|Overflow| slot.overflow())?;
    let centre_screen = view
        .transform
        .apply(centre_scene)
        .map_err(|Overflow| slot.overflow())?;
    let extent =
        screen_extent(&object.shape, centre_scene, view).map_err(|Overflow| slot.overflow())?;
    Ok(ObjectState {
        index: slot.object,
        centre_scene,
        centre_screen,
        extent,
    })
}

/// Returns the state of every object visible at `frame`, in declaration
/// order, as seen through `view`.
///
/// An object whose visibility span contains `frame` is reported even when it
/// draws nothing; its extent is then [`BoundsOrEmpty::Empty`].
/// [`render_into`](super::render_into) takes the same placements, and the
/// extent is built from the same geometry helpers as the drawing code.
///
/// # Errors
///
/// Returns [`RenderError::ViewOverflow`] or [`RenderError::ObjectOverflow`]
/// when a placement overflows exact arithmetic.
pub fn object_states(
    scene: &Scene,
    view: &View,
    frame: FrameIndex,
) -> Result<Vec<ObjectState>, RenderError> {
    let placed = view_frame(view, frame)?;
    let mut states = Vec::new();
    for (position, object) in scene.objects.iter().enumerate() {
        if object.visible.contains(frame) == Membership::Inside {
            let slot = Slot {
                frame,
                object: object_index(frame, ObjectPosition(position))?,
            };
            states.push(placement(object, &placed, slot)?);
        }
    }
    Ok(states)
}

#[cfg(test)]
mod tests;
