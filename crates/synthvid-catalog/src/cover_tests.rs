//! Tests for catalog generation.

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::collections::BTreeSet;
    use synthvid_encode::defects::Defect;
    use synthvid_scene::{Dimensions, Height, Width};

    #[test]
    fn test_entry_names_are_unique() {
        let catalogue = generate_catalogue().unwrap();
        let mut names = BTreeSet::new();
        for entry in &catalogue {
            let name = entry.manifest_name();
            assert!(
                names.insert(name.as_str().to_owned()),
                "entry names must be unique"
            );
        }
    }

    #[test]
    fn test_entry_names_are_stable() {
        let entry = sample_entry_for_test();

        let name1 = entry.manifest_name();
        let name2 = entry.manifest_name();
        assert_eq!(name1.as_str(), name2.as_str(), "entry names must be stable");
    }

    #[test]
    fn test_entry_axis_differences_produce_different_names() {
        let entry1 = sample_entry_for_test();

        let mut entry2 = entry1.clone();
        entry2.frame_rate = FrameRateValue::Sixty; // Change one axis

        let name1 = entry1.manifest_name();
        let name2 = entry2.manifest_name();
        assert_ne!(
            name1.as_str(),
            name2.as_str(),
            "different axes must produce different names"
        );
    }

    #[test]
    fn test_all_entries_have_names() {
        let catalogue = generate_catalogue().unwrap();
        assert!(!catalogue.is_empty(), "catalog must have entries");
        for entry in &catalogue {
            let _manifest = entry.manifest_name();
            let _media = entry.media_name();
            // If we reach here, names were successfully generated
        }
    }

    #[test]
    fn test_catalogue_includes_with_scale() {
        let catalogue = generate_catalogue().unwrap();
        let has_with_scale = catalogue.iter().any(|e| e.scale == ScaleAxis::WithScale);
        assert!(has_with_scale, "catalog must include entries with scale");
    }

    #[test]
    fn test_catalogue_includes_without_scale() {
        let catalogue = generate_catalogue().unwrap();
        let has_without_scale = catalogue.iter().any(|e| e.scale == ScaleAxis::NoScale);
        assert!(
            has_without_scale,
            "catalog must include entries without scale"
        );
    }

    #[test]
    fn test_catalogue_includes_large_frame_count() {
        let catalogue = generate_catalogue().unwrap();
        let has_large = catalogue.iter().any(|e| e.frame_count.get() >= 20000);
        assert!(
            has_large,
            "catalog must include entries with frame count >= 20000"
        );
    }

    #[test]
    fn test_catalogue_includes_all_track_matrices() {
        let catalogue = generate_catalogue().unwrap();
        let matrices: BTreeSet<_> = catalogue.iter().map(|e| e.track_matrix).collect();
        assert!(
            matrices.contains(&TrackMatrixAxis::Identity),
            "must include identity matrix"
        );
        assert!(
            matrices.contains(&TrackMatrixAxis::Rotate90),
            "must include 90-degree rotation"
        );
        assert!(
            matrices.contains(&TrackMatrixAxis::Rotate180),
            "must include 180-degree rotation"
        );
        assert!(
            matrices.contains(&TrackMatrixAxis::Rotate270),
            "must include 270-degree rotation"
        );
    }

    #[test]
    fn test_catalogue_includes_all_defect_variants() {
        let catalogue = generate_catalogue().unwrap();

        // Check that all discriminants are present
        let mut found_truncate_before = false;
        let mut found_truncate_within = false;
        let mut found_zero_length = false;
        let mut found_oversized_len = false;
        let mut found_undersized_len = false;
        let mut found_offset_past_end = false;
        let mut found_duplicate_box = false;
        let mut found_unknown_box = false;
        let mut found_zero_frame_rate = false;
        let mut found_zero_dimension = false;
        let mut found_absurd_frame_rate = false;
        let mut found_non_monotonic = false;
        let mut found_declared_count_mismatch = false;
        let mut found_reordered_boxes = false;
        let mut found_none = false;

        for entry in &catalogue {
            match &entry.defect {
                None => found_none = true,
                Some(Defect::TruncateBefore(_)) => found_truncate_before = true,
                Some(Defect::TruncateWithin { .. }) => found_truncate_within = true,
                Some(Defect::ZeroLengthPayload(_)) => found_zero_length = true,
                Some(Defect::OversizedBoxLength(_)) => found_oversized_len = true,
                Some(Defect::UndersizedBoxLength(_)) => found_undersized_len = true,
                Some(Defect::OffsetPastEnd(_)) => found_offset_past_end = true,
                Some(Defect::DuplicateBox(_)) => found_duplicate_box = true,
                Some(Defect::UnknownBox { .. }) => found_unknown_box = true,
                Some(Defect::ZeroFrameRate) => found_zero_frame_rate = true,
                Some(Defect::ZeroDimension(_)) => found_zero_dimension = true,
                Some(Defect::AbsurdFrameRate(_)) => found_absurd_frame_rate = true,
                Some(Defect::NonMonotonicTimestamps) => found_non_monotonic = true,
                Some(Defect::DeclaredCountMismatch { .. }) => found_declared_count_mismatch = true,
                Some(Defect::ReorderedBoxes(_, _)) => found_reordered_boxes = true,
            }
        }

        assert!(found_none, "must include no-defect entries");
        assert!(found_truncate_before, "must include TruncateBefore");
        assert!(found_truncate_within, "must include TruncateWithin");
        assert!(found_zero_length, "must include ZeroLengthPayload");
        assert!(found_oversized_len, "must include OversizedBoxLength");
        assert!(found_undersized_len, "must include UndersizedBoxLength");
        assert!(found_offset_past_end, "must include OffsetPastEnd");
        assert!(found_duplicate_box, "must include DuplicateBox");
        assert!(found_unknown_box, "must include UnknownBox");
        assert!(found_zero_frame_rate, "must include ZeroFrameRate");
        assert!(found_zero_dimension, "must include ZeroDimension");
        assert!(found_absurd_frame_rate, "must include AbsurdFrameRate");
        assert!(found_non_monotonic, "must include NonMonotonicTimestamps");
        assert!(
            found_declared_count_mismatch,
            "must include DeclaredCountMismatch"
        );
        assert!(found_reordered_boxes, "must include ReorderedBoxes");
    }

    #[test]
    fn test_media_names_have_correct_extension() {
        let entry_iso = sample_entry_for_test();

        let media_name_iso = entry_iso.media_name();
        let iso_path = std::path::Path::new(media_name_iso.as_str());
        assert!(
            matches!(
                iso_path.extension().and_then(|e| e.to_str()),
                Some(ext) if ext.eq_ignore_ascii_case("mp4")
            ),
            "ISO container must use .mp4 extension"
        );

        let mut entry_avi = entry_iso;
        entry_avi.container = ContainerAxis::Avi;
        let media_name_avi = entry_avi.media_name();
        let avi_path = std::path::Path::new(media_name_avi.as_str());
        assert!(
            matches!(
                avi_path.extension().and_then(|e| e.to_str()),
                Some(ext) if ext.eq_ignore_ascii_case("avi")
            ),
            "AVI container must use .avi extension"
        );
    }

    #[test]
    fn test_pairwise_coverage_dimensions_and_frame_count() {
        let catalogue = generate_catalogue().unwrap();
        let dimensions = [
            cover_axes::make_dimensions(cover_axes::RawSize {
                width: 16,
                height: 16,
            })
            .unwrap(),
            cover_axes::make_dimensions(cover_axes::RawSize {
                width: 320,
                height: 240,
            })
            .unwrap(),
            cover_axes::make_dimensions(cover_axes::RawSize {
                width: 640,
                height: 480,
            })
            .unwrap(),
            cover_axes::make_dimensions(cover_axes::RawSize {
                width: 1280,
                height: 720,
            })
            .unwrap(),
            cover_axes::make_dimensions(cover_axes::RawSize {
                width: 1920,
                height: 1080,
            })
            .unwrap(),
        ];
        let frame_counts = [
            FrameCountValue::new(1),
            FrameCountValue::new(10),
            FrameCountValue::new(100),
            FrameCountValue::new(1000),
            FrameCountValue::new(20000),
        ];

        // Every dimension should pair with every frame count that fits the
        // volume cap; the others would render gigabytes.
        for dim in &dimensions {
            for fc in &frame_counts {
                let pixel_frames = u64::from(dim.width.get().get())
                    * u64::from(dim.height.get().get())
                    * u64::from(fc.get());
                if pixel_frames > 10_000_000 {
                    assert!(
                        !catalogue
                            .iter()
                            .any(|e| e.dimensions == *dim && e.frame_count == *fc),
                        "catalog must not hold the oversized pair ({dim:?}, {fc:?})"
                    );
                    continue;
                }
                let found = catalogue
                    .iter()
                    .any(|e| e.dimensions == *dim && e.frame_count == *fc);
                assert!(found, "catalog must have pair ({dim:?}, {fc:?})");
            }
        }
    }

    #[test]
    fn test_pairwise_coverage_background_and_shape() {
        let catalogue = generate_catalogue().unwrap();
        let backgrounds = [
            BackgroundAxis::Solid,
            BackgroundAxis::Checker,
            BackgroundAxis::Gradient,
            BackgroundAxis::Blobs,
            BackgroundAxis::Grid,
        ];
        let shapes = [
            ShapeAxis::Disc,
            ShapeAxis::Rect,
            ShapeAxis::Polygon,
            ShapeAxis::Cross,
        ];

        // Every background should pair with every shape
        for bg in &backgrounds {
            for sh in &shapes {
                let found = catalogue
                    .iter()
                    .any(|e| e.background == *bg && e.shape == *sh);
                assert!(found, "catalog must have pair ({bg:?}, {sh:?})");
            }
        }
    }

    #[test]
    fn test_pairwise_coverage_coding_and_container() {
        let catalogue = generate_catalogue().unwrap();
        let codings = [CodingAxis::Raw, CodingAxis::MotionJpeg];
        let containers = [ContainerAxis::Iso, ContainerAxis::Avi];

        // Every coding should pair with every container
        for cod in &codings {
            for cont in &containers {
                let found = catalogue
                    .iter()
                    .any(|e| e.coding == *cod && e.container == *cont);
                assert!(found, "catalog must have pair ({cod:?}, {cont:?})");
            }
        }
    }

    #[test]
    fn test_pairwise_coverage_scale_and_frame_rate() {
        let catalogue = generate_catalogue().unwrap();
        let scales = [ScaleAxis::WithScale, ScaleAxis::NoScale];
        let frame_rates = [
            FrameRateValue::Thirty,
            FrameRateValue::Sixty,
            FrameRateValue::OneTwenty,
            FrameRateValue::TwoForty,
            FrameRateValue::Ntsc,
            FrameRateValue::Absurd,
        ];

        // Every scale should pair with every frame rate
        for sc in &scales {
            for fr in &frame_rates {
                let found = catalogue
                    .iter()
                    .any(|e| e.scale == *sc && e.frame_rate == *fr);
                assert!(found, "catalog must have pair ({sc:?}, {fr:?})");
            }
        }
    }

    #[test]
    fn test_no_entry_pairs_a_container_with_the_other_formats_defect() {
        let catalogue = generate_catalogue().unwrap();
        for entry in &catalogue {
            if let Some(defect) = &entry.defect {
                if let Some(kind) = cover_axes::defect_container_kind(defect) {
                    assert_eq!(
                        kind,
                        cover_axes::container_kind(entry.container),
                        "entry {} pairs a container with a defect addressed in the other format: {:?}",
                        entry.manifest_name().as_str(),
                        defect
                    );
                }
            }
        }
    }

    /// Every distinct container, coding and defect the catalogue combines,
    /// each as a small entry: the defect's address and the encoding it meets
    /// are what decide whether it applies, not the picture size.
    fn small_entry_per_defect_combination() -> Vec<Entry> {
        let size = Dimensions::new(Width::new(16).unwrap(), Height::new(16).unwrap());
        let mut seen: Vec<(ContainerAxis, CodingAxis, Defect)> = Vec::new();
        let mut small = Vec::new();
        for mut entry in generate_catalogue().unwrap() {
            let Some(defect) = entry.defect.clone() else {
                continue;
            };
            let combination = (entry.container, entry.coding, defect);
            if seen.contains(&combination) {
                continue;
            }
            seen.push(combination);
            entry.dimensions = size;
            entry.frame_count = FrameCountValue::new(3);
            small.push(entry);
        }
        small
    }

    #[test]
    fn test_every_defect_applies_to_every_container_and_coding_it_is_paired_with() {
        for entry in small_entry_per_defect_combination() {
            let generated = crate::generator::generate_entry(&entry);
            assert!(
                generated.is_ok(),
                "{:?} on {:?} {:?} fails to apply: {:?}",
                entry.defect,
                entry.container,
                entry.coding,
                generated.err()
            );
        }
    }

    #[test]
    fn test_both_formats_get_both_truncations() {
        use synthvid_encode::defects::ContainerKind;
        let catalogue = generate_catalogue().unwrap();
        for kind in [ContainerKind::Iso, ContainerKind::Avi] {
            for want_before in [true, false] {
                assert!(
                    catalogue.iter().any(|e| match &e.defect {
                        Some(Defect::TruncateBefore(a)) => want_before && a.kind() == kind,
                        Some(Defect::TruncateWithin { address, .. }) =>
                            !want_before && address.kind() == kind,
                        _ => false,
                    }),
                    "no {kind:?} entry with truncate-before={want_before}"
                );
            }
        }
    }
}
