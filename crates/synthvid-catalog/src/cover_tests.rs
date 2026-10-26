//! Tests for catalog generation.

#[cfg(test)]
mod tests {
    use super::super::*;
    extern crate alloc;
    use crate::generator::generate_entry;
    use alloc::collections::BTreeSet;
    use core::num::NonZeroU64;
    use std::path::Path;
    use synthvid_scene::{Height, Width};
    use synthvid_testkit::{ensure, ensure_eq, ensure_ne, refusal, Fail, OrFail as _};

    fn check_entry_names_are_unique() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        let mut names = BTreeSet::new();
        for entry in &catalogue {
            let name = entry.manifest_name();
            ensure!(
                names.insert(name.as_str().to_owned()),
                "entry names must be unique"
            );
        }
        Ok(())
    }

    #[test]
    fn entry_names_are_unique() {
        assert_eq!(check_entry_names_are_unique(), Ok(()));
    }

    fn check_entry_names_are_stable() -> Result<(), Fail> {
        let entry = sample_entry_for_test().or_fail()?;

        let name1 = entry.manifest_name();
        let name2 = entry.manifest_name();
        ensure_eq!(name1.as_str(), name2.as_str(), "entry names must be stable");
        Ok(())
    }

    #[test]
    fn entry_names_are_stable() {
        assert_eq!(check_entry_names_are_stable(), Ok(()));
    }

    fn check_entry_axis_differences_produce_different_names() -> Result<(), Fail> {
        let entry1 = sample_entry_for_test().or_fail()?;

        let mut entry2 = entry1.clone();
        entry2.frame_rate = FrameRateValue::Sixty; // Change one axis

        let name1 = entry1.manifest_name();
        let name2 = entry2.manifest_name();
        ensure_ne!(
            name1.as_str(),
            name2.as_str(),
            "different axes must produce different names"
        );
        Ok(())
    }

    #[test]
    fn entry_axis_differences_produce_different_names() {
        assert_eq!(
            check_entry_axis_differences_produce_different_names(),
            Ok(())
        );
    }

    fn check_all_entries_have_names() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        ensure!(!catalogue.is_empty(), "catalog must have entries");
        for entry in &catalogue {
            ensure!(
                !entry.manifest_name().as_str().is_empty(),
                "manifest name must be non-empty"
            );
            ensure!(
                !entry.media_name().to_string().is_empty(),
                "media name must be non-empty"
            );
        }
        Ok(())
    }

    #[test]
    fn all_entries_have_names() {
        assert_eq!(check_all_entries_have_names(), Ok(()));
    }

    fn check_catalogue_includes_with_scale() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        let has_with_scale = catalogue
            .iter()
            .any(|entry| entry.scale == ScaleAxis::Declared);
        ensure!(has_with_scale, "catalog must include entries with scale");
        Ok(())
    }

    #[test]
    fn catalogue_includes_with_scale() {
        assert_eq!(check_catalogue_includes_with_scale(), Ok(()));
    }

    fn check_catalogue_includes_without_scale() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        let has_without_scale = catalogue
            .iter()
            .any(|entry| entry.scale == ScaleAxis::Undeclared);
        ensure!(
            has_without_scale,
            "catalog must include entries without scale"
        );
        Ok(())
    }

    #[test]
    fn catalogue_includes_without_scale() {
        assert_eq!(check_catalogue_includes_without_scale(), Ok(()));
    }

    fn check_catalogue_includes_large_frame_count() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        let has_large = catalogue
            .iter()
            .any(|entry| entry.frame_count.get() >= 20000);
        ensure!(
            has_large,
            "catalog must include entries with frame count >= 20000"
        );
        Ok(())
    }

    #[test]
    fn catalogue_includes_large_frame_count() {
        assert_eq!(check_catalogue_includes_large_frame_count(), Ok(()));
    }

    fn check_catalogue_includes_all_track_matrices() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        let matrices: BTreeSet<_> = catalogue.iter().map(|entry| entry.track_matrix).collect();
        ensure!(
            matrices.contains(&TrackMatrixAxis::Identity),
            "must include identity matrix"
        );
        ensure!(
            matrices.contains(&TrackMatrixAxis::Rotate90),
            "must include 90-degree rotation"
        );
        ensure!(
            matrices.contains(&TrackMatrixAxis::Rotate180),
            "must include 180-degree rotation"
        );
        ensure!(
            matrices.contains(&TrackMatrixAxis::Rotate270),
            "must include 270-degree rotation"
        );
        Ok(())
    }

    #[test]
    fn catalogue_includes_all_track_matrices() {
        assert_eq!(check_catalogue_includes_all_track_matrices(), Ok(()));
    }

    /// The name of a defect variant.
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct VariantName(&'static str);

    /// The name of an entry's defect variant, `None` when it has none.
    ///
    /// The match is exhaustive, so a new variant cannot be added without
    /// naming it here.
    fn defect_variant(defect: Option<&Defect>) -> VariantName {
        match defect {
            None => VariantName("None"),
            Some(Defect::TruncateBefore(_)) => VariantName("TruncateBefore"),
            Some(Defect::TruncateWithin {
                address: _address,
                offset: _offset,
            }) => VariantName("TruncateWithin"),
            Some(Defect::ZeroLengthPayload(_)) => VariantName("ZeroLengthPayload"),
            Some(Defect::OversizedBoxLength(_)) => VariantName("OversizedBoxLength"),
            Some(Defect::UndersizedBoxLength(_)) => VariantName("UndersizedBoxLength"),
            Some(Defect::OffsetPastEnd(_)) => VariantName("OffsetPastEnd"),
            Some(Defect::DuplicateBox(_)) => VariantName("DuplicateBox"),
            Some(Defect::UnknownBox {
                after: _after,
                tag: _tag,
            }) => VariantName("UnknownBox"),
            Some(Defect::ZeroFrameRate) => VariantName("ZeroFrameRate"),
            Some(Defect::ZeroDimension(_)) => VariantName("ZeroDimension"),
            Some(Defect::NoContainerSize) => VariantName("NoContainerSize"),
            Some(Defect::AbsurdFrameRate(_)) => VariantName("AbsurdFrameRate"),
            Some(Defect::NonMonotonicTimestamps) => VariantName("NonMonotonicTimestamps"),
            Some(Defect::DeclaredCountMismatch {
                declared: _declared,
            }) => VariantName("DeclaredCountMismatch"),
            Some(Defect::ReorderedBoxes(_, _)) => VariantName("ReorderedBoxes"),
        }
    }

    fn check_catalogue_includes_all_defect_variants() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        let found: BTreeSet<VariantName> = catalogue
            .iter()
            .map(|entry| defect_variant(entry.defect.as_ref()))
            .collect();
        for variant in [
            "None",
            "TruncateBefore",
            "TruncateWithin",
            "ZeroLengthPayload",
            "OversizedBoxLength",
            "UndersizedBoxLength",
            "OffsetPastEnd",
            "DuplicateBox",
            "UnknownBox",
            "ZeroFrameRate",
            "ZeroDimension",
            "NoContainerSize",
            "AbsurdFrameRate",
            "NonMonotonicTimestamps",
            "DeclaredCountMismatch",
            "ReorderedBoxes",
        ] {
            ensure!(
                found.contains(&VariantName(variant)),
                "must include {variant}"
            );
        }
        Ok(())
    }

    #[test]
    fn catalogue_includes_all_defect_variants() {
        assert_eq!(check_catalogue_includes_all_defect_variants(), Ok(()));
    }

    fn check_media_names_have_correct_extension() -> Result<(), Fail> {
        let entry_iso = sample_entry_for_test().or_fail()?;

        let media_name_iso = entry_iso.media_name().to_string();
        let iso_path = Path::new(media_name_iso.as_str());
        ensure!(
            matches!(
                iso_path.extension().and_then(|extension| extension.to_str()),
                Some(ext) if ext.eq_ignore_ascii_case("mp4")
            ),
            "ISO container must use .mp4 extension"
        );

        let mut entry_avi = entry_iso;
        entry_avi.set_container_axis(ContainerAxis::Avi);
        let media_name_avi = entry_avi.media_name().to_string();
        let avi_path = Path::new(media_name_avi.as_str());
        ensure!(
            matches!(
                avi_path.extension().and_then(|extension| extension.to_str()),
                Some(ext) if ext.eq_ignore_ascii_case("avi")
            ),
            "AVI container must use .avi extension"
        );
        Ok(())
    }

    #[test]
    fn media_names_have_correct_extension() {
        assert_eq!(check_media_names_have_correct_extension(), Ok(()));
    }

    fn check_pairwise_coverage_dimensions_and_frame_count() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        let dimensions = [
            cover_axes::make_dimensions(cover_axes::RawSize {
                width: NonZeroU16::new(16),
                height: NonZeroU16::new(16),
            })
            .or_fail()?,
            cover_axes::make_dimensions(cover_axes::RawSize {
                width: NonZeroU16::new(320),
                height: NonZeroU16::new(240),
            })
            .or_fail()?,
            cover_axes::make_dimensions(cover_axes::RawSize {
                width: NonZeroU16::new(640),
                height: NonZeroU16::new(480),
            })
            .or_fail()?,
            cover_axes::make_dimensions(cover_axes::RawSize {
                width: NonZeroU16::new(1280),
                height: NonZeroU16::new(720),
            })
            .or_fail()?,
            cover_axes::make_dimensions(cover_axes::RawSize {
                width: NonZeroU16::new(1920),
                height: NonZeroU16::new(1080),
            })
            .or_fail()?,
        ];
        let frame_counts = [
            FrameCountValue::new(1).or_fail()?,
            FrameCountValue::new(10).or_fail()?,
            FrameCountValue::new(100).or_fail()?,
            FrameCountValue::new(1000).or_fail()?,
            FrameCountValue::new(20000).or_fail()?,
        ];

        // Every dimension should pair with every frame count that fits the
        // volume cap; the others would render gigabytes.
        for dim in &dimensions {
            for fc in &frame_counts {
                let pixel_frames = NonZeroU64::from(dim.width.get())
                    .get()
                    .checked_mul(NonZeroU64::from(dim.height.get()).get())
                    .or_fail()?
                    .checked_mul(u64::from(fc.get()))
                    .or_fail()?;
                if pixel_frames > 10_000_000 {
                    ensure!(
                        !catalogue
                            .iter()
                            .any(|entry| entry.dimensions == *dim && entry.frame_count == *fc),
                        "catalog must not hold the oversized pair ({dim:?}, {fc:?})"
                    );
                    continue;
                }
                let found = catalogue
                    .iter()
                    .any(|entry| entry.dimensions == *dim && entry.frame_count == *fc);
                ensure!(found, "catalog must have pair ({dim:?}, {fc:?})");
            }
        }
        Ok(())
    }

    #[test]
    fn pairwise_coverage_dimensions_and_frame_count() {
        assert_eq!(check_pairwise_coverage_dimensions_and_frame_count(), Ok(()));
    }

    fn check_pairwise_coverage_background_and_shape() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
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
                let found = catalogue.iter().any(|entry| {
                    entry
                        .scene()
                        .flat()
                        .is_some_and(|flat| flat.background() == *bg && flat.shape() == *sh)
                });
                ensure!(found, "catalog must have pair ({bg:?}, {sh:?})");
            }
        }
        Ok(())
    }

    #[test]
    fn pairwise_coverage_background_and_shape() {
        assert_eq!(check_pairwise_coverage_background_and_shape(), Ok(()));
    }

    fn check_pairwise_coverage_coding_and_container() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        let codings = [CodingAxis::Raw, CodingAxis::MotionJpeg];
        let containers = [ContainerAxis::Iso, ContainerAxis::Avi];

        // Every coding should pair with every container
        for cod in &codings {
            for cont in &containers {
                let found = catalogue
                    .iter()
                    .any(|entry| entry.coding == *cod && entry.container() == *cont);
                ensure!(found, "catalog must have pair ({cod:?}, {cont:?})");
            }
        }
        Ok(())
    }

    #[test]
    fn pairwise_coverage_coding_and_container() {
        assert_eq!(check_pairwise_coverage_coding_and_container(), Ok(()));
    }

    fn check_pairwise_coverage_scale_and_frame_rate() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        let scales = [ScaleAxis::Declared, ScaleAxis::Undeclared];
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
                    .any(|entry| entry.scale == *sc && entry.frame_rate == *fr);
                ensure!(found, "catalog must have pair ({sc:?}, {fr:?})");
            }
        }
        Ok(())
    }

    #[test]
    fn pairwise_coverage_scale_and_frame_rate() {
        assert_eq!(check_pairwise_coverage_scale_and_frame_rate(), Ok(()));
    }

    fn check_no_entry_pairs_a_container_with_the_other_formats_defect() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        for entry in &catalogue {
            if let Some(defect) = &entry.defect {
                if let Some(kind) = cover_axes::defect_container_kind(defect) {
                    ensure_eq!(
                        kind,
                        cover_axes::container_kind(entry.container()),
                        "entry {} pairs a container with a defect addressed in the other format: {:?}",
                        entry.manifest_name().as_str(),
                        defect
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn no_entry_pairs_a_container_with_the_other_formats_defect() {
        assert_eq!(
            check_no_entry_pairs_a_container_with_the_other_formats_defect(),
            Ok(())
        );
    }

    /// Every distinct container, coding and defect the catalogue combines,
    /// each as a small entry: the defect's address and the encoding it meets
    /// are what decide whether it applies, not the picture size.
    fn small_entry_per_defect_combination() -> Result<Vec<Entry>, Fail> {
        let size = Dimensions::new(Width::new(16).or_fail()?, Height::new(16).or_fail()?);
        let mut seen: Vec<(ContainerAxis, CodingAxis, Defect)> = Vec::new();
        let mut small = Vec::new();
        for mut entry in generate_catalogue().or_fail()? {
            let Some(defect) = entry.defect.clone() else {
                continue;
            };
            let combination = (entry.container(), entry.coding, defect);
            if seen.contains(&combination) {
                continue;
            }
            seen.push(combination);
            entry.dimensions = size;
            entry.frame_count = FrameCountValue::new(3).or_fail()?;
            small.push(entry);
        }
        Ok(small)
    }

    fn check_every_defect_applies_to_every_container_and_coding_it_is_paired_with(
    ) -> Result<(), Fail> {
        for entry in small_entry_per_defect_combination()? {
            let generated = generate_entry(&entry);
            if let Err(cause) = generated {
                return Err(Fail::here(format_args!(
                    "{:?} on {:?} {:?} fails to apply: {cause:?}",
                    entry.defect,
                    entry.container(),
                    entry.coding,
                )));
            }
        }
        Ok(())
    }

    #[test]
    fn every_defect_applies_to_every_container_and_coding_it_is_paired_with() {
        assert_eq!(
            check_every_defect_applies_to_every_container_and_coding_it_is_paired_with(),
            Ok(())
        );
    }

    fn check_both_formats_get_both_truncations() -> Result<(), Fail> {
        use synthvid_encode::defects::ContainerKind;
        let catalogue = generate_catalogue().or_fail()?;
        for kind in [ContainerKind::Iso, ContainerKind::Avi] {
            for want_before in [true, false] {
                ensure!(
                    catalogue.iter().any(|entry| match &entry.defect {
                        Some(Defect::TruncateBefore(address)) =>
                            want_before && address.kind() == kind,
                        Some(Defect::TruncateWithin {
                            address,
                            offset: _offset,
                        }) => !want_before && address.kind() == kind,
                        Some(Defect::UnknownBox {
                            after: _after,
                            tag: _tag,
                        }) => false,
                        Some(Defect::DeclaredCountMismatch {
                            declared: _declared,
                        }) => false,
                        Some(
                            Defect::ZeroLengthPayload(_)
                            | Defect::OversizedBoxLength(_)
                            | Defect::UndersizedBoxLength(_)
                            | Defect::OffsetPastEnd(_)
                            | Defect::DuplicateBox(_)
                            | Defect::ZeroFrameRate
                            | Defect::ZeroDimension(_)
                            | Defect::NoContainerSize
                            | Defect::AbsurdFrameRate(_)
                            | Defect::NonMonotonicTimestamps
                            | Defect::ReorderedBoxes(_, _),
                        )
                        | None => false,
                    }),
                    "no {kind:?} entry with truncate-before={want_before}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn both_formats_get_both_truncations() {
        assert_eq!(check_both_formats_get_both_truncations(), Ok(()));
    }

    /// The entry that omits the size from its container, with its coding and
    /// container replaced.
    fn no_dim_entry(coding: CodingAxis, container: ContainerAxis) -> Result<Entry, Fail> {
        entry_like(EntrySize::Tiny10, |entry| {
            entry.coding = coding;
            entry.set_container_axis(container);
            entry.defect = Some(Defect::NoContainerSize);
        })
    }

    fn check_size_omission_needs_mjpeg_in_iso() -> Result<(), Fail> {
        let refused = Err(SceneBuildError::VideoSpec(Box::new(
            synthvid_video::VideoSpecError::SizeOmissionNeedsMjpegInIso,
        )));
        for (coding, container) in [
            (CodingAxis::Raw, ContainerAxis::Iso),
            (CodingAxis::Raw, ContainerAxis::Avi),
            (CodingAxis::MotionJpeg, ContainerAxis::Avi),
        ] {
            let result = no_dim_entry(coding, container)?
                .to_video_spec()
                .map(|_spec| ());
            ensure_eq!(result, refused);
        }
        ensure!(
            no_dim_entry(CodingAxis::MotionJpeg, ContainerAxis::Iso)?
                .to_video_spec()
                .is_ok(),
            "motion-JPEG in ISO must be accepted"
        );
        Ok(())
    }

    #[test]
    fn size_omission_needs_mjpeg_in_iso() {
        assert_eq!(check_size_omission_needs_mjpeg_in_iso(), Ok(()));
    }

    fn check_render_refuses_size_omission_moved_to_avi() -> Result<(), Fail> {
        let spec = no_dim_entry(CodingAxis::MotionJpeg, ContainerAxis::Iso)?
            .to_video_spec()
            .or_fail()?
            .with_container(Container::Avi)
            .or_fail()?;
        ensure_eq!(
            synthvid_video::render(&spec).map(|_entry| ()),
            Err(synthvid_video::GenerationError::Specification(
                synthvid_video::VideoSpecError::SizeOmissionNeedsMjpegInIso
            ))
        );
        Ok(())
    }

    #[test]
    fn render_refuses_size_omission_moved_to_avi() {
        assert_eq!(check_render_refuses_size_omission_moved_to_avi(), Ok(()));
    }

    fn check_capture_names_are_derived_from_the_rate() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        let names: BTreeSet<String> = catalogue
            .iter()
            .map(|entry| entry.manifest_name().as_str().to_owned())
            .collect();
        for added in [
            "0016x0016-10-30fps-solid-disc-fixed-raw-iso-identity-unscaled-cap240-nodef",
            "0016x0016-10-ntsc-solid-disc-fixed-raw-iso-identity-unscaled-cap1000-nodef",
            "0016x0016-10-ntsc-solid-disc-fixed-raw-iso-identity-unscaled-cap24000x1001-nodef",
            "0016x0016-10-ntsc-solid-disc-fixed-raw-iso-identity-unscaled-cap25-nodef",
            "0016x0016-10-30fps-solid-disc-fixed-raw-iso-identity-unscaled-cap12-nodef",
            "0016x0016-10-30fps-solid-disc-fixed-raw-iso-identity-unscaled-cap120000x1001-nodef",
            "0016x0016-10-30fps-solid-disc-fixed-raw-iso-identity-unscaled-qtcap240-nodef",
            "0016x0016-10-ntsc-solid-disc-fixed-raw-iso-identity-unscaled-qtcap24000x1001-nodef",
            "0016x0016-10-30fps-solid-disc-fixed-raw-iso-identity-unscaled-qtcap12-nodef",
        ] {
            ensure!(names.contains(added), "missing capture entry: {added}");
        }
        Ok(())
    }

    #[test]
    fn capture_names_are_derived_from_the_rate() {
        assert_eq!(check_capture_names_are_derived_from_the_rate(), Ok(()));
    }

    fn check_catalogue_has_152_entries_nine_captured() -> Result<(), Fail> {
        let catalogue = generate_catalogue().or_fail()?;
        ensure_eq!(catalogue.len(), 152);
        let captured = catalogue
            .iter()
            .filter(|entry| entry.capture() != CaptureRate::PlaybackOnly)
            .count();
        ensure_eq!(captured, 9);
        Ok(())
    }

    #[test]
    fn catalogue_has_152_entries_nine_captured() {
        assert_eq!(check_catalogue_has_152_entries_nine_captured(), Ok(()));
    }

    /// A capture pair whose capture equals its playback is refused when its singleton is
    /// built, so the catalogue cannot contain it.
    #[test]
    fn capture_pair_equal_to_playback_is_refused_at_build_time() {
        use super::super::cover_capture_pairs::{
            CaptureDenominator, CaptureFraction, CaptureNumerator, CapturePair,
        };
        let pair = CapturePair {
            playback: FrameRateValue::Thirty,
            capture: CaptureFraction {
                numer: CaptureNumerator(30),
                denom: CaptureDenominator(1),
            },
            carrier: synthvid_scene::CaptureCarrier::ProjectBox,
        };
        let outcome = refusal(capture_singleton(&pair)).and_then(|error| {
            ensure_eq!(error, CatalogError::CaptureEqualsPlayback);
            Ok(())
        });
        assert_eq!(outcome, Ok(()));
    }
}
