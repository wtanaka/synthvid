//! Addresses the frames of an ISO file.
//!
//! A frame in ISO is not a box: the frames sit back to back in `mdat`, and the
//! sample tables under `moov` say where each begins and how long it is. This
//! lists them as regions below the `mdat` that holds them, so a frame is named
//! by its number, as it is in AVI.
//!
//! The encoder writes one sample per chunk, so the first video track's `stsz`
//! gives each frame's length and its `stco` gives each frame's offset.

use super::defects_box::{extract_4_bytes, find_iso_box, AbsoluteOffset, BoxInfo, ContainerBytes};
use super::defects_layout::{BoxAddress, BoxStep, ContainerKind, Region};
use super::defects_types::{FieldOffset, Occurrence};
use super::{BoxPath, BoxTag, DefectError};
use core::iter::once;
use synthvid_scene::FrameIndex;

/// The byte range of one frame.
struct FrameRange {
    /// Offset of the frame's first byte.
    start: AbsoluteOffset,
    /// Offset one past the frame's last byte.
    end: AbsoluteOffset,
}

/// The path to a box of the first track's sample table.
fn sample_table_box(tag: BoxTag) -> BoxPath {
    [*b"moov", *b"trak", *b"mdia", *b"minf", *b"stbl"]
        .into_iter()
        .fold(BoxPath::root(), |path, step| path.child(BoxTag::new(step)))
        .child(tag)
}

/// A big-endian `u32` read from a sample table: a count, a size, or an
/// offset.
#[derive(Debug, Clone, Copy)]
struct TableValue(usize);

/// Reads the big-endian `u32` at `at`.
fn read_u32(input: ContainerBytes<'_>, at: AbsoluteOffset) -> Result<TableValue, DefectError> {
    let bytes = extract_4_bytes(input, at).ok_or(DefectError::InvalidBoxStructure)?;
    usize::try_from(u32::from_be_bytes(bytes.0))
        .map(TableValue)
        .map_err(DefectError::size_overflow)
}

/// A sample-table box lookup in which a missing box is `Ok(None)` rather
/// than an error.
fn optional_box(found: Result<BoxInfo, DefectError>) -> Result<Option<BoxInfo>, DefectError> {
    match found {
        Ok(info) => Ok(Some(info)),
        Err(DefectError::BoxNotFound(_path)) => Ok(None),
        Err(
            error @ (DefectError::AddressNotFound
            | DefectError::OffsetOutOfRange
            | DefectError::InvalidBoxStructure
            | DefectError::IncompatibleFileType
            | DefectError::SizeOverflow),
        ) => Err(error),
    }
}

/// Reads the frames the sample tables list; a file with no sample table has no
/// frames.
fn frame_ranges(input: ContainerBytes<'_>) -> Result<Vec<FrameRange>, DefectError> {
    let stsz = optional_box(find_iso_box(
        input,
        &sample_table_box(BoxTag::new(*b"stsz")),
    ));
    let stco = optional_box(find_iso_box(
        input,
        &sample_table_box(BoxTag::new(*b"stco")),
    ));
    // Either table missing means no frames, whatever the other lookup found.
    if matches!(stsz, Ok(None)) || matches!(stco, Ok(None)) {
        return Ok(Vec::new());
    }
    let (Some(stsz_info), Some(stco_info)) = (stsz?, stco?) else {
        return Ok(Vec::new());
    };
    // Both payloads start with a version and flags word, then the entry
    // counts; `stsz` has a constant sample size before its count.
    let TableValue(constant_size) =
        read_u32(input, stsz_info.payload_start.field_at(FieldOffset(4))?)?;
    let TableValue(sample_count) =
        read_u32(input, stsz_info.payload_start.field_at(FieldOffset(8))?)?;
    let TableValue(chunk_count) =
        read_u32(input, stco_info.payload_start.field_at(FieldOffset(4))?)?;
    if sample_count != chunk_count {
        return Err(DefectError::InvalidBoxStructure);
    }
    let mut ranges = Vec::new();
    for index in 0..sample_count {
        let entry = index.checked_mul(4).ok_or(DefectError::SizeOverflow)?;
        let len = if constant_size == 0 {
            let at = entry.checked_add(12).ok_or(DefectError::SizeOverflow)?;
            read_u32(input, stsz_info.payload_start.field_at(FieldOffset(at))?)?.0
        } else {
            constant_size
        };
        let at = entry.checked_add(8).ok_or(DefectError::SizeOverflow)?;
        let TableValue(start) =
            read_u32(input, stco_info.payload_start.field_at(FieldOffset(at))?)?;
        let end = start.checked_add(len).ok_or(DefectError::SizeOverflow)?;
        ranges.push(FrameRange {
            start: AbsoluteOffset::new(start),
            end: AbsoluteOffset::new(end),
        });
    }
    Ok(ranges)
}

/// Adds a region for each frame, directly after the `mdat` that holds it.
///
/// `regions` is the box walk of the file, in document order. Frames are
/// addressed below their `mdat`, which then owns only the bytes before the
/// first frame.
///
/// # Errors
///
/// Returns an error when the sample tables disagree, or list a frame outside
/// every `mdat` or out of order.
pub(super) fn add_iso_frames(
    input: ContainerBytes<'_>,
    regions: &mut Vec<Region>,
) -> Result<(), DefectError> {
    let ranges = frame_ranges(input)?;
    let Some(first) = ranges.first() else {
        return Ok(());
    };
    let first_start = first.start;
    let file_end = AbsoluteOffset::new(input.as_slice().len());
    let mdat = BoxTag::new(*b"mdat");
    // The first `mdat` that holds the first frame; the walk lists each top-level
    // `mdat` under its own occurrence number.
    let (holder_address, position, end) = (0..)
        .map_while(|occurrence| {
            let address = BoxAddress::top(
                ContainerKind::Iso,
                BoxStep::new(mdat, Occurrence(occurrence)),
            );
            regions
                .iter()
                .position(|region| region.address == address)
                .map(|position| (address, position))
        })
        .find_map(|(address, position)| {
            // Each region ends where the next begins; the last ends with the file.
            let next_starts = regions
                .iter()
                .skip(1)
                .map(|region| region.start)
                .chain(once(file_end));
            let (region, end) = regions.iter().zip(next_starts).nth(position)?;
            let start = region.start;
            (start < first_start && first_start < end).then_some((address, position, end))
        })
        .ok_or(DefectError::InvalidBoxStructure)?;
    let mut frames = Vec::new();
    let mut expected = first_start;
    for (position_in_mdat, range) in ranges.iter().enumerate() {
        let start = range.start;
        let frame_end = range.end;
        if start < expected || frame_end > end {
            return Err(DefectError::InvalidBoxStructure);
        }
        expected = frame_end;
        let index = u32::try_from(position_in_mdat).map_err(DefectError::size_overflow)?;
        frames.push(Region {
            start,
            address: holder_address
                .clone()
                .child(BoxStep::frame(FrameIndex::new(index))),
        });
    }
    let insert_at = position.checked_add(1).ok_or(DefectError::SizeOverflow)?;
    let later_regions = regions.split_off(insert_at);
    regions.extend(frames);
    regions.extend(later_regions);
    Ok(())
}
