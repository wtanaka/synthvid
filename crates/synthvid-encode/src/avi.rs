//! AVI container writing for uncompressed RGB and motion-JPEG video.
use crate::avi_helpers::{
    top_down_bitmap_height, validate_frame_size, BufferEnd, BufferRange, BufferStart, ChunkId,
    ChunkOffset, ChunkSize, FpsDenominator, FpsNumerator, FrameCount, FrameTiming, IndexEntry,
};
use crate::byte_views::{ByteSink, FramePayload};
use crate::media::{ContainerFile, EncodedFrame};
use core::error::Error;
use core::fmt;
use core::num::NonZeroU32;
use core::num::TryFromIntError;
use synthvid_scene::{FrameRate, Height, Width};

/// Error type for AVI encoding.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AviError {
    /// Unsupported frame dimensions.
    InvalidDimensions,
    /// No frames provided.
    NoFrames,
    /// Frame data size overflow.
    SizeOverflow,
}

impl fmt::Display for AviError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions => write!(f, "invalid frame dimensions"),
            Self::NoFrames => write!(f, "no frames provided"),
            Self::SizeOverflow => write!(f, "frame data size overflow"),
        }
    }
}

impl Error for AviError {}

impl AviError {
    /// A length, count or rate too large for its field is a size overflow; the
    /// conversion error says nothing more than that.
    pub(crate) const fn size_overflow(_: TryFromIntError) -> Self {
        Self::SizeOverflow
    }
}
/// Codec types.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AviCodec {
    /// Uncompressed RGB.
    UncompressedRgb,
    /// Motion-JPEG.
    MotionJpeg,
}

/// A chunk tag that no codec gives a stream-0 frame.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct NotFrameChunk;

impl AviCodec {
    /// Every codec.
    const ALL: [Self; 2] = [Self::MotionJpeg, Self::UncompressedRgb];

    /// The `movi` chunk id for a stream-0 frame: `00dc` for a compressed
    /// frame, `00db` for an uncompressed DIB.
    const fn frame_chunk_id(self) -> ChunkId {
        match self {
            Self::MotionJpeg => ChunkId::new(*b"00dc"),
            Self::UncompressedRgb => ChunkId::new(*b"00db"),
        }
    }

    /// The codec that gives a stream-0 frame the chunk id `tag`.
    ///
    /// # Errors
    ///
    /// Returns [`NotFrameChunk`] when no codec uses the tag for a frame.
    pub(crate) fn of_frame_chunk(tag: ChunkId) -> Result<Self, NotFrameChunk> {
        Self::ALL
            .into_iter()
            .find(|codec| codec.frame_chunk_id() == tag)
            .ok_or(NotFrameChunk)
    }
}

/// The boundary RIFF pads every chunk to: an odd-length chunk is followed by
/// exactly one zero byte, which its size field does not count.
pub(crate) const RIFF_CHUNK_ALIGNMENT: usize = 2;

/// Encodes frames as an AVI container.
///
/// # Arguments
///
/// * `frame_rate` - Frame rate as an exact rational number
/// * `frames` - Sequence of encoded frames
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
/// * `codec` - Codec type (uncompressed RGB or motion-JPEG)
///
/// # Errors
///
/// Returns `AviError` if encoding fails.
pub fn encode_avi(
    frame_rate: FrameRate,
    frames: &[EncodedFrame],
    width: Width,
    height: Height,
    codec: AviCodec,
) -> Result<ContainerFile, AviError> {
    if frames.is_empty() {
        return Err(AviError::NoFrames);
    }

    let frame_count = u32::try_from(frames.len()).map_err(AviError::size_overflow)?;

    let rate = frame_rate.ratio();
    let fps_numerator = u32::try_from(rate.numer().0).map_err(AviError::size_overflow)?;
    let fps_denominator = u32::try_from(rate.denom().get()).map_err(AviError::size_overflow)?;

    let timing = FrameTiming::new(
        FrameCount::new(frame_count),
        FpsNumerator::new(fps_numerator),
        FpsDenominator::new(fps_denominator),
    );
    let mut output = Vec::new();
    // We'll write the RIFF header size later
    let riff_size_pos = output.len();
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&[0, 0, 0, 0]); // Placeholder for RIFF size

    output.extend_from_slice(b"AVI ");

    // Write hdrl LIST
    let hdrl_size_pos = output.len();
    output.extend_from_slice(b"LIST");
    output.extend_from_slice(&[0, 0, 0, 0]); // Placeholder for LIST size
    output.extend_from_slice(b"hdrl");

    // Write avih chunk
    write_avih(&mut ByteSink(&mut output), &timing, width, height)?;

    // Write strl LIST
    let strl_size_pos = output.len();
    output.extend_from_slice(b"LIST");
    output.extend_from_slice(&[0, 0, 0, 0]); // Placeholder for LIST size
    output.extend_from_slice(b"strl");

    // Write strh chunk
    write_strh(&mut ByteSink(&mut output), &timing, codec);

    // Write strf chunk
    write_strf(&mut ByteSink(&mut output), width, height, codec)?;

    // Update strl LIST size
    close_chunk(&mut ByteSink(&mut output), BufferStart::new(strl_size_pos))?;

    // Update hdrl LIST size
    close_chunk(&mut ByteSink(&mut output), BufferStart::new(hdrl_size_pos))?;

    // Write movi LIST
    let movi_size_pos = output.len();
    output.extend_from_slice(b"LIST");
    output.extend_from_slice(&[0, 0, 0, 0]); // Placeholder for LIST size

    // idx1 offsets count from the "movi" list type, so the first frame is at 4.
    let movi_data_pos = output.len();
    output.extend_from_slice(b"movi");

    let mut index_entries = Vec::new();

    // Write frame data
    for EncodedFrame(frame_data) in frames {
        let payload = FramePayload(frame_data.as_slice());
        validate_frame_size(payload, width, height, codec)?;
        index_entries.push(write_frame_chunk(
            &mut ByteSink(&mut output),
            BufferStart::new(movi_data_pos),
            payload,
            codec.frame_chunk_id(),
        )?);
    }

    // Update movi LIST size
    close_chunk(&mut ByteSink(&mut output), BufferStart::new(movi_size_pos))?;
    write_idx1(&mut ByteSink(&mut output), &index_entries)?;

    // Update RIFF size
    close_chunk(&mut ByteSink(&mut output), BufferStart::new(riff_size_pos))?;

    Ok(ContainerFile(output))
}

/// Writes one frame as a `movi` chunk, padded to RIFF alignment, and
/// returns its `idx1` entry, whose offset counts from the `movi` list type
/// at `movi_data_pos`.
fn write_frame_chunk(
    sink: &mut ByteSink<'_>,
    movi_data_pos: BufferStart,
    payload: FramePayload<'_>,
    chunk_id: ChunkId,
) -> Result<IndexEntry, AviError> {
    let ByteSink(output) = sink;
    let FramePayload(frame_data) = payload;

    // Record index entry: offset is relative to the movi list type
    let offset = u32::try_from(
        output
            .len()
            .checked_sub(movi_data_pos.get())
            .ok_or(AviError::SizeOverflow)?,
    )
    .map_err(AviError::size_overflow)?;
    let frame_size = u32::try_from(frame_data.len()).map_err(AviError::size_overflow)?;

    // Write chunk header
    output.extend_from_slice(&chunk_id.get());
    output.extend_from_slice(&frame_size.to_le_bytes());

    // Write frame data
    output.extend_from_slice(frame_data);

    // Pad an odd-length chunk with one byte, as RIFF requires.
    let padded_len = frame_data
        .len()
        .checked_next_multiple_of(RIFF_CHUNK_ALIGNMENT)
        .ok_or(AviError::SizeOverflow)?;
    let padding_bytes = padded_len
        .checked_sub(frame_data.len())
        .ok_or(AviError::SizeOverflow)?;
    output.resize(
        output
            .len()
            .checked_add(padding_bytes)
            .ok_or(AviError::SizeOverflow)?,
        0,
    );

    Ok(IndexEntry::new(
        ChunkOffset::new(offset),
        ChunkSize::new(frame_size),
        chunk_id,
    ))
}

/// Writes the idx1 (index) chunk for AVI.
fn write_idx1(sink: &mut ByteSink<'_>, index_entries: &[IndexEntry]) -> Result<(), AviError> {
    let idx1_size_pos = sink.0.len();
    sink.0.extend_from_slice(b"idx1");
    sink.0.extend_from_slice(&[0, 0, 0, 0]); // Placeholder for chunk size

    let idx1_data_pos = sink.0.len();
    for entry in index_entries {
        sink.0.extend_from_slice(&entry.chunk_id().get()[..]);
        sink.0.extend_from_slice(&[0x10, 0x00, 0x00, 0x00]); // flags (AVIIF_KEYFRAME)
        sink.0
            .extend_from_slice(&entry.offset().get().to_le_bytes());
        sink.0.extend_from_slice(&entry.size().get().to_le_bytes());
    }

    let idx1_size = u32::try_from(
        sink.0
            .len()
            .checked_sub(idx1_data_pos)
            .ok_or(AviError::SizeOverflow)?,
    )
    .map_err(AviError::size_overflow)?;
    update_chunk_size(
        sink,
        BufferStart::new(idx1_size_pos),
        ChunkSize::new(idx1_size),
    );

    Ok(())
}

/// Writes the size field of the LIST or RIFF chunk starting at `start`, so that
/// it counts every byte from its header to the end of the buffer.
fn close_chunk(sink: &mut ByteSink<'_>, start: BufferStart) -> Result<(), AviError> {
    let size = calculate_size(BufferRange::new(start, BufferEnd::new(sink.0.len())))?;
    update_chunk_size(sink, start, size);
    Ok(())
}

/// Calculates a chunk size by subtracting the header size (8 bytes).
fn calculate_size(range: BufferRange) -> Result<ChunkSize, AviError> {
    let span = range
        .end()
        .get()
        .checked_sub(range.start().get())
        .ok_or(AviError::SizeOverflow)?;
    let size = span.checked_sub(8).ok_or(AviError::SizeOverflow)?;
    u32::try_from(size)
        .map(ChunkSize::new)
        .map_err(AviError::size_overflow)
}

/// Writes the avih (AVI header) chunk.
fn write_avih(
    sink: &mut ByteSink<'_>,
    timing: &FrameTiming,
    width: Width,
    height: Height,
) -> Result<(), AviError> {
    let output = &mut *sink.0;
    output.extend_from_slice(b"avih");
    let chunk_size = 56_u32; // avih chunk is always 56 bytes
    output.extend_from_slice(&chunk_size.to_le_bytes());

    // Microseconds per frame
    // Total: fps_denominator is from a FrameRate which is guaranteed valid and positive.
    // Multiplying by 1_000_000 may overflow for very large denominators, but FrameRate
    // ensures this won't happen in practice (frame rates are reasonable values).
    let microseconds_per_frame = u32::try_from(
        (u64::from(timing.fps_denominator().get())
            .checked_mul(1_000_000)
            .ok_or(AviError::SizeOverflow)?)
        .checked_div(u64::from(timing.fps_numerator().get()))
        .ok_or(AviError::SizeOverflow)?,
    )
    .map_err(AviError::size_overflow)?;
    output.extend_from_slice(&microseconds_per_frame.to_le_bytes());

    // Max bytes per second
    output.extend_from_slice(&[0, 0, 0, 0]); // Placeholder

    // Padding granularity
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Flags (AVIF_HASINDEX)
    output.extend_from_slice(&[0x10, 0x00, 0x00, 0x00]);

    // Number of frames
    output.extend_from_slice(&timing.frame_count().get().to_le_bytes());

    // Number of initial frames
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Number of streams
    output.extend_from_slice(&1_u32.to_le_bytes());

    // Suggested buffer size
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Width
    let width_u32 = NonZeroU32::from(width.get()).get();
    output.extend_from_slice(&width_u32.to_le_bytes());

    // Height
    let height_u32 = NonZeroU32::from(height.get()).get();
    output.extend_from_slice(&height_u32.to_le_bytes());

    // Reserved
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&[0, 0, 0, 0]);

    Ok(())
}

/// Writes the strh (stream header) chunk.
fn write_strh(sink: &mut ByteSink<'_>, timing: &FrameTiming, codec: AviCodec) {
    let output = &mut *sink.0;
    output.extend_from_slice(b"strh");
    let chunk_size = 56_u32; // strh chunk is always 56 bytes
    output.extend_from_slice(&chunk_size.to_le_bytes());

    // FourCC type (vids for video)
    output.extend_from_slice(b"vids");

    // Codec
    match codec {
        AviCodec::UncompressedRgb => output.extend_from_slice(b"DIB "),
        AviCodec::MotionJpeg => output.extend_from_slice(b"MJPG"),
    }

    // Flags
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Priority (wPriority, u16) and Language (wLanguage, u16) packed in one 4-byte field
    // Total: AVISTREAMHEADER spec requires these two u16 fields packed together.
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Initial frames
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Scale (for video, this is the denominator)
    output.extend_from_slice(&timing.fps_denominator().get().to_le_bytes());

    // Rate (for video, this is the numerator)
    output.extend_from_slice(&timing.fps_numerator().get().to_le_bytes());

    // Start
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Length (number of frames)
    output.extend_from_slice(&timing.frame_count().get().to_le_bytes());

    // Suggested buffer size
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Quality
    output.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);

    // Sample size
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Frame (left, top, right, bottom) - for video, this is zero (rcFrame, 8 bytes)
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&[0, 0, 0, 0]);
}

/// Writes the strf (stream format) chunk for video.
fn write_strf(
    sink: &mut ByteSink<'_>,
    width: Width,
    height: Height,
    codec: AviCodec,
) -> Result<(), AviError> {
    let output = &mut *sink.0;
    output.extend_from_slice(b"strf");

    let width_u32 = NonZeroU32::from(width.get()).get();
    // AVI format stores height as i32 (negative for top-down). Height is guaranteed
    // to be 1..=65535 from NonZeroU16, so two's-complement negation is always safe.
    let neg_height_u32 = top_down_bitmap_height(height)?.0;

    match codec {
        AviCodec::UncompressedRgb => {
            // For uncompressed RGB, use BITMAPINFOHEADER
            let chunk_size = 40_u32;
            output.extend_from_slice(&chunk_size.to_le_bytes());

            // Header size
            output.extend_from_slice(&(40_u32).to_le_bytes());

            // Width
            output.extend_from_slice(&width_u32.to_le_bytes());

            // Height (negative for top-down bitmap)
            output.extend_from_slice(&neg_height_u32.to_le_bytes());

            // Planes
            output.extend_from_slice(&[1, 0]);

            // Bits per pixel (24 for RGB)
            output.extend_from_slice(&[24, 0]);

            // Compression (0 for uncompressed)
            output.extend_from_slice(&[0, 0, 0, 0]);

            // Image size (can be 0 for uncompressed)
            output.extend_from_slice(&[0, 0, 0, 0]);

            // X pixels per meter
            output.extend_from_slice(&[0, 0, 0, 0]);

            // Y pixels per meter
            output.extend_from_slice(&[0, 0, 0, 0]);

            // Color indices used
            output.extend_from_slice(&[0, 0, 0, 0]);

            // Color indices important
            output.extend_from_slice(&[0, 0, 0, 0]);
        }
        AviCodec::MotionJpeg => {
            // For motion-JPEG, use BITMAPINFOHEADER with MJPG fourcc
            let chunk_size = 40_u32;
            output.extend_from_slice(&chunk_size.to_le_bytes());

            // Header size
            output.extend_from_slice(&(40_u32).to_le_bytes());

            // Width
            output.extend_from_slice(&width_u32.to_le_bytes());

            // Height (negative for top-down bitmap)
            output.extend_from_slice(&neg_height_u32.to_le_bytes());

            // Planes
            output.extend_from_slice(&[1, 0]);

            // Bits per pixel (24 for MJPG)
            output.extend_from_slice(&[24, 0]);

            // Compression (MJPG)
            output.extend_from_slice(b"MJPG");

            // Image size (can be 0)
            output.extend_from_slice(&[0, 0, 0, 0]);

            // X pixels per meter
            output.extend_from_slice(&[0, 0, 0, 0]);

            // Y pixels per meter
            output.extend_from_slice(&[0, 0, 0, 0]);

            // Color indices used
            output.extend_from_slice(&[0, 0, 0, 0]);

            // Color indices important
            output.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
    Ok(())
}

/// Updates a chunk size field at the chunk starting at `start`.
fn update_chunk_size(sink: &mut ByteSink<'_>, start: BufferStart, size: ChunkSize) {
    let output = &mut *sink.0;
    let size_bytes = size.get().to_le_bytes();
    let start_offset = start.get();
    for (position, byte) in size_bytes.iter().enumerate() {
        // Total: start_offset + 4 won't overflow as start_offset is a valid buffer offset and 4 is small.
        // position is from enumerate and won't cause overflow for the u32 bytes (max position = 3).
        if let Some(index) = start_offset
            .checked_add(4)
            .and_then(|field_start| field_start.checked_add(position))
        {
            if let Some(slot) = output.get_mut(index) {
                *slot = *byte;
            }
        }
    }
}
#[cfg(test)]
#[path = "avi_tests.rs"]
mod tests;
