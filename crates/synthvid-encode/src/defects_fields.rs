//! Defects that rewrite one header field: a frame rate, a frame dimension,
//! a declared count, or a timestamp.

use super::defects_box::{
    detect_format, find_avi_box, find_iso_box, write_u32, ContainerBytes, Endianness,
};
use super::defects_types::{ContainerFormat, FieldOffset, FieldValue, FileBytes};
use super::{Axis, BoxPath, BoxTag, DefectError};

/// Apply `ZeroFrameRate` defect.
pub(super) fn apply_zero_frame_rate(input: ContainerBytes<'_>) -> Result<FileBytes, DefectError> {
    match detect_format(input)? {
        ContainerFormat::Iso => {
            let mvhd_path = BoxPath::root()
                .child(BoxTag::new(*b"moov"))
                .child(BoxTag::new(*b"mvhd"));
            let box_info = find_iso_box(input, &mvhd_path)?;
            let mut result = FileBytes::copy_of(input);

            let timescale_pos = box_info.payload_start.field_at(FieldOffset(12))?;
            write_u32(&mut result, timescale_pos, FieldValue(0), Endianness::Big)?;
            Ok(result)
        }
        ContainerFormat::Avi => {
            let strh_path = BoxPath::root()
                .child(BoxTag::new(*b"hdrl"))
                .child(BoxTag::new(*b"strl"))
                .child(BoxTag::new(*b"strh"));
            let box_info = find_avi_box(input, &strh_path)?;
            let mut result = FileBytes::copy_of(input);

            // strh payload layout: fourCC type "vids"(4) = 0-4, codec(4) = 4-8,
            // flags(4) = 8-12, priority+language(4) = 12-16, initial_frames(4) = 16-20,
            // scale/denominator(4) = 20-24, rate/numerator(4) = 24-28
            let rate_pos = box_info.payload_start.field_at(FieldOffset(24))?;
            write_u32(&mut result, rate_pos, FieldValue(0), Endianness::Little)?;
            Ok(result)
        }
        ContainerFormat::Neither => Err(DefectError::IncompatibleFileType),
    }
}

/// Apply `ZeroDimension` defect.
pub(super) fn apply_zero_dimension(
    axis: Axis,
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    match detect_format(input)? {
        ContainerFormat::Iso => {
            let tkhd_path = BoxPath::root()
                .child(BoxTag::new(*b"moov"))
                .child(BoxTag::new(*b"trak"))
                .child(BoxTag::new(*b"tkhd"));
            let box_info = find_iso_box(input, &tkhd_path)?;
            let mut result = FileBytes::copy_of(input);

            // tkhd payload layout: version(1)+flags(3) = 0-4, creation_time(4) = 4-8,
            // modification_time(4) = 8-12, track_ID(4) = 12-16, reserved(4) = 16-20,
            // duration(4) = 20-24, reserved[8] = 24-32, layer(2) = 32-34,
            // alternate_group(2) = 34-36, volume(2) = 36-38, reserved[2] = 38-40,
            // matrix[9 i32] = 40-76, width(4) = 76-80, height(4) = 80-84
            let width_pos = box_info.payload_start.field_at(FieldOffset(76))?;
            let height_pos = box_info.payload_start.field_at(FieldOffset(80))?;

            match axis {
                Axis::Width => write_u32(&mut result, width_pos, FieldValue(0), Endianness::Big)?,
                Axis::Height => {
                    write_u32(&mut result, height_pos, FieldValue(0), Endianness::Big)?;
                }
            }
            Ok(result)
        }
        ContainerFormat::Avi => {
            let avih_path = BoxPath::root()
                .child(BoxTag::new(*b"hdrl"))
                .child(BoxTag::new(*b"avih"));
            let box_info = find_avi_box(input, &avih_path)?;
            let mut result = FileBytes::copy_of(input);

            let width_pos = box_info.payload_start.field_at(FieldOffset(32))?;
            let height_pos = box_info.payload_start.field_at(FieldOffset(36))?;

            match axis {
                Axis::Width => {
                    write_u32(&mut result, width_pos, FieldValue(0), Endianness::Little)?;
                }
                Axis::Height => {
                    write_u32(&mut result, height_pos, FieldValue(0), Endianness::Little)?;
                }
            }
            Ok(result)
        }
        ContainerFormat::Neither => Err(DefectError::IncompatibleFileType),
    }
}

/// Apply `AbsurdFrameRate` defect.
pub(super) fn apply_absurd_frame_rate(
    frame_rate: &synthvid_scene::FrameRate,
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    match detect_format(input)? {
        ContainerFormat::Iso => {
            let mvhd_path = BoxPath::root()
                .child(BoxTag::new(*b"moov"))
                .child(BoxTag::new(*b"mvhd"));
            let box_info = find_iso_box(input, &mvhd_path)?;
            let mut result = FileBytes::copy_of(input);

            let timescale_pos = box_info.payload_start.field_at(FieldOffset(12))?;
            let rate = frame_rate.ratio();
            let numerator = u32::try_from(rate.numer().0).map_err(DefectError::size_overflow)?;
            write_u32(
                &mut result,
                timescale_pos,
                FieldValue(numerator),
                Endianness::Big,
            )?;
            Ok(result)
        }
        ContainerFormat::Avi => {
            let strh_path = BoxPath::root()
                .child(BoxTag::new(*b"hdrl"))
                .child(BoxTag::new(*b"strl"))
                .child(BoxTag::new(*b"strh"));
            let box_info = find_avi_box(input, &strh_path)?;
            let mut result = FileBytes::copy_of(input);

            // strh payload layout: rate/numerator(4) = 24-28
            let rate_pos = box_info.payload_start.field_at(FieldOffset(24))?;
            let rate = frame_rate.ratio();
            let numerator = u32::try_from(rate.numer().0).map_err(DefectError::size_overflow)?;
            write_u32(
                &mut result,
                rate_pos,
                FieldValue(numerator),
                Endianness::Little,
            )?;
            Ok(result)
        }
        ContainerFormat::Neither => Err(DefectError::IncompatibleFileType),
    }
}

/// Apply `NonMonotonicTimestamps` defect.
pub(super) fn apply_non_monotonic_timestamps(
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    match detect_format(input)? {
        ContainerFormat::Iso => {
            let stts_path = BoxPath::root()
                .child(BoxTag::new(*b"moov"))
                .child(BoxTag::new(*b"trak"))
                .child(BoxTag::new(*b"mdia"))
                .child(BoxTag::new(*b"minf"))
                .child(BoxTag::new(*b"stbl"))
                .child(BoxTag::new(*b"stts"));
            let box_info = find_iso_box(input, &stts_path)?;
            let mut result = FileBytes::copy_of(input);

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
            let first_delta_pos = box_info.payload_start.field_at(FieldOffset(12))?;
            write_u32(&mut result, first_delta_pos, FieldValue(0), Endianness::Big)?;
            Ok(result)
        }
        ContainerFormat::Avi | ContainerFormat::Neither => Err(DefectError::IncompatibleFileType),
    }
}

/// Apply `DeclaredCountMismatch` defect.
pub(super) fn apply_declared_count_mismatch(
    declared: FieldValue,
    input: ContainerBytes<'_>,
) -> Result<FileBytes, DefectError> {
    match detect_format(input)? {
        ContainerFormat::Iso => {
            // Find stsz box: moov -> trak -> mdia -> minf -> stbl -> stsz
            let stsz_path = BoxPath::root()
                .child(BoxTag::new(*b"moov"))
                .child(BoxTag::new(*b"trak"))
                .child(BoxTag::new(*b"mdia"))
                .child(BoxTag::new(*b"minf"))
                .child(BoxTag::new(*b"stbl"))
                .child(BoxTag::new(*b"stsz"));
            let box_info = find_iso_box(input, &stsz_path)?;
            let mut result = FileBytes::copy_of(input);

            // stsz payload layout: version(1)+flags(3) = 0-4, sample_size(4) = 4-8,
            // sample_count(4) = 8-12
            let sample_count_pos = box_info.payload_start.field_at(FieldOffset(8))?;
            write_u32(&mut result, sample_count_pos, declared, Endianness::Big)?;
            Ok(result)
        }
        ContainerFormat::Avi => {
            let avih_path = BoxPath::root()
                .child(BoxTag::new(*b"hdrl"))
                .child(BoxTag::new(*b"avih"));
            let box_info = find_avi_box(input, &avih_path)?;
            let mut result = FileBytes::copy_of(input);

            let frame_count_pos = box_info.payload_start.field_at(FieldOffset(16))?;
            write_u32(&mut result, frame_count_pos, declared, Endianness::Little)?;
            Ok(result)
        }
        ContainerFormat::Neither => Err(DefectError::IncompatibleFileType),
    }
}
