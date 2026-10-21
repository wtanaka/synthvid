//! ISO base media container writing for MP4/MOV files.
use core::error::Error;
use core::fmt;
use core::num::TryFromIntError;
use synthvid_scene::Dimensions;

use crate::byte_views::{ByteSink, FrameList};
use crate::iso_helpers::{
    patch_u32_be, write_ftyp, write_mdat, write_moov, FrameRates, TrackParams,
};
use crate::iso_types::{ByteOffset, U32Be};
use crate::media::{ContainerFile, EncodedFrame};

/// Error type for ISO media encoding.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum IsoError {
    /// No frames provided.
    NoFrames,
    /// Frame data size overflow.
    SizeOverflow,
    /// Track matrix computation failed (dimensions too large).
    TrackMatrixOverflow,
}

impl fmt::Display for IsoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFrames => write!(f, "no frames provided"),
            Self::SizeOverflow => write!(f, "frame data size overflow"),
            Self::TrackMatrixOverflow => write!(f, "track matrix dimensions too large"),
        }
    }
}

impl Error for IsoError {}

impl IsoError {
    /// A length, count or rate too large for its field is a size overflow; the
    /// conversion error says nothing more than that.
    pub(crate) const fn size_overflow(_: TryFromIntError) -> Self {
        Self::SizeOverflow
    }
}

/// Codec types for ISO media files.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum IsoCodec {
    /// Uncompressed RGB.
    UncompressedRgb,
    /// Motion-JPEG.
    MotionJpeg,
}

/// Encodes frames as an ISO base media container (MP4/MOV).
///
/// Creates a fully-formed MP4/MOV file with ftyp, moov (including track matrix),
/// and mdat boxes. Supports uncompressed RGB and motion-JPEG codecs.
///
/// # Errors
///
/// Returns `IsoError` if encoding fails.
pub fn encode_iso(
    frame_rates: FrameRates,
    frames: &[EncodedFrame],
    track_params: TrackParams,
) -> Result<ContainerFile, IsoError> {
    if frames.is_empty() {
        return Err(IsoError::NoFrames);
    }

    let mut output = Vec::new();

    // Write ftyp box (20 bytes)
    write_ftyp(&mut ByteSink(&mut output));

    // Write moov box and get stco offset info for later patching
    let stco_info = write_moov(
        &mut ByteSink(&mut output),
        frame_rates,
        FrameList(frames),
        track_params,
    )?;

    // mdat header will start at the current end of output
    let mdat_header_pos = output.len();

    // Write mdat box
    write_mdat(
        &mut ByteSink(&mut output),
        FrameList(frames),
        track_params.codec,
    )?;

    // Now patch the stco offsets with real chunk offsets
    // Each chunk offset = position in file where frame data starts
    // Frame data starts at: moov_start + moov_size + 8
    let frame_data_base = mdat_header_pos
        .checked_add(8)
        .ok_or(IsoError::SizeOverflow)?;

    // Compute cumulative offsets for each frame
    let mut current_offset = frame_data_base;
    let ByteOffset(mut offset_pos) = stco_info.first_offset_pos;

    for EncodedFrame(frame) in frames {
        // Patch this frame's offset
        let offset_u32 = u32::try_from(current_offset).map_err(IsoError::size_overflow)?;
        patch_u32_be(
            &mut ByteSink(&mut output),
            ByteOffset(offset_pos),
            U32Be(offset_u32),
        );

        // Move to next offset position in stco box
        offset_pos = offset_pos.checked_add(4).ok_or(IsoError::SizeOverflow)?;

        // Update current offset for next frame
        let frame_len = u32::try_from(frame.len()).map_err(IsoError::size_overflow)?;
        let frame_len_usize = usize::try_from(frame_len).map_err(IsoError::size_overflow)?;
        current_offset = current_offset
            .checked_add(frame_len_usize)
            .ok_or(IsoError::SizeOverflow)?;
    }

    Ok(ContainerFile(output))
}

/// A 3×3 transformation matrix for track display as per ISO/IEC 14496-12 §8.3.2.
///
/// Nine signed 32-bit values where:
/// - `a`, `b`, `c`, `d`, `x`, `y` are 16.16 fixed-point (high 16 bits, low 16 bits)
/// - `u`, `v`, `w` are 2.30 fixed-point (high 2 bits, low 30 bits)
///
/// The fields are private and accessible only through constructors that ensure
/// valid transformation matrices.
///
/// The nine values are stored in the file's order: `a, b, u, c, d, v, x, y, w`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TrackMatrix([i32; 9]);

impl TrackMatrix {
    /// Creates the identity matrix.
    ///
    /// The identity matrix produces no transformation on the track display.
    #[must_use]
    pub const fn identity() -> Self {
        Self([
            0x0001_0000, // a: 1.0 in 16.16
            0,           // b
            0,           // u
            0,           // c
            0x0001_0000, // d: 1.0 in 16.16
            0,           // v
            0,           // x
            0,           // y
            0x4000_0000, // w: 1.0 in 2.30
        ])
    }

    /// Creates a 90-degree clockwise rotation matrix for a W×H track.
    ///
    /// Returns `None` if the dimensions are too large to express in the 16.16
    /// fixed-point format used for translation (32768 << 16 = `0x8000_0000` as i32 is negative).
    ///
    /// The rotation transforms a `W`×`H` track to `H`×`W` with the origin at `(0, 0)`.
    #[must_use]
    pub fn rotate_90(dims: Dimensions) -> Option<Self> {
        let height_value = dims.height.get().get();
        // Compute height as 16.16 fixed point (height * 65536)
        let height_fixed = i32::from(height_value).checked_mul(0x0001_0000)?;

        Some(Self([
            0,
            0x0001_0000,   // b: 1.0 in 16.16
            0,             // u
            NEG_ONE_16_16, // c: -1.0 in 16.16
            0,             // d
            0,             // v
            height_fixed,  // x: translate by height
            0,             // y
            0x4000_0000,   // w: 1.0 in 2.30
        ]))
    }

    /// Creates a 180-degree rotation matrix for a W×H track.
    ///
    /// Returns `None` if the dimensions are too large to express in the 16.16
    /// fixed-point format used for translation.
    ///
    /// The rotation transforms a `W`×`H` track to `W`×`H` with the origin at `(0, 0)`.
    #[must_use]
    pub fn rotate_180(dims: Dimensions) -> Option<Self> {
        let width_value = dims.width.get().get();
        let height_value = dims.height.get().get();

        let width_fixed = i32::from(width_value).checked_mul(0x0001_0000)?;
        let height_fixed = i32::from(height_value).checked_mul(0x0001_0000)?;

        Some(Self([
            NEG_ONE_16_16, // a: -1.0 in 16.16
            0,             // b
            0,             // u
            0,             // c
            NEG_ONE_16_16, // d: -1.0 in 16.16
            0,             // v
            width_fixed,   // x: translate by width
            height_fixed,  // y: translate by height
            0x4000_0000,   // w: 1.0 in 2.30
        ]))
    }

    /// Creates a 270-degree clockwise rotation matrix for a W×H track.
    ///
    /// Returns `None` if the dimensions are too large to express in the 16.16
    /// fixed-point format used for translation.
    ///
    /// The rotation transforms a `W`×`H` track to `H`×`W` with the origin at `(0, 0)`.
    #[must_use]
    pub fn rotate_270(dims: Dimensions) -> Option<Self> {
        let width_value = dims.width.get().get();
        let width_fixed = i32::from(width_value).checked_mul(0x0001_0000)?;

        Some(Self([
            0,
            NEG_ONE_16_16, // b: -1.0 in 16.16
            0,             // u
            0x0001_0000,   // c: 1.0 in 16.16
            0,             // d
            0,             // v
            0,             // x
            width_fixed,   // y: translate by width
            0x4000_0000,   // w: 1.0 in 2.30
        ]))
    }

    /// Gets the nine matrix values as an array of i32.
    ///
    /// Used for serializing the matrix to the file format.
    pub(crate) const fn as_array(&self) -> [i32; 9] {
        self.0
    }
}

/// -1.0 in 16.16 fixed point: the two's-complement bit pattern
/// `0xFFFF_0000`, which is `-65536`.
const NEG_ONE_16_16: i32 = i32::from_be_bytes([0xFF, 0xFF, 0x00, 0x00]);

#[cfg(test)]
mod tests;
