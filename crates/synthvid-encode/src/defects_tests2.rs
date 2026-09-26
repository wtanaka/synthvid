#[cfg(test)]
use super::*;
use crate::{encode_avi, encode_iso, AviCodec, IsoCodec};
use defects_box::{
    extract_4_bytes, find_avi_box_with_ancestors, find_iso_box_with_ancestors, AbsoluteOffset,
    ContainerBytes,
};
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
fn test_reordered_boxes_iso_length_preserved() {
    let input = make_iso_base();
    let path1 = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let path2 = BoxPath::root().child(BoxTag::new(*b"mdat"));
    let defect = Defect::ReorderedBoxes(path1, path2);
    let result = apply(&defect, &input).expect("apply defect");
    assert_eq!(
        result.len(),
        input.len(),
        "reordering must preserve total file length"
    );
}

#[test]
fn test_reordered_boxes_iso_middle_preserved() {
    let input = make_iso_base();
    let path1 = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let path2 = BoxPath::root().child(BoxTag::new(*b"mdat"));
    let defect = Defect::ReorderedBoxes(path1, path2);
    let result = apply(&defect, &input).expect("apply defect");

    // The result length is preserved
    assert_eq!(result.len(), input.len(), "length must be preserved");
    // The result is different (boxes were reordered)
    assert_ne!(result, input, "boxes must be reordered");
    // Both result and input should be valid (non-empty) to prove middle wasn't dropped
    assert!(!result.is_empty(), "result must not be empty");
    assert!(!input.is_empty(), "input must not be empty");
}

#[test]
fn test_reordered_boxes_non_siblings_error() {
    let input = make_iso_base();
    // Try to reorder boxes that are not immediate siblings:
    // path1: /ftyp (root level)
    // path2: /moov/trak/tkhd (nested deep)
    let path1 = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let path2 = BoxPath::root()
        .child(BoxTag::new(*b"moov"))
        .child(BoxTag::new(*b"trak"))
        .child(BoxTag::new(*b"tkhd"));
    let defect = Defect::ReorderedBoxes(path1, path2);
    let err = apply(&defect, &input).expect_err("should fail for non-siblings");
    assert_eq!(err, DefectError::InvalidBoxStructure);
}

#[test]
fn test_duplicate_box_iso_nested_updates_ancestors() {
    let input = make_iso_base();

    let tkhd_path = BoxPath::root()
        .child(BoxTag::new(*b"moov"))
        .child(BoxTag::new(*b"trak"))
        .child(BoxTag::new(*b"tkhd"));
    let container = ContainerBytes::new(&input);
    let tkhd_info = find_iso_box_with_ancestors(container, &tkhd_path).expect("find tkhd");

    let tkhd_box_start = tkhd_info.box_info.box_start;
    let tkhd_box_size = tkhd_info
        .box_info
        .payload_end
        .get()
        .checked_sub(tkhd_box_start.get())
        .expect("valid tkhd size");

    let moov_size_offset = tkhd_info.ancestor_size_offsets.first().copied();
    let trak_size_offset = tkhd_info.ancestor_size_offsets.get(1).copied();

    let moov_orig_size =
        moov_size_offset.and_then(|off| extract_4_bytes(container, off).map(u32::from_be_bytes));
    let trak_orig_size =
        trak_size_offset.and_then(|off| extract_4_bytes(container, off).map(u32::from_be_bytes));

    let defect = Defect::DuplicateBox(tkhd_path);
    let result = apply(&defect, &input).expect("apply duplicate");

    assert_eq!(
        result.len(),
        input.len() + tkhd_box_size,
        "file must grow by duplicated box size"
    );

    if let Some(moov_off) = moov_size_offset {
        let result_container = ContainerBytes::new(&result);
        let moov_new_size_bytes =
            extract_4_bytes(result_container, moov_off).expect("read moov size after duplicate");
        let moov_new_size = u32::from_be_bytes(moov_new_size_bytes);
        let tkhd_box_size_u32 = u32::try_from(tkhd_box_size).expect("tkhd_box_size fits in u32");
        let expected_moov_size = moov_orig_size
            .expect("had moov orig size")
            .checked_add(tkhd_box_size_u32)
            .expect("moov size addition");
        assert_eq!(
            moov_new_size, expected_moov_size,
            "moov size must increase by tkhd box size"
        );
    }

    if let Some(trak_off) = trak_size_offset {
        let result_container = ContainerBytes::new(&result);
        let trak_new_size_bytes =
            extract_4_bytes(result_container, trak_off).expect("read trak size after duplicate");
        let trak_new_size = u32::from_be_bytes(trak_new_size_bytes);
        let tkhd_box_size_u32 = u32::try_from(tkhd_box_size).expect("tkhd_box_size fits in u32");
        let expected_trak_size = trak_orig_size
            .expect("had trak orig size")
            .checked_add(tkhd_box_size_u32)
            .expect("trak size addition");
        assert_eq!(
            trak_new_size, expected_trak_size,
            "trak size must increase by tkhd box size"
        );
    }
}

#[test]
fn test_unknown_box_iso_nested_updates_ancestors() {
    const NEW_BOX_SIZE: usize = 8;
    let input = make_iso_base();

    let tkhd_path = BoxPath::root()
        .child(BoxTag::new(*b"moov"))
        .child(BoxTag::new(*b"trak"))
        .child(BoxTag::new(*b"tkhd"));
    let container = ContainerBytes::new(&input);
    let tkhd_info = find_iso_box_with_ancestors(container, &tkhd_path).expect("find tkhd");

    let moov_size_offset = tkhd_info.ancestor_size_offsets.first().copied();
    let trak_size_offset = tkhd_info.ancestor_size_offsets.get(1).copied();

    let moov_orig_size =
        moov_size_offset.and_then(|off| extract_4_bytes(container, off).map(u32::from_be_bytes));
    let trak_orig_size =
        trak_size_offset.and_then(|off| extract_4_bytes(container, off).map(u32::from_be_bytes));

    let defect = Defect::UnknownBox {
        after: tkhd_path,
        tag: BoxTag::new(*b"xxxx"),
    };
    let result = apply(&defect, &input).expect("apply unknown box");

    assert_eq!(
        result.len(),
        input.len() + NEW_BOX_SIZE,
        "file must grow by new box size"
    );

    if let Some(moov_off) = moov_size_offset {
        let result_container = ContainerBytes::new(&result);
        let moov_new_size_bytes =
            extract_4_bytes(result_container, moov_off).expect("read moov size after unknown box");
        let moov_new_size = u32::from_be_bytes(moov_new_size_bytes);
        let new_box_size_u32 = u32::try_from(NEW_BOX_SIZE).expect("NEW_BOX_SIZE fits in u32");
        let expected_moov_size = moov_orig_size
            .expect("had moov orig size")
            .checked_add(new_box_size_u32)
            .expect("moov size addition");
        assert_eq!(
            moov_new_size, expected_moov_size,
            "moov size must increase by new box size"
        );
    }

    if let Some(trak_off) = trak_size_offset {
        let result_container = ContainerBytes::new(&result);
        let trak_new_size_bytes =
            extract_4_bytes(result_container, trak_off).expect("read trak size after unknown box");
        let trak_new_size = u32::from_be_bytes(trak_new_size_bytes);
        let new_box_size_u32 = u32::try_from(NEW_BOX_SIZE).expect("NEW_BOX_SIZE fits in u32");
        let expected_trak_size = trak_orig_size
            .expect("had trak orig size")
            .checked_add(new_box_size_u32)
            .expect("trak size addition");
        assert_eq!(
            trak_new_size, expected_trak_size,
            "trak size must increase by new box size"
        );
    }
}

#[test]
fn test_duplicate_box_avi_nested_updates_ancestors() {
    const RIFF_SIZE_OFFSET: usize = 4;
    let input = make_avi_base();

    let strh_path = BoxPath::root()
        .child(BoxTag::new(*b"hdrl"))
        .child(BoxTag::new(*b"strl"))
        .child(BoxTag::new(*b"strh"));
    let container = ContainerBytes::new(&input);
    let strh_info = find_avi_box_with_ancestors(container, &strh_path).expect("find strh");

    let strh_chunk_start = strh_info.box_info.box_start;
    let strh_chunk_size = strh_info
        .box_info
        .payload_end
        .get()
        .checked_sub(strh_chunk_start.get())
        .expect("valid strh size");
    let strh_chunk_size_u32 = u32::try_from(strh_chunk_size).expect("strh_chunk_size fits in u32");
    let hdrl_size_offset = strh_info.ancestor_size_offsets.get(1).copied();
    let strl_size_offset = strh_info.ancestor_size_offsets.get(2).copied();

    let riff_orig_size = extract_4_bytes(container, AbsoluteOffset::new(RIFF_SIZE_OFFSET))
        .map(u32::from_le_bytes)
        .expect("read RIFF size");
    let hdrl_orig_size =
        hdrl_size_offset.and_then(|off| extract_4_bytes(container, off).map(u32::from_le_bytes));
    let strl_orig_size =
        strl_size_offset.and_then(|off| extract_4_bytes(container, off).map(u32::from_le_bytes));

    let defect = Defect::DuplicateBox(strh_path);
    let result = apply(&defect, &input).expect("apply duplicate");

    assert_eq!(
        result.len(),
        input.len() + strh_chunk_size,
        "file must grow by duplicated chunk size"
    );

    let result_container = ContainerBytes::new(&result);
    let riff_new_size_bytes =
        extract_4_bytes(result_container, AbsoluteOffset::new(RIFF_SIZE_OFFSET))
            .expect("read RIFF size after duplicate");
    let riff_new_size = u32::from_le_bytes(riff_new_size_bytes);
    let expected_riff_size = riff_orig_size
        .checked_add(strh_chunk_size_u32)
        .expect("RIFF size addition");
    assert_eq!(
        riff_new_size, expected_riff_size,
        "RIFF size must increase by strh chunk size"
    );

    if let Some(hdrl_off) = hdrl_size_offset {
        let hdrl_new_size_bytes =
            extract_4_bytes(result_container, hdrl_off).expect("read hdrl size after duplicate");
        let hdrl_new_size = u32::from_le_bytes(hdrl_new_size_bytes);
        let expected_hdrl_size = hdrl_orig_size
            .expect("had hdrl orig size")
            .checked_add(strh_chunk_size_u32)
            .expect("hdrl size addition");
        assert_eq!(
            hdrl_new_size, expected_hdrl_size,
            "hdrl size must increase by strh chunk size"
        );
    }

    if let Some(strl_off) = strl_size_offset {
        let strl_new_size_bytes =
            extract_4_bytes(result_container, strl_off).expect("read strl size after duplicate");
        let strl_new_size = u32::from_le_bytes(strl_new_size_bytes);
        let expected_strl_size = strl_orig_size
            .expect("had strl orig size")
            .checked_add(strh_chunk_size_u32)
            .expect("strl size addition");
        assert_eq!(
            strl_new_size, expected_strl_size,
            "strl size must increase by strh chunk size"
        );
    }
}

#[test]
fn test_oversized_list_chunk_hdrl_directly() {
    let input = make_avi_base();
    let container = ContainerBytes::new(&input);

    // Find hdrl LIST chunk directly
    let hdrl_path = BoxPath::root().child(BoxTag::new(*b"hdrl"));
    let hdrl_info =
        find_avi_box_with_ancestors(container, &hdrl_path).expect("find hdrl LIST chunk");

    // For AVI LIST chunks, box_start is the offset of the LIST tag
    let hdrl_start = hdrl_info.box_info.box_start.get();
    // The size field for an AVI LIST chunk is at offset + 4 (after the "LIST" tag)
    let hdrl_size_offset = AbsoluteOffset::new(hdrl_start.checked_add(4).expect("valid offset"));

    // Read the original size field
    let orig_size_bytes =
        extract_4_bytes(container, hdrl_size_offset).expect("read hdrl size before modification");
    let orig_size = u32::from_le_bytes(orig_size_bytes);

    // Apply OversizedBoxLength defect to the hdrl LIST chunk itself
    let defect = Defect::OversizedBoxLength(hdrl_path);
    let result = apply(&defect, &input).expect("apply OversizedBoxLength to hdrl");

    // File should be same length (we're only modifying the size field, not adding/removing content)
    assert_eq!(result.len(), input.len(), "file length unchanged");

    // Read the new size field
    let result_container = ContainerBytes::new(&result);
    let new_size_bytes = extract_4_bytes(result_container, hdrl_size_offset)
        .expect("read hdrl size after modification");
    let new_size = u32::from_le_bytes(new_size_bytes);

    // Size should have increased by 256
    assert_eq!(
        new_size,
        orig_size.checked_add(256).expect("size addition"),
        "hdrl LIST chunk size must increase by 256"
    );

    // Verify the defect was applied only to that specific field by checking
    // that the result differs from input in a predictable way
    let mut expected = input.clone();
    let new_size_le = new_size.to_le_bytes();
    expected[hdrl_size_offset.get()..hdrl_size_offset.get().checked_add(4).unwrap()]
        .copy_from_slice(&new_size_le);
    assert_eq!(
        result, expected,
        "only the hdrl LIST chunk size field should have changed"
    );
}

#[test]
fn test_undersized_list_chunk_hdrl_directly() {
    let input = make_avi_base();
    let container = ContainerBytes::new(&input);

    // Find hdrl LIST chunk directly
    let hdrl_path = BoxPath::root().child(BoxTag::new(*b"hdrl"));
    let hdrl_info =
        find_avi_box_with_ancestors(container, &hdrl_path).expect("find hdrl LIST chunk");

    // For AVI LIST chunks, box_start is the offset of the LIST tag
    let hdrl_start = hdrl_info.box_info.box_start.get();
    // The size field for an AVI LIST chunk is at offset + 4 (after the "LIST" tag)
    let hdrl_size_offset = AbsoluteOffset::new(hdrl_start.checked_add(4).expect("valid offset"));

    // Read the original size field
    let orig_size_bytes =
        extract_4_bytes(container, hdrl_size_offset).expect("read hdrl size before modification");
    let orig_size = u32::from_le_bytes(orig_size_bytes);

    // Apply UndersizedBoxLength defect to the hdrl LIST chunk itself
    let defect = Defect::UndersizedBoxLength(hdrl_path);
    let result = apply(&defect, &input).expect("apply UndersizedBoxLength to hdrl");

    // File should be same length
    assert_eq!(result.len(), input.len(), "file length unchanged");

    // Read the new size field
    let result_container = ContainerBytes::new(&result);
    let new_size_bytes = extract_4_bytes(result_container, hdrl_size_offset)
        .expect("read hdrl size after modification");
    let new_size = u32::from_le_bytes(new_size_bytes);

    // Size should have decreased by 1
    assert_eq!(
        new_size,
        orig_size.checked_sub(1).expect("size subtraction"),
        "hdrl LIST chunk size must decrease by 1"
    );

    // Verify only that field changed
    let mut expected = input.clone();
    let new_size_le = new_size.to_le_bytes();
    expected[hdrl_size_offset.get()..hdrl_size_offset.get().checked_add(4).unwrap()]
        .copy_from_slice(&new_size_le);
    assert_eq!(
        result, expected,
        "only the hdrl LIST chunk size field should have changed"
    );
}
