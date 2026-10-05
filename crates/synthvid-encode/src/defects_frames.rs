//! Addresses the frames of an ISO file.
//!
//! A frame in ISO is not a box: the frames sit back to back in `mdat`, and the
//! sample tables under `moov` say where each begins and how long it is. This
//! lists them as regions below the `mdat` that holds them, so a frame is named
//! by its number, as it is in AVI.
//!
//! The encoder writes one sample per chunk, so the first video track's `stsz`
//! gives each frame's length and its `stco` gives each frame's offset.

use super::defects_box::{extract_4_bytes, find_iso_box, AbsoluteOffset, ContainerBytes};
use super::defects_layout::{BoxAddress, BoxStep, ContainerKind, Region};
use super::{BoxPath, BoxTag, DefectError};
use synthvid_scene::FrameIndex;

/// The offset and length of one frame.
struct FrameRange {
    /// Offset of the frame's first byte.
    start: usize,
    /// Length of the frame in bytes.
    len: usize,
}

/// The path to a box of the first track's sample table.
fn sample_table_box(tag: [u8; 4]) -> BoxPath {
    [*b"moov", *b"trak", *b"mdia", *b"minf", *b"stbl", tag]
        .into_iter()
        .fold(BoxPath::root(), |path, tag| path.child(BoxTag::new(tag)))
}

/// Reads the big-endian `u32` at `at`.
fn read_u32(input: ContainerBytes<'_>, at: AbsoluteOffset) -> Result<usize, DefectError> {
    let bytes = extract_4_bytes(input, at).ok_or(DefectError::InvalidBoxStructure)?;
    usize::try_from(u32::from_be_bytes(bytes)).map_err(|_| DefectError::SizeOverflow)
}

/// Reads the frames the sample tables list; a file with no sample table has no
/// frames.
fn frame_ranges(input: ContainerBytes<'_>) -> Result<Vec<FrameRange>, DefectError> {
    let (stsz, stco) = match (
        find_iso_box(input, &sample_table_box(*b"stsz")),
        find_iso_box(input, &sample_table_box(*b"stco")),
    ) {
        (Ok(stsz), Ok(stco)) => (stsz, stco),
        (Err(DefectError::BoxNotFound(_)), _) | (_, Err(DefectError::BoxNotFound(_))) => {
            return Ok(Vec::new());
        }
        (Err(other), _) | (_, Err(other)) => return Err(other),
    };
    // Both payloads start with a version and flags word, then the entry
    // counts; `stsz` has a constant sample size before its count.
    let constant_size = read_u32(input, stsz.payload_start.field_at(4)?)?;
    let sample_count = read_u32(input, stsz.payload_start.field_at(8)?)?;
    let chunk_count = read_u32(input, stco.payload_start.field_at(4)?)?;
    if sample_count != chunk_count {
        return Err(DefectError::InvalidBoxStructure);
    }
    let mut ranges = Vec::new();
    for index in 0..sample_count {
        let entry = index.checked_mul(4).ok_or(DefectError::SizeOverflow)?;
        let len = if constant_size == 0 {
            let at = entry.checked_add(12).ok_or(DefectError::SizeOverflow)?;
            read_u32(input, stsz.payload_start.field_at(at)?)?
        } else {
            constant_size
        };
        let at = entry.checked_add(8).ok_or(DefectError::SizeOverflow)?;
        let start = read_u32(input, stco.payload_start.field_at(at)?)?;
        ranges.push(FrameRange { start, len });
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
    let mdat = BoxTag::new(*b"mdat");
    // The first `mdat` that holds the first frame; the walk lists each top-level
    // `mdat` under its own occurrence number.
    let (holder_address, position, end) = (0..)
        .map_while(|occurrence| {
            let address = BoxAddress::top(ContainerKind::Iso, BoxStep::new(mdat, occurrence));
            regions
                .iter()
                .position(|region| region.address == address)
                .map(|position| (address, position))
        })
        .find_map(|(address, position)| {
            let start = regions.get(position)?.start;
            let end = position
                .checked_add(1)
                .and_then(|next| regions.get(next))
                .map_or(input.len(), |next| next.start);
            (start < first.start && first.start < end).then_some((address, position, end))
        })
        .ok_or(DefectError::InvalidBoxStructure)?;
    let mut frames = Vec::new();
    let mut expected = first.start;
    for (index, range) in ranges.iter().enumerate() {
        let frame_end = range
            .start
            .checked_add(range.len)
            .ok_or(DefectError::SizeOverflow)?;
        if range.start < expected || frame_end > end {
            return Err(DefectError::InvalidBoxStructure);
        }
        expected = frame_end;
        let index = u32::try_from(index).map_err(|_| DefectError::SizeOverflow)?;
        frames.push(Region {
            start: range.start,
            address: holder_address
                .clone()
                .child(BoxStep::frame(FrameIndex::new(index))),
        });
    }
    let insert_at = position.checked_add(1).ok_or(DefectError::SizeOverflow)?;
    regions.splice(insert_at..insert_at, frames);
    Ok(())
}
