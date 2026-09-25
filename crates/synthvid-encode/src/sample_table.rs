//! Sample table box writing for ISO base media containers.

use crate::iso::{IsoCodec, IsoError};
use crate::iso_helpers::{write_box, write_u16_be, write_u32_be};
use synthvid_scene::{FrameRate, Height, Width};

/// Information about stco chunk offsets for later patching.
///
/// Contains the buffer positions where chunk offsets are written, which can be
/// patched later with the actual byte offsets in the file.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StcoOffsetInfo {
    /// Starting position in buffer of the first offset entry.
    pub(crate) first_offset_pos: usize,
}

/// Writes the stbl box (sample table).
pub(crate) fn write_stbl(
    buffer: &mut Vec<u8>,
    frames: &[Vec<u8>],
    width: Width,
    height: Height,
    codec: IsoCodec,
    media_rate: FrameRate,
) -> Result<StcoOffsetInfo, IsoError> {
    write_box(
        buffer,
        *b"stbl",
        |buffer| -> Result<StcoOffsetInfo, IsoError> {
            // Write stsd (sample description)
            write_stsd(buffer, width, height, codec)?;

            // Write stts (decoding time to sample)
            let frame_count = u32::try_from(frames.len()).map_err(|_| IsoError::SizeOverflow)?;
            write_stts(buffer, frame_count, media_rate)?;

            // Write stsc (sample to chunk)
            write_stsc(buffer)?;

            // Write stsz (sample sizes)
            write_stsz(buffer, frames)?;

            // Write stco (chunk offsets) and return offset info for later patching
            write_stco(buffer, frames.len())
        },
    )
}

/// Writes the stsd box (sample description).
fn write_stsd(
    buffer: &mut Vec<u8>,
    width: Width,
    height: Height,
    codec: IsoCodec,
) -> Result<(), IsoError> {
    write_box(buffer, *b"stsd", |buffer| {
        buffer.push(0); // Version 0
        buffer.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(buffer, 1); // Entry count

        // Write sample entry
        match codec {
            IsoCodec::UncompressedRgb => write_rgb_sample_entry(buffer, width, height)?,
            IsoCodec::MotionJpeg => write_mjpeg_sample_entry(buffer, width, height)?,
        }

        Ok(())
    })
}

/// Writes an uncompressed RGB sample entry.
fn write_rgb_sample_entry(
    buffer: &mut Vec<u8>,
    width: Width,
    height: Height,
) -> Result<(), IsoError> {
    write_box(buffer, *b"raw ", |buffer| {
        buffer.extend_from_slice(&[0; 6]); // Reserved
        write_u16_be(buffer, 1); // Data reference index

        // VisualSampleEntry fixed fields per ISO/IEC 14496-12 §8.5.2.1
        write_u16_be(buffer, 0); // pre_defined
        write_u16_be(buffer, 0); // reserved
        write_u32_be(buffer, 0); // pre_defined[0]
        write_u32_be(buffer, 0); // pre_defined[1]
        write_u32_be(buffer, 0); // pre_defined[2]

        write_u16_be(buffer, width.get().get());
        write_u16_be(buffer, height.get().get());
        write_u32_be(buffer, 0x0048_0000); // Horizontal resolution (72 dpi in 16.16)
        write_u32_be(buffer, 0x0048_0000); // Vertical resolution (72 dpi in 16.16)
        write_u32_be(buffer, 0); // Reserved (data_size)
        write_u16_be(buffer, 1); // Frame count

        // Compressor name: 32-byte Pascal string (1-byte length + up to 31 bytes + padding)
        buffer.extend_from_slice(b"\x00"); // Length (0, empty string)
        buffer.extend_from_slice(&[0; 31]); // Padding to 32 bytes total

        write_u16_be(buffer, 24); // Depth (24-bit RGB)
        write_u16_be(buffer, 0xffff); // Pre-defined (-1, no color table)

        Ok(())
    })
}

/// Writes a motion-JPEG sample entry.
fn write_mjpeg_sample_entry(
    buffer: &mut Vec<u8>,
    width: Width,
    height: Height,
) -> Result<(), IsoError> {
    write_box(buffer, *b"mjpg", |buffer| {
        buffer.extend_from_slice(&[0; 6]); // Reserved
        write_u16_be(buffer, 1); // Data reference index

        // VisualSampleEntry fixed fields per ISO/IEC 14496-12 §8.5.2.1
        write_u16_be(buffer, 0); // pre_defined
        write_u16_be(buffer, 0); // reserved
        write_u32_be(buffer, 0); // pre_defined[0]
        write_u32_be(buffer, 0); // pre_defined[1]
        write_u32_be(buffer, 0); // pre_defined[2]

        write_u16_be(buffer, width.get().get());
        write_u16_be(buffer, height.get().get());
        write_u32_be(buffer, 0x0048_0000); // Horizontal resolution (72 dpi in 16.16)
        write_u32_be(buffer, 0x0048_0000); // Vertical resolution (72 dpi in 16.16)
        write_u32_be(buffer, 0); // Reserved (data_size)
        write_u16_be(buffer, 1); // Frame count

        // Compressor name: 32-byte Pascal string (1-byte length + up to 31 bytes + padding)
        buffer.extend_from_slice(b"\x00"); // Length (0, empty string)
        buffer.extend_from_slice(&[0; 31]); // Padding to 32 bytes total

        write_u16_be(buffer, 24); // Depth (24-bit)
        write_u16_be(buffer, 0xffff); // Pre-defined (-1, no color table)

        Ok(())
    })
}

/// Writes the stts box (decoding time to sample).
///
/// Uses the frame rate's denominator as the `sample_delta`, ensuring exact frame rate
/// representation in playback (timescale / `sample_delta` = numer / denom = exact rate).
/// The `media_rate` parameter is the media's native rate: either the capture rate
/// (when a distinct one is provided) or the playback rate (when no distinct capture rate).
fn write_stts(
    buffer: &mut Vec<u8>,
    frame_count: u32,
    media_rate: FrameRate,
) -> Result<(), IsoError> {
    write_box(buffer, *b"stts", |buffer| -> Result<(), IsoError> {
        buffer.push(0); // Version 0
        buffer.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(buffer, 1); // Entry count
        write_u32_be(buffer, frame_count); // Sample count

        // Sample delta is media rate denominator; combined with mdhd timescale
        // (which is the numerator), this gives exact media frame rate: timescale / sample_delta
        let sample_delta =
            u32::try_from(media_rate.ratio().denom().get()).map_err(|_| IsoError::SizeOverflow)?;
        write_u32_be(buffer, sample_delta);

        Ok(())
    })
}

/// Writes the stsc box (sample to chunk).
///
/// One sample per chunk: `stco` (below) writes one chunk-offset entry per
/// frame, so this must declare `samples_per_chunk = 1`, not `frame_count`
/// -- declaring "1 chunk of `frame_count` samples" while `stco` supplies
/// `frame_count` separate chunk offsets is exactly the kind of
/// declared-vs-actual mismatch this file has already been bitten by
/// several times; the two tables must agree on the same chunk layout.
fn write_stsc(buffer: &mut Vec<u8>) -> Result<(), IsoError> {
    write_box(buffer, *b"stsc", |buffer| {
        buffer.push(0); // Version 0
        buffer.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(buffer, 1); // Entry count
        write_u32_be(buffer, 1); // First chunk (1-based)
        write_u32_be(buffer, 1); // Samples per chunk
        write_u32_be(buffer, 1); // Sample description ID
        Ok(())
    })
}

/// Writes the stsz box (sample sizes).
fn write_stsz(buffer: &mut Vec<u8>, frames: &[Vec<u8>]) -> Result<(), IsoError> {
    write_box(buffer, *b"stsz", |buffer| -> Result<(), IsoError> {
        buffer.push(0); // Version 0
        buffer.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(buffer, 0); // Sample size (0 = variable)
        let frame_count = u32::try_from(frames.len()).map_err(|_| IsoError::SizeOverflow)?;
        write_u32_be(buffer, frame_count); // Sample count

        for frame in frames {
            let frame_size = u32::try_from(frame.len()).map_err(|_| IsoError::SizeOverflow)?;
            write_u32_be(buffer, frame_size);
        }

        Ok(())
    })
}

/// Writes the stco box (chunk offsets).
///
/// Records the buffer positions where chunk offsets are written (as placeholders),
/// which are patched later with actual byte offsets once mdat position is known.
/// Returns a `StcoOffsetInfo` containing the buffer positions for later patching.
fn write_stco(buffer: &mut Vec<u8>, frame_count: usize) -> Result<StcoOffsetInfo, IsoError> {
    write_box(
        buffer,
        *b"stco",
        |buffer| -> Result<StcoOffsetInfo, IsoError> {
            buffer.push(0); // Version 0
            buffer.extend_from_slice(&[0, 0, 0]); // Flags
            let frame_count_u32 = u32::try_from(frame_count).map_err(|_| IsoError::SizeOverflow)?;
            write_u32_be(buffer, frame_count_u32); // Entry count

            // Record the position where offsets start so we can patch them later
            let first_offset_pos = buffer.len();

            // Write placeholder offsets (0 for now, will be patched after mdat is written)
            for _ in 0..frame_count {
                write_u32_be(buffer, 0); // Placeholder offset
            }

            Ok(StcoOffsetInfo { first_offset_pos })
        },
    )
}
