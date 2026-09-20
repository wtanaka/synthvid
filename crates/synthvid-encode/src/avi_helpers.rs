//! AVI encoding helpers.
use crate::avi::{AviCodec, AviError};
use synthvid_scene::{Height, Width};

/// Frame count.
#[derive(Copy, Clone)]
pub(crate) struct FrameCount(u32);

impl FrameCount {
    /// Creates a frame count.
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Gets the frame count value.
    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

/// FPS numerator.
#[derive(Copy, Clone)]
pub(crate) struct FpsNumerator(u32);

impl FpsNumerator {
    /// Creates an FPS numerator.
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Gets the numerator value.
    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

/// FPS denominator.
#[derive(Copy, Clone)]
pub(crate) struct FpsDenominator(u32);

impl FpsDenominator {
    /// Creates an FPS denominator.
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Gets the denominator value.
    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

/// Frame timing.
pub(crate) struct FrameTiming {
    /// Frame count.
    frame_count: FrameCount,
    /// FPS numerator.
    fps_numerator: FpsNumerator,
    /// FPS denominator.
    fps_denominator: FpsDenominator,
}

impl FrameTiming {
    /// Creates new frame timing.
    pub(crate) const fn new(
        frame_count: FrameCount,
        fps_numerator: FpsNumerator,
        fps_denominator: FpsDenominator,
    ) -> Self {
        Self {
            frame_count,
            fps_numerator,
            fps_denominator,
        }
    }

    /// Frame count.
    pub(crate) const fn frame_count(&self) -> u32 {
        self.frame_count.get()
    }

    /// FPS numerator.
    pub(crate) const fn fps_numerator(&self) -> u32 {
        self.fps_numerator.get()
    }

    /// FPS denominator.
    pub(crate) const fn fps_denominator(&self) -> u32 {
        self.fps_denominator.get()
    }
}

/// Buffer start offset.
#[derive(Copy, Clone)]
pub(crate) struct BufferStart(usize);

impl BufferStart {
    /// Creates a buffer start offset.
    pub(crate) const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Gets the offset value.
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Buffer end offset.
#[derive(Copy, Clone)]
pub(crate) struct BufferEnd(usize);

impl BufferEnd {
    /// Creates a buffer end offset.
    pub(crate) const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Gets the offset value.
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Buffer range for chunk sizes.
#[derive(Copy, Clone)]
pub(crate) struct BufferRange {
    /// Start offset.
    start: BufferStart,
    /// End offset.
    end: BufferEnd,
}

impl BufferRange {
    /// Creates a new buffer range.
    pub(crate) const fn new(start: BufferStart, end: BufferEnd) -> Self {
        Self { start, end }
    }

    /// Start offset.
    pub(crate) const fn start(&self) -> usize {
        self.start.get()
    }

    /// End offset.
    pub(crate) const fn end(&self) -> usize {
        self.end.get()
    }
}

/// Chunk offset.
#[derive(Copy, Clone)]
pub(crate) struct ChunkOffset(u32);

impl ChunkOffset {
    /// Creates a chunk offset.
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Gets the offset value.
    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

/// Chunk size.
#[derive(Copy, Clone)]
pub(crate) struct ChunkSize(u32);

impl ChunkSize {
    /// Creates a chunk size.
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Gets the size value.
    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

/// Chunk identifier.
#[derive(Copy, Clone)]
pub(crate) struct ChunkId([u8; 4]);

impl ChunkId {
    /// Creates a chunk identifier.
    pub(crate) const fn new(value: [u8; 4]) -> Self {
        Self(value)
    }

    /// Gets the chunk ID bytes.
    pub(crate) const fn get(self) -> [u8; 4] {
        self.0
    }
}

/// Index entry for idx1 chunk.
pub(crate) struct IndexEntry {
    /// Chunk offset.
    offset: ChunkOffset,
    /// Chunk size.
    size: ChunkSize,
    /// Chunk identifier.
    chunk_id: ChunkId,
}

impl IndexEntry {
    /// Creates a new index entry.
    pub(crate) const fn new(offset: ChunkOffset, size: ChunkSize, chunk_id: ChunkId) -> Self {
        Self {
            offset,
            size,
            chunk_id,
        }
    }

    /// Chunk offset.
    pub(crate) const fn offset(&self) -> u32 {
        self.offset.get()
    }

    /// Chunk size.
    pub(crate) const fn size(&self) -> u32 {
        self.size.get()
    }

    /// Chunk identifier.
    pub(crate) const fn chunk_id(&self) -> [u8; 4] {
        self.chunk_id.get()
    }
}

/// Validates frame size.
pub(crate) fn validate_frame_size(
    frame_data: &[u8],
    width: Width,
    height: Height,
    codec: AviCodec,
) -> Result<(), AviError> {
    match codec {
        AviCodec::UncompressedRgb => {
            let w = u32::from(width.get().get());
            let h = u32::from(height.get().get());
            let expected = w
                .checked_mul(h)
                .and_then(|wh| wh.checked_mul(3))
                .ok_or(AviError::SizeOverflow)?;
            let expected_usize = usize::try_from(expected).map_err(|_| AviError::SizeOverflow)?;
            if frame_data.len() != expected_usize {
                return Err(AviError::InvalidDimensions);
            }
        }
        AviCodec::MotionJpeg => {
            if frame_data.is_empty() {
                return Err(AviError::InvalidDimensions);
            }
        }
    }
    Ok(())
}

/// Negates a non-negative u32.
pub(crate) const fn negate_u32_nonzero(magnitude: u32) -> u32 {
    let inverted = !magnitude;
    match inverted.checked_add(1) {
        Some(neg) => neg,
        None => u32::MAX,
    }
}
