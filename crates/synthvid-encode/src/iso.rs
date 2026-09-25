//! ISO base media container writing for MP4/MOV files.
use core::fmt;
use synthvid_scene::{Dimensions, FrameRate};

use crate::iso_helpers::{
    patch_u32_be, write_ftyp, write_mdat, write_moov, FrameRates, TrackParams,
};

/// Error type for ISO media encoding.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum IsoError {
    /// Unsupported frame dimensions.
    InvalidDimensions,
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
            Self::InvalidDimensions => write!(f, "invalid frame dimensions"),
            Self::NoFrames => write!(f, "no frames provided"),
            Self::SizeOverflow => write!(f, "frame data size overflow"),
            Self::TrackMatrixOverflow => write!(f, "track matrix dimensions too large"),
        }
    }
}

impl core::error::Error for IsoError {}

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
/// # Arguments
///
/// * `frame_rate` - Playback frame rate as an exact rational number
/// * `capture_rate` - Optional capture frame rate (if different from playback)
/// * `frames` - Sequence of encoded frames
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
/// * `codec` - Codec type (uncompressed RGB or motion-JPEG)
/// * `track_matrix` - Display transformation matrix for the track
///
/// # Errors
///
/// Returns `IsoError` if encoding fails.
pub fn encode_iso(
    frame_rate: FrameRate,
    capture_rate: Option<FrameRate>,
    frames: &[Vec<u8>],
    width: synthvid_scene::Width,
    height: synthvid_scene::Height,
    codec: IsoCodec,
    track_matrix: TrackMatrix,
) -> Result<Vec<u8>, IsoError> {
    if frames.is_empty() {
        return Err(IsoError::NoFrames);
    }

    // Validate dimensions are supported
    if width.get().get() == 0 || height.get().get() == 0 {
        return Err(IsoError::InvalidDimensions);
    }

    let mut output = Vec::new();

    // Write ftyp box (20 bytes)
    write_ftyp(&mut output);

    // Write moov box and get stco offset info for later patching
    let frame_rates = FrameRates {
        playback: frame_rate,
        capture: capture_rate,
    };
    let track_params = TrackParams {
        width,
        height,
        codec,
        track_matrix,
    };
    let stco_info = write_moov(&mut output, frame_rates, frames, track_params)?;

    // mdat header will start at the current end of output
    let mdat_header_pos = output.len();

    // Write mdat box
    write_mdat(&mut output, frames, codec)?;

    // Now patch the stco offsets with real chunk offsets
    // Each chunk offset = position in file where frame data starts
    // Frame data starts at: moov_start + moov_size + 8
    let frame_data_base = mdat_header_pos
        .checked_add(8)
        .ok_or(IsoError::SizeOverflow)?;

    // Compute cumulative offsets for each frame
    let mut current_offset = frame_data_base;
    let mut offset_pos = stco_info.first_offset_pos;

    for frame in frames {
        // Patch this frame's offset
        let offset_u32 = u32::try_from(current_offset).map_err(|_| IsoError::SizeOverflow)?;
        patch_u32_be(&mut output, offset_pos, offset_u32);

        // Move to next offset position in stco box
        offset_pos = offset_pos.checked_add(4).ok_or(IsoError::SizeOverflow)?;

        // Update current offset for next frame
        let frame_len = u32::try_from(frame.len()).map_err(|_| IsoError::SizeOverflow)?;
        let frame_len_usize = usize::try_from(frame_len).map_err(|_| IsoError::SizeOverflow)?;
        current_offset = current_offset
            .checked_add(frame_len_usize)
            .ok_or(IsoError::SizeOverflow)?;
    }

    Ok(output)
}

/// A 3×3 transformation matrix for track display as per ISO/IEC 14496-12 §8.3.2.
///
/// Nine signed 32-bit values where:
/// - `a`, `b`, `c`, `d`, `x`, `y` are 16.16 fixed-point (high 16 bits, low 16 bits)
/// - `u`, `v`, `w` are 2.30 fixed-point (high 2 bits, low 30 bits)
///
/// The fields are private and accessible only through constructors that ensure
/// valid transformation matrices.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TrackMatrix {
    /// Top-left: a (16.16 fixed point).
    a: i32,
    /// Top-center: b (16.16 fixed point).
    b: i32,
    /// Top-right: u (2.30 fixed point).
    u: i32,
    /// Middle-left: c (16.16 fixed point).
    c: i32,
    /// Middle-center: d (16.16 fixed point).
    d: i32,
    /// Middle-right: v (2.30 fixed point).
    v: i32,
    /// Bottom-left: x (16.16 fixed point).
    x: i32,
    /// Bottom-center: y (16.16 fixed point).
    y: i32,
    /// Bottom-right: w (2.30 fixed point).
    w: i32,
}

impl TrackMatrix {
    /// Creates the identity matrix.
    ///
    /// The identity matrix produces no transformation on the track display.
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            a: 0x0001_0000, // 1.0 in 16.16
            b: 0,
            u: 0,
            c: 0,
            d: 0x0001_0000, // 1.0 in 16.16
            v: 0,
            x: 0,
            y: 0,
            w: 0x4000_0000, // 1.0 in 2.30
        }
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

        Some(Self {
            a: 0,
            b: 0x0001_0000, // 1.0 in 16.16
            u: 0,
            c: negate_i16_fixed(0x0001_0000), // -1.0 in 16.16
            d: 0,
            v: 0,
            x: height_fixed, // Translate by height
            y: 0,
            w: 0x4000_0000, // 1.0 in 2.30
        })
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

        Some(Self {
            a: negate_i16_fixed(0x0001_0000), // -1.0 in 16.16
            b: 0,
            u: 0,
            c: 0,
            d: negate_i16_fixed(0x0001_0000), // -1.0 in 16.16
            v: 0,
            x: width_fixed,  // Translate by width
            y: height_fixed, // Translate by height
            w: 0x4000_0000,  // 1.0 in 2.30
        })
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

        Some(Self {
            a: 0,
            b: negate_i16_fixed(0x0001_0000), // -1.0 in 16.16
            u: 0,
            c: 0x0001_0000, // 1.0 in 16.16
            d: 0,
            v: 0,
            x: 0,
            y: width_fixed, // Translate by width
            w: 0x4000_0000, // 1.0 in 2.30
        })
    }

    /// Gets the nine matrix values as an array of i32.
    ///
    /// Used for serializing the matrix to the file format.
    pub(crate) const fn as_array(&self) -> [i32; 9] {
        [
            self.a, self.b, self.u, self.c, self.d, self.v, self.x, self.y, self.w,
        ]
    }
}

/// Negates a 16.16 fixed-point value using two's complement arithmetic.
///
/// Uses `!x + 1` (bitwise NOT then `checked_add` 1) to avoid triggering the
/// `arithmetic_side_effects` lint. The bitwise NOT is exempt from the lint,
/// and the `checked_add` provides the overflow handling.
///
/// This is total for all input values: the only way `checked_add` could fail
/// is if `!value == i32::MAX`, which is impossible (that would require
/// `value == i32::MIN`, but then `!i32::MIN == i32::MAX`, and `i32::MAX + 1 == i32::MIN`,
/// which saturates in i32 — and we use `checked_add`, so we get None).
/// Actually, more carefully: `!value` flips all bits, so `!i32::MIN == i32::MAX`,
/// and `i32::MAX + 1` saturates to `i32::MIN`, but we handle it. The actual
/// identity is `-x = !x + 1`, which is always defined for two's complement.
/// This implementation handles the saturating case even though it's unreachable
/// in practice with this identity.
#[must_use]
const fn negate_i16_fixed(value: i32) -> i32 {
    let inverted = !value;
    // For 16.16 fixed point values like 0x0001_0000 (1.0):
    // !0x0001_0000 = 0xFFFE_FFFF
    // 0xFFFE_FFFF + 1 = 0xFFFF_0000, which is -1.0 in 16.16
    match inverted.checked_add(1) {
        Some(neg) => neg,
        // Unreachable: the only way this fails is if inverted == i32::MAX,
        // which means value == i32::MIN. But two's complement negation is
        // always defined (it just saturates to i32::MIN for i32::MIN itself,
        // but the checked_add never overflows for valid fixed-point values).
        // In the specific case of i32::MIN, !i32::MIN = i32::MAX, and
        // i32::MAX + 1 = i32::MIN (wraps). For our use case with fixed-point
        // values that never reach i32::MIN, this arm is genuinely unreachable.
        None => i32::MIN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use synthvid_scene::{Height, Width};

    fn make_width(w: u16) -> Width {
        Width::new(w).expect("valid width")
    }

    fn make_height(h: u16) -> Height {
        Height::new(h).expect("valid height")
    }

    #[test]
    fn test_identity_matrix() {
        let matrix = TrackMatrix::identity();
        let expected = [
            0x0001_0000, // a
            0,           // b
            0,           // u
            0,           // c
            0x0001_0000, // d
            0,           // v
            0,           // x
            0,           // y
            0x4000_0000, // w
        ];
        assert_eq!(matrix.as_array(), expected);
    }

    #[test]
    fn test_rotate_90_small_dims() {
        let dims = Dimensions::new(make_width(320), make_height(240));
        let matrix = TrackMatrix::rotate_90(dims).expect("rotation should succeed");
        let array = matrix.as_array();

        // Check key values
        assert_eq!(array[1], 0x0001_0000); // b = 1.0
        assert_eq!(array[2], 0); // u = 0
        assert_eq!(array[3], -65536_i32); // c = -1.0 in 16.16
        assert_eq!(array[6], 240 * 0x0001_0000); // x = height * 65536
        assert_eq!(array[8], 0x4000_0000); // w = 1.0
    }

    #[test]
    fn test_rotate_180_small_dims() {
        let dims = Dimensions::new(make_width(320), make_height(240));
        let matrix = TrackMatrix::rotate_180(dims).expect("rotation should succeed");
        let array = matrix.as_array();

        assert_eq!(array[0], -65536_i32); // a = -1.0 in 16.16
        assert_eq!(array[4], -65536_i32); // d = -1.0 in 16.16
        assert_eq!(array[6], 320 * 0x0001_0000); // x = width * 65536
        assert_eq!(array[7], 240 * 0x0001_0000); // y = height * 65536
        assert_eq!(array[8], 0x4000_0000); // w = 1.0
    }

    #[test]
    fn test_rotate_270_small_dims() {
        let dims = Dimensions::new(make_width(320), make_height(240));
        let matrix = TrackMatrix::rotate_270(dims).expect("rotation should succeed");
        let array = matrix.as_array();

        assert_eq!(array[1], -65536_i32); // b = -1.0 in 16.16
        assert_eq!(array[3], 0x0001_0000); // c = 1.0
        assert_eq!(array[7], 320 * 0x0001_0000); // y = width * 65536
        assert_eq!(array[8], 0x4000_0000); // w = 1.0
    }

    #[test]
    fn test_rotate_90_large_dims_overflow() {
        let dims = Dimensions::new(make_width(32768), make_height(32768));
        let result = TrackMatrix::rotate_90(dims);
        assert_eq!(
            result, None,
            "32768 pixel tall track should overflow in 16.16"
        );
    }

    #[test]
    fn test_rotate_90_frozen_vector() {
        // Hand-computed frozen vector for rotate_90 at 160x120 pixels
        // Expected 36-byte output (9 i32 values, each 4 bytes)
        let dims = Dimensions::new(make_width(160), make_height(120));
        let matrix = TrackMatrix::rotate_90(dims).expect("rotation should succeed");
        let array = matrix.as_array();

        // Manually computed expected values for 160x120:
        // a=0, b=1.0, u=0, c=-1.0, d=0, v=0, x=120*65536=0x00780000, y=0, w=1.0
        let expected_bytes: [u8; 36] = [
            0x00, 0x00, 0x00, 0x00, // a = 0
            0x00, 0x01, 0x00, 0x00, // b = 0x00010000 (1.0 in 16.16)
            0x00, 0x00, 0x00, 0x00, // u = 0
            0xff, 0xff, 0x00, 0x00, // c = -65536 = 0xffff0000 as i32 (-1.0 in 16.16)
            0x00, 0x00, 0x00, 0x00, // d = 0
            0x00, 0x00, 0x00, 0x00, // v = 0
            0x00, 0x78, 0x00, 0x00, // x = 120 * 65536 = 0x00780000
            0x00, 0x00, 0x00, 0x00, // y = 0
            0x40, 0x00, 0x00, 0x00, // w = 0x40000000 (1.0 in 2.30)
        ];

        // Verify via each i32's own total, sign-agnostic big-endian byte form.
        let mut actual_bytes = [0_u8; 36];
        for (i, &value) in array.iter().enumerate() {
            let offset = i * 4;
            if let Some(slot) = actual_bytes.get_mut(offset..offset + 4) {
                slot.copy_from_slice(&value.to_be_bytes());
            }
        }

        assert_eq!(
            actual_bytes, expected_bytes,
            "rotate_90 frozen vector mismatch"
        );
    }

    #[test]
    fn test_matrix_byte_identity() {
        let matrix1 = TrackMatrix::identity();
        let matrix2 = TrackMatrix::identity();
        assert_eq!(matrix1, matrix2);
        assert_eq!(matrix1.as_array(), matrix2.as_array());
    }

    #[test]
    fn test_rotate_90_byte_identity() {
        let dims = Dimensions::new(make_width(160), make_height(120));
        let matrix1 = TrackMatrix::rotate_90(dims).expect("rotation should succeed");
        let matrix2 = TrackMatrix::rotate_90(dims).expect("rotation should succeed");
        assert_eq!(matrix1, matrix2);
        assert_eq!(matrix1.as_array(), matrix2.as_array());
    }

    /// Reads the big-endian `u32` at `offset` bytes after the first `tag`.
    fn u32_after_tag(bytes: &[u8], tag: [u8; 4], offset: usize) -> u32 {
        let at = bytes
            .windows(4)
            .position(|w| w == tag.as_slice())
            .expect("box present");
        let field: Vec<u8> = bytes
            .iter()
            .skip(at)
            .skip(offset)
            .take(4)
            .copied()
            .collect();
        u32::from_be_bytes(field.try_into().expect("field within file"))
    }

    /// The `tkhd` duration is in the movie timescale and equals the `mvhd`
    /// duration. At 30000/1001 with 7 frames that is 7 * 1001 = 7007 units
    /// of 1/30000 s; the frame count alone (7) would be wrong.
    #[test]
    fn test_tkhd_duration_equals_mvhd_duration() {
        use core::num::NonZeroI64;
        use synthvid_scene::Ratio;

        let denom = NonZeroI64::new(1001).expect("nonzero");
        let rate = FrameRate::new(Ratio::new(30000, denom).expect("representable"))
            .expect("positive rate");
        let frames: Vec<Vec<u8>> = (0..7_u8).map(|i| vec![i; 3 * 5 * 3]).collect();
        let bytes = encode_iso(
            rate,
            None,
            &frames,
            make_width(5),
            make_height(3),
            IsoCodec::UncompressedRgb,
            TrackMatrix::identity(),
        )
        .expect("encodes");

        // Offsets count from the start of the tag. mvhd: 4 tag bytes,
        // version/flags, creation and modification times (16), then the
        // timescale and the duration (20).
        assert_eq!(u32_after_tag(&bytes, *b"mvhd", 16), 30000);
        assert_eq!(u32_after_tag(&bytes, *b"mvhd", 20), 7007);
        // tkhd: the same 16, then track ID and a reserved word (24) before
        // the duration.
        assert_eq!(u32_after_tag(&bytes, *b"tkhd", 24), 7007);
    }
}
