//! AVI encoding helpers.
use crate::avi::{AviCodec, AviError};
use crate::byte_views::FramePayload;
use core::num::NonZeroU32;
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
    pub(crate) const fn frame_count(&self) -> FrameCount {
        self.frame_count
    }

    /// FPS numerator.
    pub(crate) const fn fps_numerator(&self) -> FpsNumerator {
        self.fps_numerator
    }

    /// FPS denominator.
    pub(crate) const fn fps_denominator(&self) -> FpsDenominator {
        self.fps_denominator
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
    pub(crate) const fn start(&self) -> BufferStart {
        self.start
    }

    /// End offset.
    pub(crate) const fn end(&self) -> BufferEnd {
        self.end
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
#[derive(Copy, Clone, PartialEq, Eq)]
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
    pub(crate) const fn offset(&self) -> ChunkOffset {
        self.offset
    }

    /// Chunk size.
    pub(crate) const fn size(&self) -> ChunkSize {
        self.size
    }

    /// Chunk identifier.
    pub(crate) const fn chunk_id(&self) -> ChunkId {
        self.chunk_id
    }
}

/// Validates frame size.
pub(crate) fn validate_frame_size(
    frame_data: FramePayload<'_>,
    width: Width,
    height: Height,
    codec: AviCodec,
) -> Result<(), AviError> {
    match codec {
        AviCodec::UncompressedRgb => {
            let width_px = NonZeroU32::from(width.get()).get();
            let height_px = NonZeroU32::from(height.get()).get();
            let expected = width_px
                .checked_mul(height_px)
                .and_then(|wh| wh.checked_mul(3))
                .ok_or(AviError::SizeOverflow)?;
            let expected_usize = usize::try_from(expected).map_err(AviError::size_overflow)?;
            if frame_data.0.len() != expected_usize {
                return Err(AviError::InvalidDimensions);
            }
        }
        AviCodec::MotionJpeg => {
            if frame_data.0.is_empty() {
                return Err(AviError::InvalidDimensions);
            }
        }
    }
    Ok(())
}

/// The height field of a top-down bitmap header, which holds the negated height.
#[derive(Copy, Clone)]
pub(crate) struct BitmapHeight(pub(crate) u32);

/// Negates a height into the two's-complement field a top-down bitmap uses.
///
/// # Errors
///
/// Returns [`AviError::InvalidDimensions`] when the negation has no
/// representation, which a non-zero height never produces.
pub(crate) fn top_down_bitmap_height(height: Height) -> Result<BitmapHeight, AviError> {
    let magnitude = NonZeroU32::from(height.get()).get();
    let Some(neg) = (!magnitude).checked_add(1) else {
        return Err(AviError::InvalidDimensions);
    };
    Ok(BitmapHeight(neg))
}
