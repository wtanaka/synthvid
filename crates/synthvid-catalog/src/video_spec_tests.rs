//! Tests that render catalogue entries through a video specification and check the bytes.

#[cfg(test)]
mod video_spec_test_cases {
    use crate::cover::{
        avi_truncated_entry, entry_like, generate_catalogue, BackgroundAxis, CodingAxis,
        ContainerAxis, FrameRateValue, MotionAxis, ScaleAxis, ShapeAxis, TrackMatrixAxis,
    };
    use crate::sha256;
    use synthvid_encode::defects::{BoxPath, BoxTag, Defect};
    use synthvid_video::{render, Container, DisplayRotation, Encoding};

    #[test]
    fn test_frozen_digest_mjpeg_avi_rot90() {
        let entry = avi_truncated_entry();

        let generated = crate::generator::generate_entry(&entry).expect("generate_entry failed");
        let manifest_digest = sha256(generated.manifest_json().as_bytes()).to_string();
        let media_digest = sha256(generated.media_bytes()).to_string();

        assert_eq!(
            manifest_digest,
            "bd9419358c2218049e88ca8ea0987ced463534ea1d0db9c36314bef5daae3c18"
        );
        assert_eq!(
            media_digest,
            "a7a9ef428a03c7f34b0be2d622afe37cf1898d871de3970d44d1af2f9f422167"
        );
        assert_eq!(generated.manifest_json().len(), 4231);
        // The RIFF header (12 bytes) and the `hdrl` list (200 bytes) come
        // before the `movi` list, which is where the cut falls.
        assert_eq!(generated.media_bytes().len(), 212);
        // The cut file is a prefix of the same file without the defect.
        let mut whole = entry;
        whole.defect = None;
        let whole = crate::generator::generate_entry(&whole).expect("generate_entry failed");
        assert_eq!(generated.media_bytes(), &whole.media_bytes()[..212]);
    }

    #[test]
    fn test_frozen_digest_raw_iso_rot270() {
        let entry = entry_like("0016x0016-10-", |e| {
            e.frame_rate = FrameRateValue::Ntsc;
            e.background = BackgroundAxis::Gradient;
            e.shape = ShapeAxis::Polygon;
            e.motion = MotionAxis::Fixed;
            e.coding = CodingAxis::Raw;
            e.container = ContainerAxis::Iso;
            e.track_matrix = TrackMatrixAxis::Rotate270;
            e.scale = ScaleAxis::NoScale;
            e.defect = Some(Defect::ReorderedBoxes(
                BoxPath::root().child(BoxTag::new(*b"moov")),
                BoxPath::root().child(BoxTag::new(*b"mdat")),
            ));
        });

        let generated = crate::generator::generate_entry(&entry).expect("generate_entry failed");
        let manifest_digest = sha256(generated.manifest_json().as_bytes()).to_string();
        let media_digest = sha256(generated.media_bytes()).to_string();

        assert_eq!(
            manifest_digest,
            "45e6a9fe7be0bb352ac7f7ec3460dca5dcf3576af973817535805630fa3587c1"
        );
        assert_eq!(
            media_digest,
            "a3b0fc24629da5fad7d776ce58d7d3e1619072385a84d0f67ca7ce5c70472fa7"
        );
        assert_eq!(generated.manifest_json().len(), 4253);
        assert_eq!(generated.media_bytes().len(), 8350);
    }

    #[test]
    fn test_frozen_digest_mjpeg_iso_rot180() {
        let entry = entry_like("0320x0240-10-", |e| {
            e.frame_rate = FrameRateValue::Thirty;
            e.background = BackgroundAxis::Grid;
            e.shape = ShapeAxis::Cross;
            e.motion = MotionAxis::Linear;
            e.coding = CodingAxis::MotionJpeg;
            e.container = ContainerAxis::Iso;
            e.track_matrix = TrackMatrixAxis::Rotate180;
            e.scale = ScaleAxis::WithScale;
            e.defect = Some(Defect::DuplicateBox(
                BoxPath::root()
                    .child(BoxTag::new(*b"moov"))
                    .child(BoxTag::new(*b"trak")),
            ));
        });

        let generated = crate::generator::generate_entry(&entry).expect("generate_entry failed");
        let manifest_digest = sha256(generated.manifest_json().as_bytes()).to_string();
        let media_digest = sha256(generated.media_bytes()).to_string();

        assert_eq!(
            manifest_digest,
            "6bcaedba7ac92261afd841376c1feb984f6ae1d76288515ec5362bdac94e1840"
        );
        assert_eq!(
            media_digest,
            "f0200ae32c2a7287b8764d81f9b22ebc48f585c3d0cc1fc0c0bbb0df1fedc0f4"
        );
        assert_eq!(generated.manifest_json().len(), 4_913);
        assert_eq!(generated.media_bytes().len(), 95_005);
    }

    #[test]
    fn test_frozen_digest_raw_iso_rot180_zero_fps_defect() {
        let entry = entry_like("0016x0016-1-", |e| {
            e.frame_rate = FrameRateValue::Thirty;
            e.background = BackgroundAxis::Gradient;
            e.shape = ShapeAxis::Disc;
            e.motion = MotionAxis::Fixed;
            e.coding = CodingAxis::Raw;
            e.container = ContainerAxis::Iso;
            e.track_matrix = TrackMatrixAxis::Rotate180;
            e.scale = ScaleAxis::WithScale;
            e.defect = Some(Defect::ZeroFrameRate);
        });

        let generated = crate::generator::generate_entry(&entry).expect("generate_entry failed");
        let manifest_digest = sha256(generated.manifest_json().as_bytes()).to_string();
        let media_digest = sha256(generated.media_bytes()).to_string();

        assert_eq!(
            manifest_digest,
            "e52713819ed859efd82877cb39b047d0e07547f1e3b6d7e95f6448d9d9abff6c"
        );
        assert_eq!(
            media_digest,
            "c206d384d6b32232ebcb4d40944f3b9f9524bef0b3786391fb5843a51a5f3eab"
        );
        assert_eq!(generated.manifest_json().len(), 770);
        assert_eq!(generated.media_bytes().len(), 1366);
    }

    #[test]
    fn test_16x16_name_and_axis_mapping() {
        let entries = generate_catalogue().expect("failed to generate catalogue");

        for entry in &entries {
            if entry.manifest_name().as_str().starts_with("0016x0016") {
                let spec = entry.to_video_spec().expect("to_video_spec failed");
                assert_eq!(spec.name().as_str(), entry.manifest_name().as_str());

                match entry.coding {
                    CodingAxis::Raw => {
                        assert_eq!(spec.encoding(), Encoding::Raw);
                    }
                    CodingAxis::MotionJpeg => {
                        assert!(matches!(spec.encoding(), Encoding::Mjpeg { .. }));
                    }
                }

                let rotation = match entry.track_matrix {
                    TrackMatrixAxis::Identity => DisplayRotation::Identity,
                    TrackMatrixAxis::Rotate90 => DisplayRotation::Rot90,
                    TrackMatrixAxis::Rotate180 => DisplayRotation::Rot180,
                    TrackMatrixAxis::Rotate270 => DisplayRotation::Rot270,
                };
                match entry.container {
                    ContainerAxis::Iso => {
                        assert_eq!(spec.container(), Container::Iso { rotation });
                    }
                    ContainerAxis::Avi => {
                        assert_eq!(spec.container(), Container::Avi);
                    }
                }
            }
        }
    }

    #[test]
    fn test_rotation_changes_iso_but_not_manifest() {
        let entries = generate_catalogue().expect("failed to generate catalogue");
        let spec = entries
            .iter()
            .map(|e| e.to_video_spec().expect("to_video_spec failed"))
            .find(|spec| {
                matches!(
                    spec.container(),
                    Container::Iso { rotation } if rotation != DisplayRotation::Identity
                ) && spec.defect().is_none()
            })
            .expect("rotated ISO spec not found");

        let has_defect = spec.defect().is_some();
        let rotated = render(&spec).expect("render failed");
        let identity = render(&spec.with_container(Container::Iso {
            rotation: DisplayRotation::Identity,
        }))
        .expect("render failed");

        assert_eq!(rotated.manifest_json(), identity.manifest_json());
        if !has_defect {
            assert_ne!(rotated.media_bytes(), identity.media_bytes());
        }
    }

    #[test]
    fn test_avi_entry_carries_no_rotation() {
        let entry = avi_truncated_entry();

        let spec = entry.to_video_spec().expect("to_video_spec failed");
        assert_eq!(spec.container(), Container::Avi);
    }

    #[test]
    fn test_frozen_digest_via_render_api() {
        let entry = avi_truncated_entry();

        let spec = entry.to_video_spec().expect("to_video_spec failed");
        let generated = render(&spec).expect("render failed");

        let manifest_digest = sha256(generated.manifest_json().as_bytes()).to_string();
        let media_digest = sha256(generated.media_bytes()).to_string();

        assert_eq!(
            manifest_digest,
            "bd9419358c2218049e88ca8ea0987ced463534ea1d0db9c36314bef5daae3c18"
        );
        assert_eq!(
            media_digest,
            "a7a9ef428a03c7f34b0be2d622afe37cf1898d871de3970d44d1af2f9f422167"
        );
    }
}
