//! Box reordering implementation for defects.

use super::defects_box::{
    detect_format, find_avi_box, find_iso_box, BoxInfo, BoxLen, BoxStart, ContainerBytes,
};
use super::defects_types::{BoxPair, ContainerFormat, FileBytes, PathPair};
use super::DefectError;

/// Extracted box information for manipulation.
pub(super) struct ExtractedBox {
    /// Start byte offset in file.
    pub(super) start: BoxStart,
    /// Length in bytes.
    pub(super) len: BoxLen,
    /// Box bytes.
    pub(super) bytes: FileBytes,
}

/// Check that two paths are immediate siblings (all segments except the last are identical).
fn check_immediate_siblings(paths: PathPair<'_>) -> Result<(), DefectError> {
    let tags1 = paths.first.tags();
    let tags2 = paths.second.tags();

    // Both paths must have at least one element (the child under root or another parent)
    if tags1.is_empty() || tags2.is_empty() {
        return Err(DefectError::InvalidBoxStructure);
    }

    // They must have the same depth
    if tags1.len() != tags2.len() {
        return Err(DefectError::InvalidBoxStructure);
    }

    // All elements except the last must be identical
    // Split returns (last_elem, rest_slice), so we compare rest slices and last elements
    match (tags1.split_last(), tags2.split_last()) {
        (Some((last1, prefix1)), Some((last2, prefix2))) => {
            // Parents must match, and the boxes being reordered must be different
            if prefix1 != prefix2 || last1 == last2 {
                Err(DefectError::InvalidBoxStructure)
            } else {
                Ok(())
            }
        }
        _ => Err(DefectError::InvalidBoxStructure),
    }
}

/// Apply `ReorderedBoxes` defect.
pub(super) fn apply_reordered_boxes(
    paths: PathPair<'_>,
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    // Validate that paths refer to immediate siblings
    check_immediate_siblings(paths)?;

    match detect_format(input)? {
        ContainerFormat::Iso => {
            let pair = extract_box_pairs_iso(input, paths)?;
            reorder_boxes_impl(input, &pair)
        }
        ContainerFormat::Avi => {
            let pair = extract_box_pairs_avi(input, paths)?;
            reorder_boxes_impl(input, &pair)
        }
        ContainerFormat::Neither => Err(DefectError::IncompatibleFileType),
    }
}

/// Helper to perform box reordering given two extracted boxes.
fn reorder_boxes_impl(input: ContainerBytes<'_>, pair: &BoxPair) -> Result<FileBytes, DefectError> {
    let raw = input.as_slice();
    // Whichever box comes first in the file trades places with the other.
    let (earlier, later) = if pair.first.start < pair.second.start {
        (&pair.first, &pair.second)
    } else {
        (&pair.second, &pair.first)
    };
    // buffer indexing requires a raw usize; convert newtypes at the boundary
    let middle_start = earlier
        .start
        .get()
        .checked_add(earlier.len.get())
        .ok_or(DefectError::SizeOverflow)?;
    let later_end = later
        .start
        .get()
        .checked_add(later.len.get())
        .ok_or(DefectError::SizeOverflow)?;
    let middle_bytes = raw
        .get(middle_start..later.start.get())
        .ok_or(DefectError::SizeOverflow)?;
    let head = raw
        .get(..earlier.start.get())
        .ok_or(DefectError::SizeOverflow)?;
    let tail = raw.get(later_end..).ok_or(DefectError::SizeOverflow)?;
    Ok(FileBytes(
        [
            head,
            later.bytes.0.as_slice(),
            middle_bytes,
            earlier.bytes.0.as_slice(),
            tail,
        ]
        .concat(),
    ))
}

/// Copies one box out of the container, from its start to its payload's end.
fn extract_box(input: ContainerBytes<'_>, info: &BoxInfo) -> Result<ExtractedBox, DefectError> {
    let start = info.box_start;
    let len = info
        .payload_end
        .get()
        .checked_sub(start.get())
        .ok_or(DefectError::SizeOverflow)?;
    // buffer indexing requires a raw usize; this is the single point where
    // the offset re-enters raw-usize space
    let end = start
        .get()
        .checked_add(len)
        .ok_or(DefectError::SizeOverflow)?;
    let bytes = input
        .as_slice()
        .get(start.get()..end)
        .ok_or(DefectError::SizeOverflow)?
        .to_vec();
    Ok(ExtractedBox {
        start,
        len: BoxLen::new(len),
        bytes: FileBytes(bytes),
    })
}

/// Helper to extract two ISO boxes for reordering.
fn extract_box_pairs_iso(
    input: ContainerBytes<'_>,
    paths: PathPair<'_>,
) -> Result<BoxPair, DefectError> {
    let box1 = find_iso_box(input, paths.first)?;
    let box2 = find_iso_box(input, paths.second)?;
    Ok(BoxPair {
        first: extract_box(input, &box1)?,
        second: extract_box(input, &box2)?,
    })
}

/// Helper to extract two AVI boxes for reordering.
fn extract_box_pairs_avi(
    input: ContainerBytes<'_>,
    paths: PathPair<'_>,
) -> Result<BoxPair, DefectError> {
    let box1 = find_avi_box(input, paths.first)?;
    let box2 = find_avi_box(input, paths.second)?;
    Ok(BoxPair {
        first: extract_box(input, &box1)?,
        second: extract_box(input, &box2)?,
    })
}
