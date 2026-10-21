//! The per-frame state of every object a frame draws, as the manifest
//! records it.
//!
//! The placement comes from `synthvid_scene::object_states`. The renderer takes
//! the same placements, and the extent is built from the same geometry helpers
//! as the drawing code, so the manifest reports what the renderer draws.

use core::num::NonZeroI64;

use crate::frame_bounds::FrameBounds;
use crate::manifest::{FrameObjectState, ManifestBounds, ManifestPoint, OnScreen};
use crate::manifest_values::OnScreenError;
use crate::render::{GenerationError, ObjectStep};
use synthvid_scene::raster::Corners;
use synthvid_scene::{
    object_states, Bounds, BoundsError, BoundsOrEmpty, FrameIndex, Numerator, ObjectState,
    Overflow, Point, Ratio, Scale, Scene,
};

/// Returns the fraction of `bbox` that lies inside `frame_bounds`, exactly.
///
/// A box disjoint from the frame is `0`, not an error. A box with no area (a
/// degenerate polygon) has no fraction to take, and is also `0`.
fn visible_fraction(bbox: Bounds, frame_bounds: FrameBounds) -> Result<Ratio, GenerationError> {
    let area = bbox
        .area()
        .map_err(|Overflow| GenerationError::ObjectState(ObjectStep::BoxArea))?;
    if area == Ratio::from_integer(Numerator(0)) {
        return Ok(Ratio::from_integer(Numerator(0)));
    }
    match bbox.intersect(frame_bounds.0) {
        Ok(inside) => inside
            .area()
            .map_err(|Overflow| GenerationError::ObjectState(ObjectStep::VisibleArea))?
            .checked_div(area)
            .map_err(|Overflow| GenerationError::ObjectState(ObjectStep::VisibleFraction)),
        Err(BoundsError::Inverted) => Ok(Ratio::from_integer(Numerator(0))),
    }
}

/// Divides a scene-space point by the scale to give world units.
fn world_centre(centre: Point, scale: Scale) -> Result<ManifestPoint, GenerationError> {
    let units = scale.get();
    let divide = |coordinate: Ratio| {
        coordinate
            .checked_div(units)
            .map_err(|Overflow| GenerationError::ObjectState(ObjectStep::WorldCoordinate))
    };
    Ok(ManifestPoint::from_point(Point {
        across: divide(centre.across)?,
        down: divide(centre.down)?,
    }))
}

/// Records every object the renderer draws at `frame`.
///
/// [`synthvid_scene::render_into`] takes the same placements from the scene
/// crate's `object_states`, and the extent is built from the same geometry
/// helpers as the drawing code. An object whose span contains the frame but
/// whose extent is empty draws nothing, and is omitted.
///
/// # Errors
///
/// Returns [`GenerationError::SceneFrameRender`] when a placement overflows exact
/// arithmetic, and [`GenerationError::ObjectState`] when a box area, a visible fraction,
/// or a world coordinate does.
pub fn frame_objects(
    scene: &Scene,
    frame: FrameIndex,
) -> Result<Vec<FrameObjectState>, GenerationError> {
    let frame_bounds = FrameBounds(
        Bounds::new(Corners {
            min: Point {
                across: Ratio::from_integer(Numerator(0)),
                down: Ratio::from_integer(Numerator(0)),
            },
            max: Point {
                across: Ratio::from_integer(Numerator(
                    NonZeroI64::from(scene.dimensions.width.get()).get(),
                )),
                down: Ratio::from_integer(Numerator(
                    NonZeroI64::from(scene.dimensions.height.get()).get(),
                )),
            },
        })
        .map_err(|BoundsError::Inverted| GenerationError::ObjectState(ObjectStep::FrameBounds))?,
    );
    let states = object_states(scene, frame).map_err(GenerationError::SceneFrameRender)?;
    let mut objects = Vec::new();
    for state in states {
        match state.extent {
            BoundsOrEmpty::Covering(bbox) => {
                objects.push(object_state(scene, &state, bbox, frame_bounds)?);
            }
            BoundsOrEmpty::Empty => {}
        }
    }
    Ok(objects)
}

/// Records one object whose extent covers `bbox`.
fn object_state(
    scene: &Scene,
    state: &ObjectState,
    bbox: Bounds,
    frame_bounds: FrameBounds,
) -> Result<FrameObjectState, GenerationError> {
    let on_screen = OnScreen::new(visible_fraction(bbox, frame_bounds)?).map_err(
        |OnScreenError::OutOfRange| GenerationError::ObjectState(ObjectStep::OnScreenValue),
    )?;
    let centre_world = scene
        .scale
        .map(|scale| world_centre(state.centre_scene, scale))
        .transpose()?;
    Ok(FrameObjectState::new(
        state.index,
        ManifestBounds::from_bounds(bbox),
        ManifestPoint::from_point(state.centre_screen),
        centre_world,
        on_screen,
    ))
}

#[cfg(test)]
mod tests;
