//! Box reordering implementation for defects.

use super::{defects_box, BoxPath, DefectError};
use defects_box::{
    find_avi_box, find_iso_box, is_avi_format, is_iso_format, BoxLen, BoxStart, ContainerBytes,
};

/// Extracted box information for manipulation.
pub(super) struct ExtractedBox {
    /// Start byte offset in file
    pub(super) start: BoxStart,
    /// Length in bytes
    pub(super) len: BoxLen,
    /// Box bytes
    pub(super) bytes: Vec<u8>,
}

/// Check that two paths are immediate siblings (all segments except the last are identical).
fn are_immediate_siblings(path1: &BoxPath, path2: &BoxPath) -> bool {
    let tags1 = path1.tags();
    let tags2 = path2.tags();

    // Both paths must have at least one element (the child under root or another parent)
    if tags1.is_empty() || tags2.is_empty() {
        return false;
    }

    // They must have the same depth
    if tags1.len() != tags2.len() {
        return false;
    }

    // All elements except the last must be identical
    // Split returns (last_elem, rest_slice), so we compare rest slices and last elements
    match (tags1.split_last(), tags2.split_last()) {
        (Some((last1, prefix1)), Some((last2, prefix2))) => {
            // Parents must match
            if prefix1 != prefix2 {
                return false;
            }
            // The boxes being reordered must be different
            last1 != last2
        }
        _ => false,
    }
}

/// Apply `ReorderedBoxes` defect
pub(super) fn apply_reordered_boxes(
    path1: &BoxPath,
    path2: &BoxPath,
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    // Validate that paths refer to immediate siblings
    if !are_immediate_siblings(path1, path2) {
        return Err(DefectError::InvalidBoxStructure);
    }

    if is_iso_format(input)? {
        let (box1, box2) = extract_box_pairs_iso(input, path1, path2)?;
        reorder_boxes_impl(input.as_slice(), &box1, &box2)
    } else if is_avi_format(input) {
        let (box1, box2) = extract_box_pairs_avi(input, path1, path2)?;
        reorder_boxes_impl(input.as_slice(), &box1, &box2)
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}

/// Helper to perform box reordering given two extracted boxes.
fn reorder_boxes_impl(
    input: &[u8],
    box1: &ExtractedBox,
    box2: &ExtractedBox,
) -> Result<Vec<u8>, DefectError> {
    let mut result = input.to_vec();
    if box1.start < box2.start {
        // buffer indexing requires a raw usize; convert newtypes at the boundary
        let middle_start = box1
            .start
            .get()
            .checked_add(box1.len.get())
            .ok_or(DefectError::SizeOverflow)?;
        let middle_end = box2.start.get();
        let middle_bytes = input
            .get(middle_start..middle_end)
            .ok_or(DefectError::SizeOverflow)?;
        result.splice(
            box1.start.get()
                ..box2
                    .start
                    .get()
                    .checked_add(box2.len.get())
                    .ok_or(DefectError::SizeOverflow)?,
            box2.bytes
                .iter()
                .chain(middle_bytes.iter())
                .chain(box1.bytes.iter())
                .copied(),
        );
    } else {
        // buffer indexing requires a raw usize; convert newtypes at the boundary
        let middle_start = box2
            .start
            .get()
            .checked_add(box2.len.get())
            .ok_or(DefectError::SizeOverflow)?;
        let middle_end = box1.start.get();
        let middle_bytes = input
            .get(middle_start..middle_end)
            .ok_or(DefectError::SizeOverflow)?;
        result.splice(
            box2.start.get()
                ..box1
                    .start
                    .get()
                    .checked_add(box1.len.get())
                    .ok_or(DefectError::SizeOverflow)?,
            box1.bytes
                .iter()
                .chain(middle_bytes.iter())
                .chain(box2.bytes.iter())
                .copied(),
        );
    }
    Ok(result)
}

/// Helper to extract two ISO boxes for reordering
fn extract_box_pairs_iso(
    input: ContainerBytes<'_>,
    path1: &BoxPath,
    path2: &BoxPath,
) -> Result<(ExtractedBox, ExtractedBox), DefectError> {
    let box1 = find_iso_box(input, path1)?;
    let box2 = find_iso_box(input, path2)?;
    let b1_start = box1.box_start;
    let b1_start_usize = b1_start.get();
    let b1_len_usize = box1
        .payload_end
        .get()
        .checked_sub(b1_start_usize)
        .ok_or(DefectError::SizeOverflow)?;
    let b2_start = box2.box_start;
    let b2_start_usize = b2_start.get();
    let b2_len_usize = box2
        .payload_end
        .get()
        .checked_sub(b2_start_usize)
        .ok_or(DefectError::SizeOverflow)?;
    // buffer indexing requires a raw usize; this is the single point where
    // the offset re-enters raw-usize space
    let b1_bytes = input
        .as_slice()
        .get(
            b1_start_usize
                ..b1_start_usize
                    .checked_add(b1_len_usize)
                    .ok_or(DefectError::SizeOverflow)?,
        )
        .ok_or(DefectError::SizeOverflow)?
        .to_vec();
    let b2_bytes = input
        .as_slice()
        .get(
            b2_start_usize
                ..b2_start_usize
                    .checked_add(b2_len_usize)
                    .ok_or(DefectError::SizeOverflow)?,
        )
        .ok_or(DefectError::SizeOverflow)?
        .to_vec();
    Ok((
        ExtractedBox {
            start: b1_start,
            len: BoxLen::new(b1_len_usize),
            bytes: b1_bytes,
        },
        ExtractedBox {
            start: b2_start,
            len: BoxLen::new(b2_len_usize),
            bytes: b2_bytes,
        },
    ))
}

/// Helper to extract two AVI boxes for reordering
fn extract_box_pairs_avi(
    input: ContainerBytes<'_>,
    path1: &BoxPath,
    path2: &BoxPath,
) -> Result<(ExtractedBox, ExtractedBox), DefectError> {
    let box1 = find_avi_box(input, path1)?;
    let box2 = find_avi_box(input, path2)?;
    let b1_start = box1.box_start;
    let b1_start_usize = b1_start.get();
    let b1_len_usize = box1
        .payload_end
        .get()
        .checked_sub(b1_start_usize)
        .ok_or(DefectError::SizeOverflow)?;
    let b2_start = box2.box_start;
    let b2_start_usize = b2_start.get();
    let b2_len_usize = box2
        .payload_end
        .get()
        .checked_sub(b2_start_usize)
        .ok_or(DefectError::SizeOverflow)?;
    // buffer indexing requires a raw usize; this is the single point where
    // the offset re-enters raw-usize space
    let b1_bytes = input
        .as_slice()
        .get(
            b1_start_usize
                ..b1_start_usize
                    .checked_add(b1_len_usize)
                    .ok_or(DefectError::SizeOverflow)?,
        )
        .ok_or(DefectError::SizeOverflow)?
        .to_vec();
    let b2_bytes = input
        .as_slice()
        .get(
            b2_start_usize
                ..b2_start_usize
                    .checked_add(b2_len_usize)
                    .ok_or(DefectError::SizeOverflow)?,
        )
        .ok_or(DefectError::SizeOverflow)?
        .to_vec();
    Ok((
        ExtractedBox {
            start: b1_start,
            len: BoxLen::new(b1_len_usize),
            bytes: b1_bytes,
        },
        ExtractedBox {
            start: b2_start,
            len: BoxLen::new(b2_len_usize),
            bytes: b2_bytes,
        },
    ))
}
