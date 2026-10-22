//! Sample table box writing for ISO base media containers.

use crate::avi_helpers::FrameCount;
use crate::byte_views::{ByteSink, FrameList};
use crate::iso::{IsoCodec, IsoError};
use crate::iso_helpers::{write_box, write_u16_be, write_u32_be};
use crate::iso_types::{BoxType, ByteOffset, U16Be, U32Be};
use crate::media::EncodedFrame;
use synthvid_scene::{Dimensions, FrameRate, Height, Width};

/// Information about stco chunk offsets for later patching.
///
/// Contains the buffer positions where chunk offsets are written, which can be
/// patched later with the actual byte offsets in the file.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StcoOffsetInfo {
    /// Starting position in buffer of the first offset entry.
    pub(crate) first_offset_pos: ByteOffset,
}

/// Writes the stbl box (sample table).
pub(crate) fn write_stbl(
    buffer: &mut ByteSink<'_>,
    frames: FrameList<'_>,
    dimensions: Dimensions,
    codec: IsoCodec,
    playback: FrameRate,
) -> Result<StcoOffsetInfo, IsoError> {
    let Dimensions { width, height } = dimensions;
    write_box(
        buffer,
        BoxType(*b"stbl"),
        |stbl_buffer| -> Result<StcoOffsetInfo, IsoError> {
            // Write stsd (sample description)
            write_stsd(stbl_buffer, width, height, codec)?;

            // Write stts (decoding time to sample)
            let frame_count = u32::try_from(frames.0.len()).map_err(IsoError::size_overflow)?;
            write_stts(stbl_buffer, FrameCount::new(frame_count), playback)?;

            // Write stsc (sample to chunk)
            write_stsc(stbl_buffer)?;

            // Write stsz (sample sizes)
            write_stsz(stbl_buffer, frames)?;

            // Write stco (chunk offsets) and return offset info for later patching
            write_stco(stbl_buffer, FrameCount::new(frame_count))
        },
    )
}

/// Writes the stsd box (sample description).
fn write_stsd(
    buffer: &mut ByteSink<'_>,
    width: Width,
    height: Height,
    codec: IsoCodec,
) -> Result<(), IsoError> {
    write_box(buffer, BoxType(*b"stsd"), |stsd_buffer| {
        stsd_buffer.0.push(0); // Version 0
        stsd_buffer.0.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(stsd_buffer, U32Be(1)); // Entry count

        // Write sample entry
        match codec {
            IsoCodec::UncompressedRgb => {
                write_rgb_sample_entry(stsd_buffer, width, height)?;
            }
            IsoCodec::MotionJpeg => write_mjpeg_sample_entry(stsd_buffer, width, height)?,
        }

        Ok(())
    })
}

/// Writes an uncompressed RGB sample entry.
fn write_rgb_sample_entry(
    buffer: &mut ByteSink<'_>,
    width: Width,
    height: Height,
) -> Result<(), IsoError> {
    write_box(buffer, BoxType(*b"raw "), |raw_buffer| {
        raw_buffer.0.extend_from_slice(&[0; 6]); // Reserved
        write_u16_be(raw_buffer, U16Be(1)); // Data reference index

        // VisualSampleEntry fixed fields per ISO/IEC 14496-12 §8.5.2.1
        write_u16_be(raw_buffer, U16Be(0)); // pre_defined
        write_u16_be(raw_buffer, U16Be(0)); // reserved
        write_u32_be(raw_buffer, U32Be(0)); // pre_defined[0]
        write_u32_be(raw_buffer, U32Be(0)); // pre_defined[1]
        write_u32_be(raw_buffer, U32Be(0)); // pre_defined[2]

        write_u16_be(raw_buffer, U16Be(width.get().get()));
        write_u16_be(raw_buffer, U16Be(height.get().get()));
        write_u32_be(raw_buffer, U32Be(0x0048_0000)); // Horizontal resolution (72 dpi in 16.16)
        write_u32_be(raw_buffer, U32Be(0x0048_0000)); // Vertical resolution (72 dpi in 16.16)
        write_u32_be(raw_buffer, U32Be(0)); // Reserved (data_size)
        write_u16_be(raw_buffer, U16Be(1)); // Frame count

        // Compressor name: 32-byte Pascal string (1-byte length + up to 31 bytes + padding)
        raw_buffer.0.extend_from_slice(b"\x00"); // Length (0, empty string)
        raw_buffer.0.extend_from_slice(&[0; 31]); // Padding to 32 bytes total

        write_u16_be(raw_buffer, U16Be(24)); // Depth (24-bit RGB)
        write_u16_be(raw_buffer, U16Be(0xffff)); // Pre-defined (-1, no color table)

        Ok(())
    })
}

/// Writes a motion-JPEG sample entry.
fn write_mjpeg_sample_entry(
    buffer: &mut ByteSink<'_>,
    width: Width,
    height: Height,
) -> Result<(), IsoError> {
    write_box(buffer, BoxType(*b"mjpg"), |mjpg_buffer| {
        mjpg_buffer.0.extend_from_slice(&[0; 6]); // Reserved
        write_u16_be(mjpg_buffer, U16Be(1)); // Data reference index

        // VisualSampleEntry fixed fields per ISO/IEC 14496-12 §8.5.2.1
        write_u16_be(mjpg_buffer, U16Be(0)); // pre_defined
        write_u16_be(mjpg_buffer, U16Be(0)); // reserved
        write_u32_be(mjpg_buffer, U32Be(0)); // pre_defined[0]
        write_u32_be(mjpg_buffer, U32Be(0)); // pre_defined[1]
        write_u32_be(mjpg_buffer, U32Be(0)); // pre_defined[2]

        write_u16_be(mjpg_buffer, U16Be(width.get().get()));
        write_u16_be(mjpg_buffer, U16Be(height.get().get()));
        write_u32_be(mjpg_buffer, U32Be(0x0048_0000)); // Horizontal resolution (72 dpi in 16.16)
        write_u32_be(mjpg_buffer, U32Be(0x0048_0000)); // Vertical resolution (72 dpi in 16.16)
        write_u32_be(mjpg_buffer, U32Be(0)); // Reserved (data_size)
        write_u16_be(mjpg_buffer, U16Be(1)); // Frame count

        // Compressor name: 32-byte Pascal string (1-byte length + up to 31 bytes + padding)
        mjpg_buffer.0.extend_from_slice(b"\x00"); // Length (0, empty string)
        mjpg_buffer.0.extend_from_slice(&[0; 31]); // Padding to 32 bytes total

        write_u16_be(mjpg_buffer, U16Be(24)); // Depth (24-bit)
        write_u16_be(mjpg_buffer, U16Be(0xffff)); // Pre-defined (-1, no color table)

        Ok(())
    })
}

/// Writes the stts box (decoding time to sample).
///
/// Uses the playback rate's denominator as the `sample_delta`, ensuring exact frame rate
/// representation in playback (timescale / `sample_delta` = numer / denom = exact rate).
fn write_stts(
    buffer: &mut ByteSink<'_>,
    frame_count: FrameCount,
    playback: FrameRate,
) -> Result<(), IsoError> {
    write_box(
        buffer,
        BoxType(*b"stts"),
        |stts_buffer| -> Result<(), IsoError> {
            stts_buffer.0.push(0); // Version 0
            stts_buffer.0.extend_from_slice(&[0, 0, 0]); // Flags
            write_u32_be(stts_buffer, U32Be(1)); // Entry count
            write_u32_be(stts_buffer, U32Be(frame_count.get())); // Sample count

            // Sample delta is the playback denominator; combined with the mdhd timescale
            // (the playback numerator), this gives the exact playback frame rate.
            let sample_delta =
                u32::try_from(playback.ratio().denom().get()).map_err(IsoError::size_overflow)?;
            write_u32_be(stts_buffer, U32Be(sample_delta));

            Ok(())
        },
    )
}

/// Writes the stsc box (sample to chunk).
///
/// One sample per chunk: `stco` (below) writes one chunk-offset entry per
/// frame, so this must declare `samples_per_chunk = 1`, not `frame_count`
/// -- declaring "1 chunk of `frame_count` samples" while `stco` supplies
/// `frame_count` separate chunk offsets is exactly the kind of
/// declared-vs-actual mismatch this file has already been bitten by
/// several times; the two tables must agree on the same chunk layout.
fn write_stsc(buffer: &mut ByteSink<'_>) -> Result<(), IsoError> {
    write_box(buffer, BoxType(*b"stsc"), |stsc_buffer| {
        stsc_buffer.0.push(0); // Version 0
        stsc_buffer.0.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(stsc_buffer, U32Be(1)); // Entry count
        write_u32_be(stsc_buffer, U32Be(1)); // First chunk (1-based)
        write_u32_be(stsc_buffer, U32Be(1)); // Samples per chunk
        write_u32_be(stsc_buffer, U32Be(1)); // Sample description ID
        Ok(())
    })
}

/// Writes the stsz box (sample sizes).
fn write_stsz(buffer: &mut ByteSink<'_>, frames: FrameList<'_>) -> Result<(), IsoError> {
    write_box(
        buffer,
        BoxType(*b"stsz"),
        |stsz_buffer| -> Result<(), IsoError> {
            stsz_buffer.0.push(0); // Version 0
            stsz_buffer.0.extend_from_slice(&[0, 0, 0]); // Flags
            write_u32_be(stsz_buffer, U32Be(0)); // Sample size (0 = variable)
            let frame_count = u32::try_from(frames.0.len()).map_err(IsoError::size_overflow)?;
            write_u32_be(stsz_buffer, U32Be(frame_count)); // Sample count

            for EncodedFrame(frame) in frames.0 {
                let frame_size = u32::try_from(frame.len()).map_err(IsoError::size_overflow)?;
                write_u32_be(stsz_buffer, U32Be(frame_size));
            }

            Ok(())
        },
    )
}

/// Writes the stco box (chunk offsets).
///
/// Records the buffer positions where chunk offsets are written (as placeholders),
/// which are patched later with actual byte offsets once mdat position is known.
/// Returns a `StcoOffsetInfo` containing the buffer positions for later patching.
fn write_stco(
    buffer: &mut ByteSink<'_>,
    frame_count: FrameCount,
) -> Result<StcoOffsetInfo, IsoError> {
    write_box(
        buffer,
        BoxType(*b"stco"),
        |stco_buffer| -> Result<StcoOffsetInfo, IsoError> {
            stco_buffer.0.push(0); // Version 0
            stco_buffer.0.extend_from_slice(&[0, 0, 0]); // Flags
            write_u32_be(stco_buffer, U32Be(frame_count.get())); // Entry count

            // Record the position where offsets start so we can patch them later
            let first_offset_pos = stco_buffer.0.len();

            // Write placeholder offsets (0 for now, will be patched after mdat is written)
            for _ in 0..frame_count.get() {
                write_u32_be(stco_buffer, U32Be(0)); // Placeholder offset
            }

            Ok(StcoOffsetInfo {
                first_offset_pos: ByteOffset(first_offset_pos),
            })
        },
    )
}
