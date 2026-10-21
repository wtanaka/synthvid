//! Implementation of per-defect application functions.

use super::defects_layout::Layout;
use super::defects_types::{ContainerFormat, FieldOffset, FieldValue, FileBytes};
use super::{defects_box, BoxAddress, BoxPath, BoxTag, DefectError};
use core::num::NonZeroU32;
use defects_box::{
    detect_format, extract_4_bytes, find_avi_box, find_avi_box_with_ancestors, find_iso_box,
    find_iso_box_with_ancestors, write_u32, AbsoluteOffset, BoxInfo, BoxLen, ContainerBytes,
    Endianness,
};

// Re-export apply_reordered_boxes from the reorder module
pub(super) use super::defects_reorder::apply_reordered_boxes;

/// Apply `TruncateBefore` defect.
pub(super) fn apply_truncate_before(
    address: &BoxAddress,
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    let (start, _) = Layout::of(input)?.span(address)?;
    cut_at(input, start)
}

/// Apply `TruncateWithin` defect.
pub(super) fn apply_truncate_within(
    address: &BoxAddress,
    offset: NonZeroU32,
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    let (start, end) = Layout::of(input)?.span(address)?;
    let owned = end
        .get()
        .checked_sub(start.get())
        .ok_or(DefectError::SizeOverflow)?;
    let into = usize::try_from(offset.get()).map_err(DefectError::offset_out_of_range)?;
    if into >= owned {
        return Err(DefectError::OffsetOutOfRange);
    }
    cut_at(
        input,
        AbsoluteOffset::new(
            start
                .get()
                .checked_add(into)
                .ok_or(DefectError::SizeOverflow)?,
        ),
    )
}

/// Keeps the first `cut` bytes. The one place a cut position re-enters raw
/// `usize` space.
fn cut_at(input: ContainerBytes<'_>, cut: AbsoluteOffset) -> Result<FileBytes, DefectError> {
    input
        .as_slice()
        .get(..cut.get())
        .map(|prefix| FileBytes(prefix.to_vec()))
        .ok_or(DefectError::OffsetOutOfRange)
}

/// Apply `ZeroLengthPayload` defect.
pub(super) fn apply_zero_length_payload(
    path: &BoxPath,
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    match detect_format(input)? {
        ContainerFormat::Iso => {
            let box_info = find_iso_box(input, path)?;
            let mut result = FileBytes::copy_of(input);
            write_u32(
                &mut result,
                box_info.box_start.as_absolute(),
                FieldValue(8),
                Endianness::Big,
            )?;
            Ok(result)
        }
        ContainerFormat::Avi => {
            let box_info = find_avi_box(input, path)?;
            let mut result = FileBytes::copy_of(input);
            write_u32(
                &mut result,
                box_info.box_start.as_absolute(),
                FieldValue(8),
                Endianness::Little,
            )?;
            Ok(result)
        }
        ContainerFormat::Neither => Err(DefectError::IncompatibleFileType),
    }
}

/// Apply `OversizedBoxLength` defect.
pub(super) fn apply_oversized_box_length(
    path: &BoxPath,
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    match detect_format(input)? {
        ContainerFormat::Iso => {
            let box_info = find_iso_box(input, path)?;
            let size_pos = box_info.box_start.as_absolute();
            let size_bytes =
                extract_4_bytes(input, size_pos).ok_or(DefectError::InvalidBoxStructure)?;
            let orig_size = u32::from_be_bytes(size_bytes.0);
            let oversized = orig_size
                .checked_add(256)
                .ok_or(DefectError::SizeOverflow)?;
            let mut result = FileBytes::copy_of(input);
            write_u32(
                &mut result,
                size_pos,
                FieldValue(oversized),
                Endianness::Big,
            )?;
            Ok(result)
        }
        ContainerFormat::Avi => {
            let box_info = find_avi_box(input, path)?;
            let size_pos = box_info.box_start.avi_size_field()?;
            let size_bytes =
                extract_4_bytes(input, size_pos).ok_or(DefectError::InvalidBoxStructure)?;
            let orig_size = u32::from_le_bytes(size_bytes.0);
            let oversized = orig_size
                .checked_add(256)
                .ok_or(DefectError::SizeOverflow)?;
            let mut result = FileBytes::copy_of(input);
            write_u32(
                &mut result,
                size_pos,
                FieldValue(oversized),
                Endianness::Little,
            )?;
            Ok(result)
        }
        ContainerFormat::Neither => Err(DefectError::IncompatibleFileType),
    }
}

/// Apply `UndersizedBoxLength` defect.
pub(super) fn apply_undersized_box_length(
    path: &BoxPath,
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    match detect_format(input)? {
        ContainerFormat::Iso => {
            let box_info = find_iso_box(input, path)?;
            let size_pos = box_info.box_start.as_absolute();
            let size_bytes =
                extract_4_bytes(input, size_pos).ok_or(DefectError::InvalidBoxStructure)?;
            let orig_size = u32::from_be_bytes(size_bytes.0);
            let undersized = orig_size.checked_sub(1).ok_or(DefectError::SizeOverflow)?;
            let mut result = FileBytes::copy_of(input);
            write_u32(
                &mut result,
                size_pos,
                FieldValue(undersized),
                Endianness::Big,
            )?;
            Ok(result)
        }
        ContainerFormat::Avi => {
            let box_info = find_avi_box(input, path)?;
            let size_pos = box_info.box_start.avi_size_field()?;
            let size_bytes =
                extract_4_bytes(input, size_pos).ok_or(DefectError::InvalidBoxStructure)?;
            let orig_size = u32::from_le_bytes(size_bytes.0);
            let undersized = orig_size.checked_sub(1).ok_or(DefectError::SizeOverflow)?;
            let mut result = FileBytes::copy_of(input);
            write_u32(
                &mut result,
                size_pos,
                FieldValue(undersized),
                Endianness::Little,
            )?;
            Ok(result)
        }
        ContainerFormat::Neither => Err(DefectError::IncompatibleFileType),
    }
}

/// The two container formats a defect can be applied to, as found in a file.
#[derive(Debug, Clone, Copy)]
enum BoxFormat {
    /// An ISO base media file: big-endian sizes, size before tag.
    Iso,
    /// An AVI (RIFF) file: little-endian sizes, tag before size.
    Avi,
}

impl BoxFormat {
    /// The format of `input`, or `IncompatibleFileType` for neither.
    fn of(input: ContainerBytes<'_>) -> Result<Self, DefectError> {
        match detect_format(input)? {
            ContainerFormat::Iso => Ok(Self::Iso),
            ContainerFormat::Avi => Ok(Self::Avi),
            ContainerFormat::Neither => Err(DefectError::IncompatibleFileType),
        }
    }

    /// The byte order of this format's size and offset fields.
    const fn endianness(self) -> Endianness {
        match self {
            Self::Iso => Endianness::Big,
            Self::Avi => Endianness::Little,
        }
    }

    /// An empty box tagged `tag`: an 8-byte header declaring a size of 8.
    const fn empty_box(self, tag: BoxTag) -> EmptyBox {
        EmptyBox(match self {
            Self::Iso => [8_u32.to_be_bytes(), tag.get()],
            Self::Avi => [tag.get(), 8_u32.to_le_bytes()],
        })
    }
}

/// The bytes of an empty box: its two 4-byte header words, in file order.
struct EmptyBox([[u8; 4]; 2]);

/// A box found in a file, with the size fields of every box that holds it.
struct FoundBox {
    /// The format the box was found in.
    format: BoxFormat,
    /// The box itself.
    box_info: BoxInfo,
    /// Offsets of the ancestors' size fields, from the root down to the
    /// immediate parent.
    ancestor_size_offsets: Vec<AbsoluteOffset>,
}

impl FoundBox {
    /// Finds the box at `path` and its ancestors' size fields.
    fn at(input: ContainerBytes<'_>, path: &BoxPath) -> Result<Self, DefectError> {
        let format = BoxFormat::of(input)?;
        let (box_info, ancestor_size_offsets) = match format {
            BoxFormat::Iso => {
                let found = find_iso_box_with_ancestors(input, path)?;
                (found.box_info, found.ancestor_size_offsets)
            }
            BoxFormat::Avi => {
                let found = find_avi_box_with_ancestors(input, path)?;
                (found.box_info, found.ancestor_size_offsets)
            }
        };
        Ok(Self {
            format,
            box_info,
            ancestor_size_offsets,
        })
    }

    /// Adds `grown_by` to the size field of every ancestor.
    fn grow_ancestors(&self, result: &mut FileBytes, grown_by: BoxLen) -> Result<(), DefectError> {
        for &ancestor_size_offset in &self.ancestor_size_offsets {
            grow_size_field(
                result,
                ancestor_size_offset,
                grown_by,
                self.format.endianness(),
            )?;
        }
        Ok(())
    }
}

/// Adds `grown_by` to the 4-byte size field at `offset`.
fn grow_size_field(
    result: &mut FileBytes,
    offset: AbsoluteOffset,
    grown_by: BoxLen,
    endianness: Endianness,
) -> Result<(), DefectError> {
    let size_bytes =
        extract_4_bytes(result.as_container(), offset).ok_or(DefectError::InvalidBoxStructure)?;
    let current_size = match endianness {
        Endianness::Big => u32::from_be_bytes(size_bytes.0),
        Endianness::Little => u32::from_le_bytes(size_bytes.0),
    };
    let grown_by_u32 = u32::try_from(grown_by.get()).map_err(DefectError::size_overflow)?;
    let new_size = current_size
        .checked_add(grown_by_u32)
        .ok_or(DefectError::SizeOverflow)?;
    write_u32(result, offset, FieldValue(new_size), endianness)
}

/// Apply `OffsetPastEnd` defect.
///
/// The ISO `stco` and AVI `idx1` payloads share a layout here:
/// version (1 byte) and flags (3) at 0-4, `entry_count` (4) at 4-8, and the
/// entries from 8.
pub(super) fn apply_offset_past_end(
    path: &BoxPath,
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    let format = BoxFormat::of(input)?;
    let box_info = match format {
        BoxFormat::Iso => find_iso_box(input, path)?,
        BoxFormat::Avi => find_avi_box(input, path)?,
    };
    let mut result = FileBytes::copy_of(input);

    if box_info
        .payload_start
        .get()
        .checked_add(8)
        .ok_or(DefectError::SizeOverflow)?
        > box_info.payload_end.get()
    {
        return Err(DefectError::InvalidBoxStructure);
    }

    let entry_count_pos = box_info.payload_start.field_at(FieldOffset(4))?;
    let entry_count_bytes =
        extract_4_bytes(input, entry_count_pos).ok_or(DefectError::InvalidBoxStructure)?;
    let entry_count = match format.endianness() {
        Endianness::Big => u32::from_be_bytes(entry_count_bytes.0),
        Endianness::Little => u32::from_le_bytes(entry_count_bytes.0),
    };

    if entry_count == 0 {
        return Err(DefectError::InvalidBoxStructure);
    }

    // The first entry is at payload_start + 8
    let entries_start = box_info.payload_start.field_at(FieldOffset(8))?;
    let new_offset = u32::try_from(
        input
            .as_slice()
            .len()
            .checked_add(1)
            .ok_or(DefectError::SizeOverflow)?,
    )
    .map_err(DefectError::size_overflow)?;
    write_u32(
        &mut result,
        entries_start,
        FieldValue(new_offset),
        format.endianness(),
    )?;
    Ok(result)
}

/// Apply `DuplicateBox` defect.
pub(super) fn apply_duplicate_box(
    path: &BoxPath,
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    let found = FoundBox::at(input, path)?;
    let box_start_usize = found.box_info.box_start.get();
    let box_len = BoxLen::new(
        found
            .box_info
            .payload_end
            .get()
            .checked_sub(box_start_usize)
            .ok_or(DefectError::SizeOverflow)?,
    );

    // buffer indexing requires a raw usize; this is the single point where
    // the offset re-enters raw-usize space
    let insert_pos = box_start_usize
        .checked_add(box_len.get())
        .ok_or(DefectError::SizeOverflow)?;
    let box_bytes = input
        .as_slice()
        .get(box_start_usize..insert_pos)
        .ok_or(DefectError::SizeOverflow)?;
    let (head, tail) = input
        .as_slice()
        .split_at_checked(insert_pos)
        .ok_or(DefectError::SizeOverflow)?;
    let mut result = FileBytes([head, box_bytes, tail].concat());

    found.grow_ancestors(&mut result, box_len)?;
    Ok(result)
}

/// Apply `UnknownBox` defect.
pub(super) fn apply_unknown_box(
    after: &BoxPath,
    tag: BoxTag,
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    let found = FoundBox::at(input, after)?;
    // Insert the new box immediately after this box (after its header + payload)
    // buffer indexing requires a raw usize; this is the single point where
    // the offset re-enters raw-usize space
    let insert_pos = found.box_info.payload_end.get();

    let new_box = found.format.empty_box(tag);
    let new_box_bytes = new_box.0.as_flattened();
    let insert_size = BoxLen::new(new_box_bytes.len());
    let (head, tail) = input
        .as_slice()
        .split_at_checked(insert_pos)
        .ok_or(DefectError::InvalidBoxStructure)?;
    let mut result = FileBytes([head, new_box_bytes, tail].concat());

    // Patch all ancestor size fields (including the immediate parent of 'after')
    found.grow_ancestors(&mut result, insert_size)?;

    // Also patch the parent of 'after' if 'after' itself was the target (not just an intermediate ancestor)
    grow_size_field(
        &mut result,
        found.box_info.box_start.as_absolute(),
        insert_size,
        found.format.endianness(),
    )?;

    Ok(result)
}
