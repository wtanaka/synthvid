//! The per-frame state of every object a frame draws, as the manifest
//! records it.
//!
//! The placement comes from `synthvid_scene::object_states`. The renderer takes
//! the same placements, and the extent is built from the same geometry helpers
//! as the drawing code, so the manifest reports what the renderer draws.

use crate::generator::GenerationError;
use crate::manifest::{FrameObjectState, ManifestBounds, ManifestPoint, OnScreen};
use synthvid_scene::{
    object_states, Bounds, BoundsError, BoundsOrEmpty, FrameIndex, Point, Ratio, Scale, Scene,
};

/// Names a failure to evaluate the per-frame state of the objects.
fn object_state_error(what: &str) -> GenerationError {
    GenerationError::RenderError(format!("object state: {what}"))
}

/// Returns the fraction of `bbox` that lies inside `frame_bounds`, exactly.
///
/// A box disjoint from the frame is `0`, not an error. A box with no area (a
/// degenerate polygon) has no fraction to take, and is also `0`.
fn visible_fraction(bbox: Bounds, frame_bounds: Bounds) -> Result<Ratio, GenerationError> {
    let area = bbox
        .area()
        .map_err(|_| object_state_error("bbox area overflow"))?;
    if area == Ratio::from_integer(0) {
        return Ok(Ratio::from_integer(0));
    }
    let inside = match bbox.intersect(frame_bounds) {
        Ok(inside) => inside,
        Err(BoundsError::Inverted) => return Ok(Ratio::from_integer(0)),
    };
    inside
        .area()
        .map_err(|_| object_state_error("visible area overflow"))?
        .checked_div(area)
        .map_err(|_| object_state_error("visible fraction overflow"))
}

/// Divides a scene-space point by the scale to give world units.
fn world_centre(centre: Point, scale: Scale) -> Result<ManifestPoint, GenerationError> {
    let units = scale.get();
    let divide = |c: Ratio| {
        c.checked_div(units)
            .map_err(|_| object_state_error("world centre overflow"))
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
/// Returns [`GenerationError::RenderError`] when a placement, a box area, or
/// a world coordinate overflows exact arithmetic.
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
    .map_err(|_| object_state_error("frame bounds inverted"))?;
    let states =
        object_states(scene, frame).map_err(|e| GenerationError::RenderError(e.to_string()))?;
    let mut objects = Vec::new();
    for state in states {
        let BoundsOrEmpty::Covering(bbox) = state.extent else {
            continue;
        };
        let on_screen = OnScreen::new(visible_fraction(bbox, frame_bounds)?)
            .map_err(|e| object_state_error(&e.to_string()))?;
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
    use crate::cover::generate_catalogue;
    use crate::generator::generate_entry;
    use core::num::NonZeroI64;

    /// Generates the manifest of the catalogue entry called `name`.
    fn manifest_of(name: &str) -> String {
        let catalogue = generate_catalogue().expect("catalogue");
        let entry = catalogue
            .iter()
            .find(|e| e.manifest_name().as_str() == name)
            .expect("entry exists");
        generate_entry(entry)
            .expect("entry generates")
            .manifest_json()
            .to_owned()
    }

    /// Extracts the `objects` array of the first frame.
    fn first_frame_objects(manifest: &str) -> &str {
        let after = manifest
            .split_once("\"objects\":[")
            .expect("frame objects present")
            .1;
        after.split_once("}]}],").expect("end of objects").0
    }

    fn r(numer: i64, denom: i64) -> Ratio {
        Ratio::new(numer, NonZeroI64::new(denom).expect("nonzero denominator")).expect("ratio")
    }

    /// A 16x16 one-frame fixed disc of radius 20, centred on the frame at
    /// (8, 8), under the identity camera.
    ///
    /// Computed by hand, independently of the program:
    ///
    /// * bbox = [8 - 20, 8 + 20] on each axis = [-12, 28] x [-12, 28],
    ///   a 40 x 40 box of area 1600.
    /// * Intersected with the frame [0, 16] x [0, 16] it is the whole frame,
    ///   area 16 * 16 = 256.
    /// * `on_screen` = 256 / 1600 = 4 / 25.
    /// * `centre_screen` = (8, 8), since the camera is the identity.
    /// * scaled: 5/2 scene units per world unit, so `centre_world` is
    ///   8 / (5/2) = 16/5 on each axis.
    #[test]
    fn test_overhanging_disc_reports_hand_computed_state() {
        let manifest =
            manifest_of("0016x0016-1-30fps-solid-disc-fixed-raw-iso-identity-scaled-nodef");
        assert_eq!(
            first_frame_objects(&manifest),
            concat!(
                "{\"bbox\":{\"max_x\":{\"den\":1,\"num\":28},\"max_y\":{\"den\":1,\"num\":28},",
                "\"min_x\":{\"den\":1,\"num\":-12},\"min_y\":{\"den\":1,\"num\":-12}},",
                "\"centre_screen\":{\"x\":{\"den\":1,\"num\":8},\"y\":{\"den\":1,\"num\":8}},",
                "\"centre_world\":{\"x\":{\"den\":5,\"num\":16},\"y\":{\"den\":5,\"num\":16}},",
                "\"index\":0,\"on_screen\":{\"den\":25,\"num\":4}"
            )
        );
        assert!(manifest.contains("\"scale\":{\"den\":2,\"num\":5}"));
    }

    /// The same disc moving linearly: at frame 0 it starts at (w/4, h/2) =
    /// (4, 8), so the box is not symmetric about the frame.
    ///
    /// * bbox = [4 - 20, 4 + 20] x [8 - 20, 8 + 20] = [-16, 24] x [-12, 28],
    ///   area 40 * 40 = 1600; the frame lies inside it, area 256, so
    ///   `on_screen` = 4 / 25.
    /// * `centre_world` = (4 / (5/2), 8 / (5/2)) = (8/5, 16/5): different
    ///   coordinates, so a swapped x and y would show.
    #[test]
    fn test_off_centre_scaled_disc_reports_distinct_world_coordinates() {
        let manifest =
            manifest_of("0016x0016-1-60fps-solid-disc-linear-raw-iso-identity-scaled-trunc-box");
        let objects = first_frame_objects(&manifest);
        assert!(objects.contains(concat!(
            "\"bbox\":{\"max_x\":{\"den\":1,\"num\":24},\"max_y\":{\"den\":1,\"num\":28},",
            "\"min_x\":{\"den\":1,\"num\":-16},\"min_y\":{\"den\":1,\"num\":-12}}"
        )));
        assert!(objects.contains(
            "\"centre_world\":{\"x\":{\"den\":5,\"num\":8},\"y\":{\"den\":5,\"num\":16}}"
        ));
        assert!(objects.contains("\"on_screen\":{\"den\":25,\"num\":4}"));
    }

    #[test]
    fn test_unscaled_entry_has_no_world_centre() {
        let catalogue = generate_catalogue().expect("catalogue");
        let entry = catalogue
            .iter()
            .find(|e| e.scale == crate::cover::ScaleAxis::NoScale)
            .expect("an unscaled entry exists");
        let manifest = generate_entry(entry).expect("generates");
        assert!(manifest.manifest_json().contains("\"objects\":[{\"bbox\""));
        assert!(!manifest.manifest_json().contains("centre_world"));
        assert!(!manifest.manifest_json().contains("\"scale\""));
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
