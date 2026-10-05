//! Tests of the per-frame object state a catalogue entry's manifest records.

#[cfg(test)]
mod object_state_test_cases {
    use crate::cover::{
        entry_like, generate_catalogue, BackgroundAxis, CodingAxis, ContainerAxis, FrameRateValue,
        MotionAxis, ScaleAxis, ShapeAxis, TrackMatrixAxis,
    };
    use crate::generator::generate_entry;

    /// Generates the manifest of a 16 by 16, one-frame solid-background disc
    /// entry with the given motion and frame rate. The manifest does not
    /// depend on the defect, container or coding, so those are fixed.
    fn manifest_of(frame_rate: FrameRateValue, motion: MotionAxis) -> String {
        let entry = entry_like("0016x0016-1-", |e| {
            e.frame_rate = frame_rate;
            e.background = BackgroundAxis::Solid;
            e.shape = ShapeAxis::Disc;
            e.motion = motion;
            e.coding = CodingAxis::Raw;
            e.container = ContainerAxis::Iso;
            e.track_matrix = TrackMatrixAxis::Identity;
            e.scale = ScaleAxis::WithScale;
            e.defect = None;
        });
        generate_entry(&entry)
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
        let manifest = manifest_of(FrameRateValue::Thirty, MotionAxis::Fixed);
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
        let manifest = manifest_of(FrameRateValue::Sixty, MotionAxis::Linear);
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
            .find(|e| e.scale == ScaleAxis::NoScale)
            .expect("an unscaled entry exists");
        let manifest = generate_entry(entry).expect("generates");
        assert!(manifest.manifest_json().contains("\"objects\":[{\"bbox\""));
        assert!(!manifest.manifest_json().contains("centre_world"));
        assert!(!manifest.manifest_json().contains("\"scale\""));
    }
}
