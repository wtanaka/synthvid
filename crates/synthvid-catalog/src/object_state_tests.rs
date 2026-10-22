//! Tests of the per-frame object state a catalogue entry's manifest records.

use crate::cover::{
    entry_like, generate_catalogue, AxisValues, BackgroundAxis, CodingAxis, ContainerAxis,
    EntrySize, FrameRateValue, MotionAxis, ScaleAxis, ShapeAxis, TrackMatrixAxis,
};
use crate::generator::generate_entry;
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};
use synthvid_video::ManifestJson;

/// The `objects` array of a manifest's first frame, as text.
#[derive(Copy, Clone, Debug)]
struct FrameObjects<'json>(&'json str);

/// Generates the manifest of a 16 by 16, one-frame solid-background disc
/// entry with the given motion and frame rate. The manifest does not
/// depend on the defect, container or coding, so those are fixed.
fn manifest_of(frame_rate: FrameRateValue, motion: MotionAxis) -> Result<ManifestJson, Fail> {
    let entry = entry_like(EntrySize::Tiny1, |entry| {
        entry.set_axes(AxisValues {
            frame_rate,
            background: BackgroundAxis::Solid,
            shape: ShapeAxis::Disc,
            motion,
            coding: CodingAxis::Raw,
            container: ContainerAxis::Iso,
            track_matrix: TrackMatrixAxis::Identity,
            scale: ScaleAxis::Declared,
            defect: None,
        });
    })
    .or_fail()?;
    Ok(generate_entry(&entry).or_fail()?.manifest_json().clone())
}

/// Extracts the `objects` array of the first frame.
fn first_frame_objects(manifest: &ManifestJson) -> Result<FrameObjects<'_>, Fail> {
    let after = manifest.as_str().split_once("\"objects\":[").or_fail()?.1;
    Ok(FrameObjects(after.split_once("}]}],").or_fail()?.0))
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
fn overhanging_disc_reports_hand_computed_state() {
    assert_eq!(check_overhanging_disc_reports_hand_computed_state(), Ok(()));
}

fn check_overhanging_disc_reports_hand_computed_state() -> Result<(), Fail> {
    let manifest = manifest_of(FrameRateValue::Thirty, MotionAxis::Fixed)?;
    ensure_eq!(
        first_frame_objects(&manifest)?.0,
        concat!(
            "{\"bbox\":{\"max_x\":{\"den\":1,\"num\":28},\"max_y\":{\"den\":1,\"num\":28},",
            "\"min_x\":{\"den\":1,\"num\":-12},\"min_y\":{\"den\":1,\"num\":-12}},",
            "\"centre_screen\":{\"x\":{\"den\":1,\"num\":8},\"y\":{\"den\":1,\"num\":8}},",
            "\"centre_world\":{\"x\":{\"den\":5,\"num\":16},\"y\":{\"den\":5,\"num\":16}},",
            "\"index\":0,\"on_screen\":{\"den\":25,\"num\":4}"
        )
    );
    ensure!(manifest
        .as_str()
        .contains("\"scale\":{\"den\":2,\"num\":5}"));
    Ok(())
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
fn off_centre_scaled_disc_reports_distinct_world_coordinates() {
    assert_eq!(
        check_off_centre_scaled_disc_reports_distinct_world_coordinates(),
        Ok(())
    );
}

fn check_off_centre_scaled_disc_reports_distinct_world_coordinates() -> Result<(), Fail> {
    let manifest = manifest_of(FrameRateValue::Sixty, MotionAxis::Linear)?;
    let FrameObjects(objects) = first_frame_objects(&manifest)?;
    ensure!(objects.contains(concat!(
        "\"bbox\":{\"max_x\":{\"den\":1,\"num\":24},\"max_y\":{\"den\":1,\"num\":28},",
        "\"min_x\":{\"den\":1,\"num\":-16},\"min_y\":{\"den\":1,\"num\":-12}}"
    )));
    ensure!(objects
        .contains("\"centre_world\":{\"x\":{\"den\":5,\"num\":8},\"y\":{\"den\":5,\"num\":16}}"));
    ensure!(objects.contains("\"on_screen\":{\"den\":25,\"num\":4}"));
    Ok(())
}

#[test]
fn unscaled_entry_has_no_world_centre() {
    assert_eq!(check_unscaled_entry_has_no_world_centre(), Ok(()));
}

fn check_unscaled_entry_has_no_world_centre() -> Result<(), Fail> {
    let catalogue = generate_catalogue().or_fail()?;
    let entry = catalogue
        .iter()
        .find(|entry| entry.scale() == ScaleAxis::Undeclared)
        .or_fail()?;
    let generated = generate_entry(entry).or_fail()?;
    let manifest = generated.manifest_json().as_str();
    ensure!(manifest.contains("\"objects\":[{\"bbox\""));
    ensure!(!manifest.contains("centre_world"));
    ensure!(!manifest.contains("\"scale\""));
    Ok(())
}
