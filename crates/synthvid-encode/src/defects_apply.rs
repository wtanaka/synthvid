//! Implementation of per-defect application functions.

use super::{defects_box, Axis, BoxPath, BoxTag, ByteOffset, DefectError};
use defects_box::{
    extract_4_bytes, find_avi_box, find_avi_box_with_ancestors, find_iso_box,
    find_iso_box_with_ancestors, is_avi_format, is_iso_format, write_u32, ContainerBytes,
    Endianness,
};

// Re-export apply_reordered_boxes from the reorder module
pub(super) use super::defects_reorder::apply_reordered_boxes;

/// Apply `TruncateAt` defect
pub(super) fn apply_truncate_at(
    offset: ByteOffset,
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    let off = offset.get();
    if off > input.len() {
        return Err(DefectError::OffsetOutOfRange);
    }
    Ok(input
        .as_slice()
        .get(..off)
        .ok_or(DefectError::OffsetOutOfRange)?
        .to_vec())
}

/// Apply `TruncateBox` defect
pub(super) fn apply_truncate_box(
    path: &BoxPath,
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    let is_iso = is_iso_format(input)?;
    if is_iso {
        let box_info = find_iso_box(input, path)?;
        // buffer indexing requires a raw usize; this is the single point where
        // the offset re-enters raw-usize space
        let start_pos = box_info.payload_start.get();
        Ok(input
            .as_slice()
            .get(..start_pos)
            .ok_or(DefectError::InvalidBoxStructure)?
            .to_vec())
    } else if is_avi_format(input) {
        let box_info = find_avi_box(input, path)?;
        // buffer indexing requires a raw usize; this is the single point where
        // the offset re-enters raw-usize space
        let start_pos = box_info.payload_start.get();
        Ok(input
            .as_slice()
            .get(..start_pos)
            .ok_or(DefectError::InvalidBoxStructure)?
            .to_vec())
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}

/// Apply `ZeroLengthPayload` defect
pub(super) fn apply_zero_length_payload(
    path: &BoxPath,
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    let is_iso = is_iso_format(input)?;
    if is_iso {
        let box_info = find_iso_box(input, path)?;
        let mut result = input.as_slice().to_vec();
        write_u32(
            &mut result,
            box_info.box_start.as_absolute(),
            8,
            Endianness::Big,
        )?;
        Ok(result)
    } else if is_avi_format(input) {
        let box_info = find_avi_box(input, path)?;
        let mut result = input.as_slice().to_vec();
        write_u32(
            &mut result,
            box_info.box_start.as_absolute(),
            8,
            Endianness::Little,
        )?;
        Ok(result)
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}

/// Apply `OversizedBoxLength` defect
pub(super) fn apply_oversized_box_length(
    path: &BoxPath,
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    let is_iso = is_iso_format(input)?;
    if is_iso {
        let box_info = find_iso_box(input, path)?;
        let size_pos = box_info.box_start.as_absolute();
        let size_bytes =
            extract_4_bytes(input, size_pos).ok_or(DefectError::InvalidBoxStructure)?;
        let orig_size = u32::from_be_bytes(size_bytes);
        let oversized = orig_size
            .checked_add(256)
            .ok_or(DefectError::SizeOverflow)?;
        let mut result = input.as_slice().to_vec();
        write_u32(&mut result, size_pos, oversized, Endianness::Big)?;
        Ok(result)
    } else if is_avi_format(input) {
        let box_info = find_avi_box(input, path)?;
        let size_pos = box_info.box_start.avi_size_field()?;
        let size_bytes =
            extract_4_bytes(input, size_pos).ok_or(DefectError::InvalidBoxStructure)?;
        let orig_size = u32::from_le_bytes(size_bytes);
        let oversized = orig_size
            .checked_add(256)
            .ok_or(DefectError::SizeOverflow)?;
        let mut result = input.as_slice().to_vec();
        write_u32(&mut result, size_pos, oversized, Endianness::Little)?;
        Ok(result)
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}

/// Apply `UndersizedBoxLength` defect
pub(super) fn apply_undersized_box_length(
    path: &BoxPath,
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    let is_iso = is_iso_format(input)?;
    if is_iso {
        let box_info = find_iso_box(input, path)?;
        let size_pos = box_info.box_start.as_absolute();
        let size_bytes =
            extract_4_bytes(input, size_pos).ok_or(DefectError::InvalidBoxStructure)?;
        let orig_size = u32::from_be_bytes(size_bytes);
        let undersized = orig_size.checked_sub(1).ok_or(DefectError::SizeOverflow)?;
        let mut result = input.as_slice().to_vec();
        write_u32(&mut result, size_pos, undersized, Endianness::Big)?;
        Ok(result)
    } else if is_avi_format(input) {
        let box_info = find_avi_box(input, path)?;
        let size_pos = box_info.box_start.avi_size_field()?;
        let size_bytes =
            extract_4_bytes(input, size_pos).ok_or(DefectError::InvalidBoxStructure)?;
        let orig_size = u32::from_le_bytes(size_bytes);
        let undersized = orig_size.checked_sub(1).ok_or(DefectError::SizeOverflow)?;
        let mut result = input.as_slice().to_vec();
        write_u32(&mut result, size_pos, undersized, Endianness::Little)?;
        Ok(result)
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}

/// Apply `OffsetPastEnd` defect
pub(super) fn apply_offset_past_end(
    path: &BoxPath,
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    let is_iso = is_iso_format(input)?;
    let is_avi = is_avi_format(input);

    if is_iso {
        let box_info = find_iso_box(input, path)?;
        let mut result = input.as_slice().to_vec();

        if box_info
            .payload_start
            .get()
            .checked_add(8)
            .ok_or(DefectError::SizeOverflow)?
            > box_info.payload_end.get()
        {
            return Err(DefectError::InvalidBoxStructure);
        }

        // stco payload layout: version(1)+flags(3) = 0-4, entry_count(4) = 4-8, entries at 8+
        let entry_count_pos = box_info.payload_start.field_at(4)?;
        let entry_count_bytes =
            extract_4_bytes(input, entry_count_pos).ok_or(DefectError::InvalidBoxStructure)?;
        let entry_count = u32::from_be_bytes(entry_count_bytes);

        if entry_count == 0 {
            return Err(DefectError::InvalidBoxStructure);
        }

        // First chunk-offset entry is at payload_start + 8
        let entries_start = box_info.payload_start.field_at(8)?;
        let new_offset = u32::try_from(
            input
                .len()
                .checked_add(1)
                .ok_or(DefectError::SizeOverflow)?,
        )
        .map_err(|_| DefectError::SizeOverflow)?;
        write_u32(&mut result, entries_start, new_offset, Endianness::Big)?;
        Ok(result)
    } else if is_avi {
        let box_info = find_avi_box(input, path)?;
        let mut result = input.as_slice().to_vec();

        if box_info
            .payload_start
            .get()
            .checked_add(8)
            .ok_or(DefectError::SizeOverflow)?
            > box_info.payload_end.get()
        {
            return Err(DefectError::InvalidBoxStructure);
        }

        // idx1 payload layout: version(1)+flags(3) = 0-4, entry_count(4) = 4-8, entries at 8+
        let entry_count_pos = box_info.payload_start.field_at(4)?;
        let entry_count_bytes =
            extract_4_bytes(input, entry_count_pos).ok_or(DefectError::InvalidBoxStructure)?;
        let entry_count = u32::from_le_bytes(entry_count_bytes);

        if entry_count == 0 {
            return Err(DefectError::InvalidBoxStructure);
        }

        // First index entry is at payload_start + 8
        let entries_start = box_info.payload_start.field_at(8)?;
        let new_offset = u32::try_from(
            input
                .len()
                .checked_add(1)
                .ok_or(DefectError::SizeOverflow)?,
        )
        .map_err(|_| DefectError::SizeOverflow)?;
        write_u32(&mut result, entries_start, new_offset, Endianness::Little)?;
        Ok(result)
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}

/// Apply `DuplicateBox` defect
pub(super) fn apply_duplicate_box(
    path: &BoxPath,
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    let is_iso = is_iso_format(input)?;
    let is_avi = is_avi_format(input);

    if is_iso {
        let box_with_ancestors = find_iso_box_with_ancestors(input, path)?;
        let box_info = &box_with_ancestors.box_info;
        let box_start_usize = box_info.box_start.get();
        let box_len = box_info
            .payload_end
            .get()
            .checked_sub(box_start_usize)
            .ok_or(DefectError::SizeOverflow)?;

        let mut result = input.as_slice().to_vec();
        // buffer indexing requires a raw usize; this is the single point where
        // the offset re-enters raw-usize space
        let box_bytes = input
            .as_slice()
            .get(
                box_start_usize
                    ..box_start_usize
                        .checked_add(box_len)
                        .ok_or(DefectError::SizeOverflow)?,
            )
            .ok_or(DefectError::SizeOverflow)?
            .to_vec();

        let insert_pos = box_start_usize
            .checked_add(box_len)
            .ok_or(DefectError::SizeOverflow)?;
        result.splice(insert_pos..insert_pos, box_bytes.iter().copied());

        // Patch all ancestor size fields
        for &ancestor_size_offset in &box_with_ancestors.ancestor_size_offsets {
            let result_container = ContainerBytes::new(&result);
            let size_bytes = extract_4_bytes(result_container, ancestor_size_offset)
                .ok_or(DefectError::InvalidBoxStructure)?;
            let current_size = u32::from_be_bytes(size_bytes);
            let box_len_u32 = u32::try_from(box_len).map_err(|_| DefectError::SizeOverflow)?;
            let new_size = current_size
                .checked_add(box_len_u32)
                .ok_or(DefectError::SizeOverflow)?;
            write_u32(&mut result, ancestor_size_offset, new_size, Endianness::Big)?;
        }

        Ok(result)
    } else if is_avi {
        let box_with_ancestors = find_avi_box_with_ancestors(input, path)?;
        let box_info = &box_with_ancestors.box_info;
        let box_start_usize = box_info.box_start.get();
        let box_len = box_info
            .payload_end
            .get()
            .checked_sub(box_start_usize)
            .ok_or(DefectError::SizeOverflow)?;

        let mut result = input.as_slice().to_vec();
        // buffer indexing requires a raw usize; this is the single point where
        // the offset re-enters raw-usize space
        let box_bytes = input
            .as_slice()
            .get(
                box_start_usize
                    ..box_start_usize
                        .checked_add(box_len)
                        .ok_or(DefectError::SizeOverflow)?,
            )
            .ok_or(DefectError::SizeOverflow)?
            .to_vec();

        let insert_pos = box_start_usize
            .checked_add(box_len)
            .ok_or(DefectError::SizeOverflow)?;
        result.splice(insert_pos..insert_pos, box_bytes.iter().copied());

        // Patch all ancestor size fields
        for &ancestor_size_offset in &box_with_ancestors.ancestor_size_offsets {
            let result_container = ContainerBytes::new(&result);
            let size_bytes = extract_4_bytes(result_container, ancestor_size_offset)
                .ok_or(DefectError::InvalidBoxStructure)?;
            let current_size = u32::from_le_bytes(size_bytes);
            let box_len_u32 = u32::try_from(box_len).map_err(|_| DefectError::SizeOverflow)?;
            let new_size = current_size
                .checked_add(box_len_u32)
                .ok_or(DefectError::SizeOverflow)?;
            write_u32(
                &mut result,
                ancestor_size_offset,
                new_size,
                Endianness::Little,
            )?;
        }

        Ok(result)
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}

/// Apply `UnknownBox` defect
pub(super) fn apply_unknown_box(
    after: &BoxPath,
    tag: BoxTag,
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    let is_iso = is_iso_format(input)?;
    let is_avi = is_avi_format(input);

    if is_iso {
        let box_with_ancestors = find_iso_box_with_ancestors(input, after)?;
        let box_info = &box_with_ancestors.box_info;
        // Insert the new box immediately after this box (after its header + payload)
        // buffer indexing requires a raw usize; this is the single point where
        // the offset re-enters raw-usize space
        let insert_pos = box_info.payload_end.get();

        let mut result = input.as_slice().to_vec();
        // Create an 8-byte box: 4 bytes size (value 8) + 4 bytes tag, zero payload
        let new_box = [8u32.to_be_bytes().as_ref(), tag.get().as_ref()].concat();
        let insert_size = new_box.len();
        result.splice(insert_pos..insert_pos, new_box.iter().copied());

        // Patch all ancestor size fields (including the immediate parent of 'after')
        for &ancestor_size_offset in &box_with_ancestors.ancestor_size_offsets {
            let result_container = ContainerBytes::new(&result);
            let size_bytes = extract_4_bytes(result_container, ancestor_size_offset)
                .ok_or(DefectError::InvalidBoxStructure)?;
            let current_size = u32::from_be_bytes(size_bytes);
            let insert_size_u32 =
                u32::try_from(insert_size).map_err(|_| DefectError::SizeOverflow)?;
            let new_size = current_size
                .checked_add(insert_size_u32)
                .ok_or(DefectError::SizeOverflow)?;
            write_u32(&mut result, ancestor_size_offset, new_size, Endianness::Big)?;
        }

        // Also patch the parent of 'after' if 'after' itself was the target (not just an intermediate ancestor)
        let result_container = ContainerBytes::new(&result);
        let after_size_bytes = extract_4_bytes(result_container, box_info.box_start.as_absolute())
            .ok_or(DefectError::InvalidBoxStructure)?;
        let after_current_size = u32::from_be_bytes(after_size_bytes);
        let insert_size_u32 = u32::try_from(insert_size).map_err(|_| DefectError::SizeOverflow)?;
        let after_new_size = after_current_size
            .checked_add(insert_size_u32)
            .ok_or(DefectError::SizeOverflow)?;
        write_u32(
            &mut result,
            box_info.box_start.as_absolute(),
            after_new_size,
            Endianness::Big,
        )?;

        Ok(result)
    } else if is_avi {
        let box_with_ancestors = find_avi_box_with_ancestors(input, after)?;
        let box_info = &box_with_ancestors.box_info;
        // Insert the new box immediately after this box
        // buffer indexing requires a raw usize; this is the single point where
        // the offset re-enters raw-usize space
        let insert_pos = box_info.payload_end.get();

        let mut result = input.as_slice().to_vec();
        // Create an 8-byte box: 4 bytes tag + 4 bytes size (value 8), zero payload
        let size_le = 8u32.to_le_bytes();
        let new_box = [tag.get().as_ref(), size_le.as_ref()].concat();
        let insert_size = new_box.len();
        result.splice(insert_pos..insert_pos, new_box.iter().copied());

        // Patch all ancestor size fields (including the immediate parent of 'after')
        for &ancestor_size_offset in &box_with_ancestors.ancestor_size_offsets {
            let result_container = ContainerBytes::new(&result);
            let size_bytes = extract_4_bytes(result_container, ancestor_size_offset)
                .ok_or(DefectError::InvalidBoxStructure)?;
            let current_size = u32::from_le_bytes(size_bytes);
            let insert_size_u32 =
                u32::try_from(insert_size).map_err(|_| DefectError::SizeOverflow)?;
            let new_size = current_size
                .checked_add(insert_size_u32)
                .ok_or(DefectError::SizeOverflow)?;
            write_u32(
                &mut result,
                ancestor_size_offset,
                new_size,
                Endianness::Little,
            )?;
        }

        // Also patch the parent of 'after' if 'after' itself was the target (not just an intermediate ancestor)
        let result_container = ContainerBytes::new(&result);
        let after_size_bytes = extract_4_bytes(result_container, box_info.box_start.as_absolute())
            .ok_or(DefectError::InvalidBoxStructure)?;
        let after_current_size = u32::from_le_bytes(after_size_bytes);
        let insert_size_u32 = u32::try_from(insert_size).map_err(|_| DefectError::SizeOverflow)?;
        let after_new_size = after_current_size
            .checked_add(insert_size_u32)
            .ok_or(DefectError::SizeOverflow)?;
        write_u32(
            &mut result,
            box_info.box_start.as_absolute(),
            after_new_size,
            Endianness::Little,
        )?;

        Ok(result)
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}

/// Apply `ZeroFrameRate` defect
pub(super) fn apply_zero_frame_rate(input: ContainerBytes<'_>) -> Result<Vec<u8>, DefectError> {
    let is_iso = is_iso_format(input)?;
    let is_avi = is_avi_format(input);

    if is_iso {
        let mvhd_path = BoxPath::root()
            .child(BoxTag::new(*b"moov"))
            .child(BoxTag::new(*b"mvhd"));
        let box_info = find_iso_box(input, &mvhd_path)?;
        let mut result = input.as_slice().to_vec();

        let timescale_pos = box_info.payload_start.field_at(12)?;
        write_u32(&mut result, timescale_pos, 0, Endianness::Big)?;
        Ok(result)
    } else if is_avi {
        let strh_path = BoxPath::root()
            .child(BoxTag::new(*b"hdrl"))
            .child(BoxTag::new(*b"strl"))
            .child(BoxTag::new(*b"strh"));
        let box_info = find_avi_box(input, &strh_path)?;
        let mut result = input.as_slice().to_vec();

        // strh payload layout: fourCC type "vids"(4) = 0-4, codec(4) = 4-8,
        // flags(4) = 8-12, priority+language(4) = 12-16, initial_frames(4) = 16-20,
        // scale/denominator(4) = 20-24, rate/numerator(4) = 24-28
        let rate_pos = box_info.payload_start.field_at(24)?;
        write_u32(&mut result, rate_pos, 0, Endianness::Little)?;
        Ok(result)
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}

/// Apply `ZeroDimension` defect
pub(super) fn apply_zero_dimension(
    axis: Axis,
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    let is_iso = is_iso_format(input)?;
    let is_avi = is_avi_format(input);

    if is_iso {
        let tkhd_path = BoxPath::root()
            .child(BoxTag::new(*b"moov"))
            .child(BoxTag::new(*b"trak"))
            .child(BoxTag::new(*b"tkhd"));
        let box_info = find_iso_box(input, &tkhd_path)?;
        let mut result = input.as_slice().to_vec();

        // tkhd payload layout: version(1)+flags(3) = 0-4, creation_time(4) = 4-8,
        // modification_time(4) = 8-12, track_ID(4) = 12-16, reserved(4) = 16-20,
        // duration(4) = 20-24, reserved[8] = 24-32, layer(2) = 32-34,
        // alternate_group(2) = 34-36, volume(2) = 36-38, reserved[2] = 38-40,
        // matrix[9 i32] = 40-76, width(4) = 76-80, height(4) = 80-84
        let width_pos = box_info.payload_start.field_at(76)?;
        let height_pos = box_info.payload_start.field_at(80)?;

        match axis {
            Axis::Width => write_u32(&mut result, width_pos, 0, Endianness::Big)?,
            Axis::Height => write_u32(&mut result, height_pos, 0, Endianness::Big)?,
        }
        Ok(result)
    } else if is_avi {
        let avih_path = BoxPath::root()
            .child(BoxTag::new(*b"hdrl"))
            .child(BoxTag::new(*b"avih"));
        let box_info = find_avi_box(input, &avih_path)?;
        let mut result = input.as_slice().to_vec();

        let width_pos = box_info.payload_start.field_at(32)?;
        let height_pos = box_info.payload_start.field_at(36)?;

        match axis {
            Axis::Width => write_u32(&mut result, width_pos, 0, Endianness::Little)?,
            Axis::Height => write_u32(&mut result, height_pos, 0, Endianness::Little)?,
        }
        Ok(result)
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}

/// Apply `AbsurdFrameRate` defect
pub(super) fn apply_absurd_frame_rate(
    frame_rate: &synthvid_scene::FrameRate,
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    let is_iso = is_iso_format(input)?;
    let is_avi = is_avi_format(input);

    if is_iso {
        let mvhd_path = BoxPath::root()
            .child(BoxTag::new(*b"moov"))
            .child(BoxTag::new(*b"mvhd"));
        let box_info = find_iso_box(input, &mvhd_path)?;
        let mut result = input.as_slice().to_vec();

        let timescale_pos = box_info.payload_start.field_at(12)?;
        let rate = frame_rate.ratio();
        let numerator = u32::try_from(rate.numer()).map_err(|_| DefectError::SizeOverflow)?;
        write_u32(&mut result, timescale_pos, numerator, Endianness::Big)?;
        Ok(result)
    } else if is_avi {
        let strh_path = BoxPath::root()
            .child(BoxTag::new(*b"hdrl"))
            .child(BoxTag::new(*b"strl"))
            .child(BoxTag::new(*b"strh"));
        let box_info = find_avi_box(input, &strh_path)?;
        let mut result = input.as_slice().to_vec();

        // strh payload layout: rate/numerator(4) = 24-28
        let rate_pos = box_info.payload_start.field_at(24)?;
        let rate = frame_rate.ratio();
        let numerator = u32::try_from(rate.numer()).map_err(|_| DefectError::SizeOverflow)?;
        write_u32(&mut result, rate_pos, numerator, Endianness::Little)?;
        Ok(result)
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}

/// Apply `NonMonotonicTimestamps` defect
pub(super) fn apply_non_monotonic_timestamps(
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    let is_iso = is_iso_format(input)?;

    if is_iso {
        let stts_path = BoxPath::root()
            .child(BoxTag::new(*b"moov"))
            .child(BoxTag::new(*b"trak"))
            .child(BoxTag::new(*b"mdia"))
            .child(BoxTag::new(*b"minf"))
            .child(BoxTag::new(*b"stbl"))
            .child(BoxTag::new(*b"stts"));
        let box_info = find_iso_box(input, &stts_path)?;
        let mut result = input.as_slice().to_vec();

        if box_info
            .payload_start
            .get()
            .checked_add(12)
            .ok_or(DefectError::SizeOverflow)?
            > box_info.payload_end.get()
        {
            return Err(DefectError::InvalidBoxStructure);
        }

        // stts payload layout: version(1)+flags(3) = 0-4, entry_count(4) = 4-8,
        // then per-entry pairs: sample_count(4) = 8-12, sample_delta(4) = 12-16
        let first_delta_pos = box_info.payload_start.field_at(12)?;
        write_u32(&mut result, first_delta_pos, 0, Endianness::Big)?;
        Ok(result)
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}

/// Apply `DeclaredCountMismatch` defect
pub(super) fn apply_declared_count_mismatch(
    declared: u32,
    input: ContainerBytes<'_>,
) -> Result<Vec<u8>, DefectError> {
    let is_iso = is_iso_format(input)?;
    let is_avi = is_avi_format(input);

    if is_iso {
        // Find stsz box: moov -> trak -> mdia -> minf -> stbl -> stsz
        let stsz_path = BoxPath::root()
            .child(BoxTag::new(*b"moov"))
            .child(BoxTag::new(*b"trak"))
            .child(BoxTag::new(*b"mdia"))
            .child(BoxTag::new(*b"minf"))
            .child(BoxTag::new(*b"stbl"))
            .child(BoxTag::new(*b"stsz"));
        let box_info = find_iso_box(input, &stsz_path)?;
        let mut result = input.as_slice().to_vec();

        // stsz payload layout: version(1)+flags(3) = 0-4, sample_size(4) = 4-8,
        // sample_count(4) = 8-12
        let sample_count_pos = box_info.payload_start.field_at(8)?;
        write_u32(&mut result, sample_count_pos, declared, Endianness::Big)?;
        Ok(result)
    } else if is_avi {
        let avih_path = BoxPath::root()
            .child(BoxTag::new(*b"hdrl"))
            .child(BoxTag::new(*b"avih"));
        let box_info = find_avi_box(input, &avih_path)?;
        let mut result = input.as_slice().to_vec();

        let frame_count_pos = box_info.payload_start.field_at(16)?;
        write_u32(&mut result, frame_count_pos, declared, Endianness::Little)?;
        Ok(result)
    } else {
        Err(DefectError::IncompatibleFileType)
    }
}
