//! The per-frame state of every object a frame draws, as the manifest
//! records it.
//!
//! The placement comes from `synthvid_scene::object_states`. The renderer takes
//! the same placements, and the extent is built from the same geometry helpers
//! as the drawing code, so the manifest reports what the renderer draws.

use crate::manifest::{FrameObjectState, ManifestBounds, ManifestPoint, OnScreen};
use crate::render::{GenerationError, ObjectStep};
use synthvid_scene::{
    object_states, Bounds, BoundsError, BoundsOrEmpty, FrameIndex, Point, Ratio, Scale, Scene,
};

/// Returns the fraction of `bbox` that lies inside `frame_bounds`, exactly.
///
/// A box disjoint from the frame is `0`, not an error. A box with no area (a
/// degenerate polygon) has no fraction to take, and is also `0`.
fn visible_fraction(bbox: Bounds, frame_bounds: Bounds) -> Result<Ratio, GenerationError> {
    let area = bbox
        .area()
        .map_err(|_| GenerationError::ObjectState(ObjectStep::BoxArea))?;
    if area == Ratio::from_integer(0) {
        return Ok(Ratio::from_integer(0));
    }
    let inside = match bbox.intersect(frame_bounds) {
        Ok(inside) => inside,
        Err(BoundsError::Inverted) => return Ok(Ratio::from_integer(0)),
    };
    inside
        .area()
        .map_err(|_| GenerationError::ObjectState(ObjectStep::VisibleArea))?
        .checked_div(area)
        .map_err(|_| GenerationError::ObjectState(ObjectStep::VisibleFraction))
}

/// Divides a scene-space point by the scale to give world units.
fn world_centre(centre: Point, scale: Scale) -> Result<ManifestPoint, GenerationError> {
    let units = scale.get();
    let divide = |c: Ratio| {
        c.checked_div(units)
            .map_err(|_| GenerationError::ObjectState(ObjectStep::WorldCoordinate))
    };
    Ok(ManifestPoint::new(divide(centre.x)?, divide(centre.y)?))
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
    let frame_bounds = Bounds::new(
        Ratio::from_integer(0),
        Ratio::from_integer(0),
        Ratio::from_integer(i64::from(scene.dimensions.width.get().get())),
        Ratio::from_integer(i64::from(scene.dimensions.height.get().get())),
    )
    .map_err(|_| GenerationError::ObjectState(ObjectStep::FrameBounds))?;
    let states = object_states(scene, frame).map_err(GenerationError::SceneFrameRender)?;
    let mut objects = Vec::new();
    for state in states {
        let BoundsOrEmpty::Covering(bbox) = state.extent else {
            continue;
        };
        let on_screen = OnScreen::new(visible_fraction(bbox, frame_bounds)?)
            .map_err(|_| GenerationError::ObjectState(ObjectStep::OnScreenValue))?;
        let centre_world = scene
            .scale
            .map(|scale| world_centre(state.centre_scene, scale))
            .transpose()?;
        objects.push(FrameObjectState::new(
            state.index,
            ManifestBounds::from_bounds(bbox),
            ManifestPoint::new(state.centre_screen.x, state.centre_screen.y),
            centre_world,
            on_screen,
        ));
    }
    Ok(objects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroI64;

    fn r(numer: i64, denom: i64) -> Ratio {
        Ratio::new(numer, NonZeroI64::new(denom).expect("nonzero denominator")).expect("ratio")
    }

    /// bbox [-1, 3] x [0, 2] (area 8) against the frame [0, 4] x [0, 4]:
    /// the overlap is [0, 3] x [0, 2], area 6, so 6 / 8 = 3 / 4.
    #[test]
    fn test_visible_fraction_partial_overlap() {
        let bbox = Bounds::new(r(-1, 1), r(0, 1), r(3, 1), r(2, 1)).unwrap();
        let frame = Bounds::new(r(0, 1), r(0, 1), r(4, 1), r(4, 1)).unwrap();
        assert_eq!(visible_fraction(bbox, frame), Ok(r(3, 4)));
    }

    /// A box entirely outside the frame, and a box with no area, are both
    /// not on screen, and neither is an error.
    #[test]
    fn test_visible_fraction_disjoint_and_degenerate_are_zero() {
        let frame = Bounds::new(r(0, 1), r(0, 1), r(4, 1), r(4, 1)).unwrap();
        let away = Bounds::new(r(5, 1), r(0, 1), r(7, 1), r(2, 1)).unwrap();
        assert_eq!(visible_fraction(away, frame), Ok(r(0, 1)));
        let flat = Bounds::new(r(1, 1), r(1, 1), r(3, 1), r(1, 1)).unwrap();
        assert_eq!(visible_fraction(flat, frame), Ok(r(0, 1)));
    }
}
