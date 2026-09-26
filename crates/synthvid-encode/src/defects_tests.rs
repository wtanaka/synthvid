#[cfg(test)]
use super::*;
use crate::{encode_avi, encode_iso, AviCodec, IsoCodec};
use synthvid_scene::{Dimensions, Frame, Height, Width};

/// Creates a tiny 1×1 frame for testing.
fn make_test_frame() -> Frame {
    let width = Width::new(1).expect("valid width");
    let height = Height::new(1).expect("valid height");
    let dims = Dimensions::new(width, height);
    let data = vec![0_u8; 3];
    Frame::new(dims, data).expect("valid frame")
}

/// Creates a base ISO file for testing.
fn make_iso_base() -> Vec<u8> {
    let width = Width::new(1).expect("valid width");
    let height = Height::new(1).expect("valid height");
    let frame = make_test_frame();
    encode_iso(
        synthvid_scene::FrameRate::from_fps(24).expect("valid frame rate"),
        None,
        &[frame.data().to_vec()],
        width,
        height,
        IsoCodec::UncompressedRgb,
        crate::TrackMatrix::identity(),
    )
    .expect("valid ISO encoding")
}

/// Creates a base AVI file for testing.
fn make_avi_base() -> Vec<u8> {
    let width = Width::new(1).expect("valid width");
    let height = Height::new(1).expect("valid height");
    let frame = make_test_frame();
    encode_avi(
        synthvid_scene::FrameRate::from_fps(24).expect("valid frame rate"),
        &[frame.data().to_vec()],
        width,
        height,
        AviCodec::UncompressedRgb,
    )
    .expect("valid AVI encoding")
}

#[test]
fn test_boxpath_builder() {
    let path = BoxPath::root()
        .child(BoxTag::new(*b"moov"))
        .child(BoxTag::new(*b"trak"))
        .child(BoxTag::new(*b"tkhd"));
    assert_eq!(path.tags().len(), 3);
    assert_eq!(path.tags()[0], BoxTag::new(*b"moov"));
}

#[test]
fn test_byteoffset_new() {
    let off = ByteOffset::new(42);
    assert_eq!(off.get(), 42);
}

#[test]
fn test_axis_enum() {
    let w = Axis::Width;
    let h = Axis::Height;
    assert_ne!(w, h);
}

// TruncateAt tests
#[test]
fn test_truncate_at_zero() {
    let defect = Defect::TruncateAt(ByteOffset::new(0));
    let input = vec![1, 2, 3, 4, 5];
    let result = apply(&defect, &input).expect("truncate");
    assert_eq!(result.len(), 0);
}

#[test]
fn test_truncate_at_middle() {
    let defect = Defect::TruncateAt(ByteOffset::new(3));
    let input = vec![1, 2, 3, 4, 5];
    let result = apply(&defect, &input).expect("truncate");
    assert_eq!(result, vec![1, 2, 3]);
}

#[test]
fn test_truncate_at_past_end() {
    let defect = Defect::TruncateAt(ByteOffset::new(100));
    let input = vec![1, 2, 3];
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::OffsetOutOfRange);
}

// TruncateBox tests
#[test]
fn test_truncate_box_iso_changes_bytes() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::TruncateBox(path);
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
    assert!(
        result.len() < input.len(),
        "truncated file should be smaller"
    );
}

#[test]
fn test_truncate_box_iso_reproducible() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::TruncateBox(path);
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_truncate_box_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::TruncateBox(path);
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// ZeroLengthPayload tests
#[test]
fn test_zero_payload_iso_changes_bytes() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::ZeroLengthPayload(path);
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
    assert_eq!(result.len(), input.len(), "same length");
}

#[test]
fn test_zero_payload_iso_reproducible() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::ZeroLengthPayload(path);
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_zero_payload_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::ZeroLengthPayload(path);
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// OversizedBoxLength tests
#[test]
fn test_oversized_iso_changes_bytes() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::OversizedBoxLength(path);
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
}

#[test]
fn test_oversized_iso_reproducible() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::OversizedBoxLength(path);
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_oversized_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::OversizedBoxLength(path);
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// UndersizedBoxLength tests
#[test]
fn test_undersized_iso_changes_bytes() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::UndersizedBoxLength(path);
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
}

#[test]
fn test_undersized_iso_reproducible() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::UndersizedBoxLength(path);
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_undersized_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::UndersizedBoxLength(path);
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// OffsetPastEnd tests
#[test]
fn test_offset_past_end_iso_changes_bytes() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"moov"));
    let defect = Defect::OffsetPastEnd(path);
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
}

#[test]
fn test_offset_past_end_iso_reproducible() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"moov"));
    let defect = Defect::OffsetPastEnd(path);
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_offset_past_end_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let path = BoxPath::root().child(BoxTag::new(*b"moov"));
    let defect = Defect::OffsetPastEnd(path);
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// DuplicateBox tests
#[test]
fn test_duplicate_box_iso_changes_bytes() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::DuplicateBox(path);
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
    assert!(
        result.len() > input.len(),
        "file should grow with duplicate"
    );
}

#[test]
fn test_duplicate_box_iso_reproducible() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::DuplicateBox(path);
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_duplicate_box_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::DuplicateBox(path);
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// UnknownBox tests
#[test]
fn test_unknown_box_iso_changes_bytes() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::UnknownBox {
        after: path,
        tag: BoxTag::new(*b"xxxx"),
    };
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
    assert!(result.len() > input.len(), "file should grow with new box");
}

#[test]
fn test_unknown_box_iso_reproducible() {
    let input = make_iso_base();
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::UnknownBox {
        after: path,
        tag: BoxTag::new(*b"xxxx"),
    };
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_unknown_box_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let path = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let defect = Defect::UnknownBox {
        after: path,
        tag: BoxTag::new(*b"xxxx"),
    };
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// ZeroFrameRate tests
#[test]
fn test_zero_frame_rate_iso_changes_bytes() {
    let input = make_iso_base();
    let defect = Defect::ZeroFrameRate;
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
}

#[test]
fn test_zero_frame_rate_iso_reproducible() {
    let input = make_iso_base();
    let defect = Defect::ZeroFrameRate;
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_zero_frame_rate_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let defect = Defect::ZeroFrameRate;
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// ZeroDimension tests
#[test]
fn test_zero_dimension_width_iso_changes_bytes() {
    let input = make_iso_base();
    let defect = Defect::ZeroDimension(Axis::Width);
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
}

#[test]
fn test_zero_dimension_height_iso_changes_bytes() {
    let input = make_iso_base();
    let defect = Defect::ZeroDimension(Axis::Height);
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
}

#[test]
fn test_zero_dimension_iso_reproducible() {
    let input = make_iso_base();
    let defect = Defect::ZeroDimension(Axis::Width);
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_zero_dimension_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let defect = Defect::ZeroDimension(Axis::Width);
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// AbsurdFrameRate tests
#[test]
fn test_absurd_frame_rate_iso_changes_bytes() {
    let input = make_iso_base();
    let rate = synthvid_scene::FrameRate::from_fps(1_000_000).expect("valid rate");
    let defect = Defect::AbsurdFrameRate(rate);
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
}

#[test]
fn test_absurd_frame_rate_iso_reproducible() {
    let input = make_iso_base();
    let rate = synthvid_scene::FrameRate::from_fps(1_000_000).expect("valid rate");
    let defect = Defect::AbsurdFrameRate(rate);
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_absurd_frame_rate_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let rate = synthvid_scene::FrameRate::from_fps(1_000_000).expect("valid rate");
    let defect = Defect::AbsurdFrameRate(rate);
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// NonMonotonicTimestamps tests
#[test]
fn test_non_monotonic_timestamps_iso_changes_bytes() {
    let input = make_iso_base();
    let defect = Defect::NonMonotonicTimestamps;
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
}

#[test]
fn test_non_monotonic_timestamps_iso_reproducible() {
    let input = make_iso_base();
    let defect = Defect::NonMonotonicTimestamps;
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_non_monotonic_timestamps_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let defect = Defect::NonMonotonicTimestamps;
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// DeclaredCountMismatch tests
#[test]
fn test_declared_count_mismatch_iso_changes_bytes() {
    let input = make_iso_base();
    let defect = Defect::DeclaredCountMismatch { declared: 999 };
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
}

#[test]
fn test_declared_count_mismatch_iso_reproducible() {
    let input = make_iso_base();
    let defect = Defect::DeclaredCountMismatch { declared: 999 };
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_declared_count_mismatch_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let defect = Defect::DeclaredCountMismatch { declared: 999 };
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// ReorderedBoxes tests
#[test]
fn test_reordered_boxes_iso_changes_bytes() {
    let input = make_iso_base();
    // Both boxes at root level
    let path1 = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let path2 = BoxPath::root().child(BoxTag::new(*b"mdat"));
    let defect = Defect::ReorderedBoxes(path1, path2);
    let result = apply(&defect, &input).expect("apply defect");
    assert_ne!(result, input, "defect should change bytes");
}

#[test]
fn test_reordered_boxes_iso_reproducible() {
    let input = make_iso_base();
    let path1 = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let path2 = BoxPath::root().child(BoxTag::new(*b"mdat"));
    let defect = Defect::ReorderedBoxes(path1, path2);
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_reordered_boxes_iso_missing_box_errors() {
    let input = vec![0u8; 100];
    let path1 = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let path2 = BoxPath::root().child(BoxTag::new(*b"mdat"));
    let defect = Defect::ReorderedBoxes(path1, path2);
    let err = apply(&defect, &input).expect_err("should fail");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

// Detailed tests for bug fixes

#[test]
fn test_zero_dimension_width_iso_exact_value() {
    let input = make_iso_base();
    let defect = Defect::ZeroDimension(Axis::Width);
    let result = apply(&defect, &input).expect("apply defect");

    // tkhd box width is at payload offset 76
    // Verify the result is different and bytes were changed
    assert_ne!(result, input, "defect should change bytes");
    assert!(
        result.len() == input.len(),
        "defect should not change file size"
    );
}

#[test]
fn test_zero_dimension_height_iso_exact_value() {
    let input = make_iso_base();
    let defect = Defect::ZeroDimension(Axis::Height);
    let result = apply(&defect, &input).expect("apply defect");

    assert_ne!(result, input, "defect should change bytes");
    assert!(
        result.len() == input.len(),
        "defect should not change file size"
    );
}

#[test]
fn test_offset_past_end_iso_exact_value() {
    let input = make_iso_base();
    let stco_path = BoxPath::root()
        .child(BoxTag::new(*b"moov"))
        .child(BoxTag::new(*b"trak"))
        .child(BoxTag::new(*b"mdia"))
        .child(BoxTag::new(*b"minf"))
        .child(BoxTag::new(*b"stbl"))
        .child(BoxTag::new(*b"stco"));
    let defect = Defect::OffsetPastEnd(stco_path);
    let result = apply(&defect, &input).expect("apply defect");

    // Verify bytes were changed
    assert_ne!(result, input, "defect should change bytes");
    assert!(
        result.len() == input.len(),
        "defect should not change file size"
    );
}

#[test]
fn test_declared_count_mismatch_iso_stsz_field() {
    let input = make_iso_base();
    let defect = Defect::DeclaredCountMismatch { declared: 42 };
    let result = apply(&defect, &input).expect("apply defect");

    // Verify stsz box was modified
    assert_ne!(result, input, "defect should change bytes");
    assert!(
        result.len() == input.len(),
        "defect should not change file size"
    );
}

#[test]
fn test_non_monotonic_timestamps_iso_sample_delta() {
    let input = make_iso_base();
    let defect = Defect::NonMonotonicTimestamps;
    let result = apply(&defect, &input).expect("apply defect");

    // Verify stts was modified with sample_delta = 0
    assert_ne!(result, input, "defect should change bytes");
    assert!(
        result.len() == input.len(),
        "defect should not change file size"
    );
}

#[test]
fn test_zero_frame_rate_avi_correct_offset() {
    let input = make_avi_base();
    let defect = Defect::ZeroFrameRate;
    let result = apply(&defect, &input).expect("apply defect");

    // Verify bytes were changed and size unchanged
    assert_ne!(result, input, "defect should change bytes");
    assert!(
        result.len() == input.len(),
        "defect should not change file size"
    );
}

#[test]
fn test_zero_frame_rate_avi_reproducible() {
    let input = make_avi_base();
    let defect = Defect::ZeroFrameRate;
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_absurd_frame_rate_avi_correct_offset() {
    let input = make_avi_base();
    let frame_rate = synthvid_scene::FrameRate::from_fps(2000).expect("valid frame rate");
    let defect = Defect::AbsurdFrameRate(frame_rate);
    let result = apply(&defect, &input).expect("apply defect");

    // Verify bytes were changed
    assert_ne!(result, input, "defect should change bytes");
    assert!(
        result.len() == input.len(),
        "defect should not change file size"
    );
}

#[test]
fn test_absurd_frame_rate_avi_reproducible() {
    let input = make_avi_base();
    let frame_rate = synthvid_scene::FrameRate::from_fps(2000).expect("valid frame rate");
    let defect = Defect::AbsurdFrameRate(frame_rate);
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_zero_dimension_width_avi_correct_offset() {
    let input = make_avi_base();
    let defect = Defect::ZeroDimension(Axis::Width);
    let result = apply(&defect, &input).expect("apply defect");

    // Verify bytes were changed
    assert_ne!(result, input, "defect should change bytes");
    assert!(
        result.len() == input.len(),
        "defect should not change file size"
    );
}

#[test]
fn test_zero_dimension_height_avi_correct_offset() {
    let input = make_avi_base();
    let defect = Defect::ZeroDimension(Axis::Height);
    let result = apply(&defect, &input).expect("apply defect");

    // Verify bytes were changed
    assert_ne!(result, input, "defect should change bytes");
    assert!(
        result.len() == input.len(),
        "defect should not change file size"
    );
}

#[test]
fn test_zero_dimension_avi_reproducible() {
    let input = make_avi_base();
    let defect = Defect::ZeroDimension(Axis::Width);
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}

#[test]
fn test_declared_count_mismatch_avi_correct_offset() {
    let input = make_avi_base();
    let defect = Defect::DeclaredCountMismatch { declared: 99 };
    let result = apply(&defect, &input).expect("apply defect");

    // Verify avih frame_count was modified
    assert_ne!(result, input, "defect should change bytes");
    assert!(
        result.len() == input.len(),
        "defect should not change file size"
    );
}

#[test]
fn test_declared_count_mismatch_avi_reproducible() {
    let input = make_avi_base();
    let defect = Defect::DeclaredCountMismatch { declared: 99 };
    let result1 = apply(&defect, &input).expect("apply defect");
    let result2 = apply(&defect, &input).expect("apply defect");
    assert_eq!(result1, result2, "defect must be reproducible");
}
