//! AVI container writing for uncompressed RGB and motion-JPEG video.
use crate::avi_helpers::{
    negate_u32_nonzero, validate_frame_size, BufferEnd, BufferRange, BufferStart, ChunkId,
    ChunkOffset, ChunkSize, FpsDenominator, FpsNumerator, FrameCount, FrameTiming, IndexEntry,
};
use core::fmt;
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

impl core::error::Error for AviError {}
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

    /// The `movi` chunk tag for a stream-0 frame: `00dc` for a compressed
    /// frame, `00db` for an uncompressed DIB.
    const fn frame_chunk_tag(self) -> [u8; 4] {
        match self {
            Self::MotionJpeg => *b"00dc",
            Self::UncompressedRgb => *b"00db",
        }
    }

    /// The `movi` chunk id for a stream-0 frame.
    const fn frame_chunk_id(self) -> ChunkId {
        ChunkId::new(self.frame_chunk_tag())
    }

    /// The codec that gives a stream-0 frame the chunk tag `tag`.
    ///
    /// # Errors
    ///
    /// Returns [`NotFrameChunk`] when no codec uses the tag for a frame.
    pub(crate) fn of_frame_chunk(tag: [u8; 4]) -> Result<Self, NotFrameChunk> {
        Self::ALL
            .into_iter()
            .find(|codec| codec.frame_chunk_tag() == tag)
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
    frames: &[Vec<u8>],
    width: Width,
    height: Height,
    codec: AviCodec,
) -> Result<Vec<u8>, AviError> {
    if frames.is_empty() {
        return Err(AviError::NoFrames);
    }

    let frame_count = u32::try_from(frames.len()).map_err(|_| AviError::SizeOverflow)?;

    let rate = frame_rate.ratio();
    let fps_numerator = u32::try_from(rate.numer()).map_err(|_| AviError::SizeOverflow)?;
    let fps_denominator = u32::try_from(rate.denom().get()).map_err(|_| AviError::SizeOverflow)?;

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
    write_avih(&mut output, &timing, width, height)?;

    // Write strl LIST
    let strl_size_pos = output.len();
    output.extend_from_slice(b"LIST");
    output.extend_from_slice(&[0, 0, 0, 0]); // Placeholder for LIST size
    output.extend_from_slice(b"strl");

    // Write strh chunk
    write_strh(&mut output, &timing, codec);

    // Write strf chunk
    write_strf(&mut output, width, height, codec);

    // Update strl LIST size
    let strl_size = calculate_size(BufferRange::new(
        BufferStart::new(strl_size_pos),
        BufferEnd::new(output.len()),
    ))?;
    update_chunk_size(&mut output, strl_size_pos, strl_size);

    // Update hdrl LIST size
    let hdrl_size = calculate_size(BufferRange::new(
        BufferStart::new(hdrl_size_pos),
        BufferEnd::new(output.len()),
    ))?;
    update_chunk_size(&mut output, hdrl_size_pos, hdrl_size);

    // Write movi LIST
    let movi_size_pos = output.len();
    output.extend_from_slice(b"LIST");
    output.extend_from_slice(&[0, 0, 0, 0]); // Placeholder for LIST size

    // idx1 offsets count from the "movi" list type, so the first frame is at 4.
    let movi_data_pos = output.len();
    output.extend_from_slice(b"movi");

    let mut index_entries = Vec::new();

    // Write frame data
    for frame_data in frames {
        let chunk_id = codec.frame_chunk_id();

        // Validate frame data size
        validate_frame_size(frame_data, width, height, codec)?;

        // Record index entry: offset is relative to the movi list type
        let offset = u32::try_from(
            output
                .len()
                .checked_sub(movi_data_pos)
                .ok_or(AviError::SizeOverflow)?,
        )
        .map_err(|_| AviError::SizeOverflow)?;
        let frame_size = u32::try_from(frame_data.len()).map_err(|_| AviError::SizeOverflow)?;
        index_entries.push(IndexEntry::new(
            ChunkOffset::new(offset),
            ChunkSize::new(frame_size),
            chunk_id,
        ));

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
    }

    // Update movi LIST size
    let movi_size = calculate_size(BufferRange::new(
        BufferStart::new(movi_size_pos),
        BufferEnd::new(output.len()),
    ))?;
    update_chunk_size(&mut output, movi_size_pos, movi_size);
    write_idx1(&mut output, &index_entries)?;

    // Update RIFF size
    let riff_size = calculate_size(BufferRange::new(
        BufferStart::new(riff_size_pos),
        BufferEnd::new(output.len()),
    ))?;
    update_chunk_size(&mut output, riff_size_pos, riff_size);

    Ok(output)
}

/// Writes the idx1 (index) chunk for AVI.
fn write_idx1(output: &mut Vec<u8>, index_entries: &[IndexEntry]) -> Result<(), AviError> {
    let idx1_size_pos = output.len();
    output.extend_from_slice(b"idx1");
    output.extend_from_slice(&[0, 0, 0, 0]); // Placeholder for chunk size

    let idx1_data_pos = output.len();
    for entry in index_entries {
        output.extend_from_slice(&entry.chunk_id()[..]);
        output.extend_from_slice(&[0x10, 0x00, 0x00, 0x00]); // flags (AVIIF_KEYFRAME)
        output.extend_from_slice(&entry.offset().to_le_bytes());
        output.extend_from_slice(&entry.size().to_le_bytes());
    }

    let idx1_size = u32::try_from(
        output
            .len()
            .checked_sub(idx1_data_pos)
            .ok_or(AviError::SizeOverflow)?,
    )
    .map_err(|_| AviError::SizeOverflow)?;
    update_chunk_size(output, idx1_size_pos, idx1_size);

    Ok(())
}

/// Calculates a chunk size by subtracting the header size (8 bytes).
fn calculate_size(range: BufferRange) -> Result<u32, AviError> {
    let size = range
        .end()
        .checked_sub(range.start())
        .ok_or(AviError::SizeOverflow)?;
    let size = size.checked_sub(8).ok_or(AviError::SizeOverflow)?;
    u32::try_from(size).map_err(|_| AviError::SizeOverflow)
}

/// Writes the avih (AVI header) chunk.
fn write_avih(
    output: &mut Vec<u8>,
    timing: &FrameTiming,
    width: Width,
    height: Height,
) -> Result<(), AviError> {
    output.extend_from_slice(b"avih");
    let chunk_size = 56u32; // avih chunk is always 56 bytes
    output.extend_from_slice(&chunk_size.to_le_bytes());

    // Microseconds per frame
    // Total: fps_denominator is from a FrameRate which is guaranteed valid and positive.
    // Multiplying by 1_000_000 may overflow for very large denominators, but FrameRate
    // ensures this won't happen in practice (frame rates are reasonable values).
    let microseconds_per_frame = u32::try_from(
        (u64::from(timing.fps_denominator())
            .checked_mul(1_000_000)
            .ok_or(AviError::SizeOverflow)?)
        .checked_div(u64::from(timing.fps_numerator()))
        .ok_or(AviError::SizeOverflow)?,
    )
    .map_err(|_| AviError::SizeOverflow)?;
    output.extend_from_slice(&microseconds_per_frame.to_le_bytes());

    // Max bytes per second
    output.extend_from_slice(&[0, 0, 0, 0]); // Placeholder

    // Padding granularity
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Flags (AVIF_HASINDEX)
    output.extend_from_slice(&[0x10, 0x00, 0x00, 0x00]);

    // Number of frames
    output.extend_from_slice(&timing.frame_count().to_le_bytes());

    // Number of initial frames
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Number of streams
    output.extend_from_slice(&1u32.to_le_bytes());

    // Suggested buffer size
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Width
    let width_u32 = u32::from(width.get().get());
    output.extend_from_slice(&width_u32.to_le_bytes());

    // Height
    let height_u32 = u32::from(height.get().get());
    output.extend_from_slice(&height_u32.to_le_bytes());

    // Reserved
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&[0, 0, 0, 0]);

    Ok(())
}

/// Writes the strh (stream header) chunk.
fn write_strh(output: &mut Vec<u8>, timing: &FrameTiming, codec: AviCodec) {
    output.extend_from_slice(b"strh");
    let chunk_size = 56u32; // strh chunk is always 56 bytes
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
    output.extend_from_slice(&timing.fps_denominator().to_le_bytes());

    // Rate (for video, this is the numerator)
    output.extend_from_slice(&timing.fps_numerator().to_le_bytes());

    // Start
    output.extend_from_slice(&[0, 0, 0, 0]);

    // Length (number of frames)
    output.extend_from_slice(&timing.frame_count().to_le_bytes());

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
fn write_strf(output: &mut Vec<u8>, width: Width, height: Height, codec: AviCodec) {
    output.extend_from_slice(b"strf");

    let width_u32 = u32::from(width.get().get());
    let height_val = u32::from(height.get().get());
    // AVI format stores height as i32 (negative for top-down). Height is guaranteed
    // to be 1..=65535 from NonZeroU16, so two's-complement negation is always safe.
    let neg_height_u32 = negate_u32_nonzero(height_val);

    match codec {
        AviCodec::UncompressedRgb => {
            // For uncompressed RGB, use BITMAPINFOHEADER
            let chunk_size = 40u32;
            output.extend_from_slice(&chunk_size.to_le_bytes());

            // Header size
            output.extend_from_slice(&(40u32).to_le_bytes());

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
            let chunk_size = 40u32;
            output.extend_from_slice(&chunk_size.to_le_bytes());

            // Header size
            output.extend_from_slice(&(40u32).to_le_bytes());

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
}

/// Updates a chunk size field at the given position.
fn update_chunk_size(output: &mut [u8], pos: usize, size: u32) {
    let size_bytes = size.to_le_bytes();
    for (i, byte) in size_bytes.iter().enumerate() {
        // Total: pos + 4 won't overflow as pos is a valid buffer offset and 4 is small.
        // i is from enumerate and won't cause overflow for the u32 bytes (max i = 3).
        if let Some(index) = pos.checked_add(4).and_then(|p| p.checked_add(i)) {
            if let Some(b) = output.get_mut(index) {
                *b = *byte;
            }
        }
    }
}
#[cfg(test)]
#[path = "avi_tests.rs"]
mod tests;
