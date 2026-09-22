//! Owned RGB pixel buffers.
//!
//! [`Frame`] pairs [`Dimensions`] with a byte buffer whose
//! length is guaranteed to be exactly `width * height * 3`. Every accessor is
//! bounds-checked and returns [`Option`], so no operation on a frame panics.

use core::fmt;

use crate::color::{blend_channel, required_buffer_len, Rgb8};
use crate::units::{Dimensions, Height, Width};

/// A pixel coordinate with x and y components.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PixelCoord {
    /// The x coordinate of the pixel.
    x: u16,
    /// The y coordinate of the pixel.
    y: u16,
}

impl PixelCoord {
    /// Creates a new pixel coordinate with the given x and y values.
    #[must_use]
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

/// A video frame owning a 24-bit RGB pixel buffer of exactly `width * height * 3` bytes.
///
/// Constructed only through constructors that strictly guarantee the length invariant.
/// Accessors are bounds-checked and return [`Option`]. No indexing operators can panic.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Frame {
    /// The 2D dimensions of the frame.
    dimensions: Dimensions,
    /// The raw 24-bit RGB pixel buffer (3 bytes per pixel: red, green, blue).
    data: Vec<u8>,
}

/// A pixel coordinate lay outside the frame.
///
/// This is returned rather than an `Option<()>` so that dropping it is a
/// compile error: `Result` is `#[must_use]` and `Option` is not, and a
/// silently dropped write is how a frame came to be emitted with pixels
/// missing.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PixelOutOfBounds;

impl fmt::Display for PixelOutOfBounds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pixel coordinate lies outside the frame")
    }
}

impl core::error::Error for PixelOutOfBounds {}

/// Error returned when attempting to construct a [`Frame`] with invalid data.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FrameError {
    /// Buffer length does not match the required size for the dimensions or overflows.
    InvalidBufferSize,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBufferSize => {
                write!(f, "buffer length does not match the required size")
            }
        }
    }
}

impl core::error::Error for FrameError {}

/// Largest pixel index any frame can produce: `y * width + x` with the maximum
/// `u16` dimensions. Computed in `u32`, which is the narrowest type that must
/// hold it. If this bound were false the constant would overflow and the crate
/// would not compile.
const _MAX_PIXEL_INDEX: u32 = 65_535 * 65_535 + 65_535;

/// This crate addresses pixel buffers through `usize`, so it requires a target
/// whose `usize` is at least 32 bits. On anything narrower this constant
/// underflows and the crate does not build, rather than silently addressing
/// the wrong byte.
const _USIZE_AT_LEAST_32_BITS: usize = usize::MAX - 4_294_967_295;

impl Frame {
    /// Creates a new [`Frame`] from [`Dimensions`] and an owned pixel buffer.
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameError::InvalidBufferSize)` if `data.len()` does not exactly match
    /// `width * height * 3` bytes, or if the required buffer length calculation overflows `usize`.
    pub fn new(dimensions: Dimensions, data: Vec<u8>) -> Result<Self, FrameError> {
        let expected_len = required_buffer_len(dimensions).ok_or(FrameError::InvalidBufferSize)?;
        if data.len() != expected_len {
            return Err(FrameError::InvalidBufferSize);
        }
        Ok(Self { dimensions, data })
    }

    /// Creates a new [`Frame`] from [`Dimensions`] and an owned vector of bytes.
    ///
    /// Alias for [`Self::new`].
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameError::InvalidBufferSize)` if `data.len()` does not exactly match
    /// the required size or if the buffer length calculation overflows `usize`.
    pub fn from_vec(dimensions: Dimensions, data: Vec<u8>) -> Result<Self, FrameError> {
        Self::new(dimensions, data)
    }

    /// Creates a new [`Frame`] from [`Dimensions`] and an owned buffer of bytes.
    ///
    /// Alias for [`Self::new`].
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameError::InvalidBufferSize)` if `data.len()` does not exactly match
    /// the required size or if the buffer length calculation overflows `usize`.
    pub fn from_buffer(dimensions: Dimensions, data: Vec<u8>) -> Result<Self, FrameError> {
        Self::new(dimensions, data)
    }

    /// Creates a zero-initialized (black) [`Frame`] with the given dimensions.
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameError::InvalidBufferSize)` if the required buffer length
    /// calculation overflows `usize`.
    pub fn zeroed(dimensions: Dimensions) -> Result<Self, FrameError> {
        let len = required_buffer_len(dimensions).ok_or(FrameError::InvalidBufferSize)?;
        let data = vec![0_u8; len];
        Ok(Self { dimensions, data })
    }

    /// Creates a new [`Frame`] filled uniformly with the given [`Rgb8`] color.
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameError::InvalidBufferSize)` if the required buffer length
    /// calculation overflows `usize`.
    pub fn from_color(dimensions: Dimensions, color: Rgb8) -> Result<Self, FrameError> {
        let mut frame = Self::zeroed(dimensions)?;
        frame.fill(color);
        Ok(frame)
    }

    /// Returns the 2D dimensions of the frame.
    #[must_use]
    pub const fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    /// Returns the width of the frame.
    #[must_use]
    pub const fn width(&self) -> Width {
        self.dimensions.width
    }

    /// Returns the height of the frame.
    #[must_use]
    pub const fn height(&self) -> Height {
        self.dimensions.height
    }

    /// Returns a slice of the underlying raw RGB byte buffer.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns a slice of the underlying raw RGB byte buffer.
    ///
    /// Alias for [`Self::data`].
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Returns a slice of the underlying raw RGB byte buffer.
    ///
    /// Alias for [`Self::data`].
    #[must_use]
    pub fn buffer(&self) -> &[u8] {
        &self.data
    }

    /// Consumes the frame and returns the owned raw RGB byte buffer.
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.data
    }

    /// Consumes the frame and returns the owned raw RGB byte buffer.
    ///
    /// Alias for [`Self::into_vec`].
    #[must_use]
    pub fn into_buffer(self) -> Vec<u8> {
        self.data
    }

    /// Returns the total number of bytes in the pixel buffer.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the pixel buffer is empty.
    ///
    /// Note that frames always have non-zero dimensions, so this always returns `false`.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the byte value at the given raw buffer index, or `None` if out of bounds.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<u8> {
        self.data.get(index).copied()
    }

    /// Returns the [`Rgb8`] pixel color at the given coordinate,
    /// or `None` if the coordinate is out of bounds.
    #[must_use]
    pub fn pixel(&self, coord: PixelCoord) -> Option<Rgb8> {
        self.pixel_inner(coord).ok()
    }

    /// Retrieves pixel, distinguishing bounds from arithmetic errors.
    fn pixel_inner(&self, coord: PixelCoord) -> Result<Rgb8, PixelOutOfBounds> {
        let width = self.dimensions.width.get().get();
        let height = self.dimensions.height.get().get();

        if coord.x >= width || coord.y >= height {
            return Err(PixelOutOfBounds);
        }

        let [r_idx, g_idx, b_idx] = self.channel_offsets(coord);

        let red = *self.data.get(r_idx).ok_or(PixelOutOfBounds)?;
        let green = *self.data.get(g_idx).ok_or(PixelOutOfBounds)?;
        let blue = *self.data.get(b_idx).ok_or(PixelOutOfBounds)?;

        Ok(Rgb8::new(red, green, blue))
    }

    /// Returns the [`Rgb8`] pixel color at the given coordinate,
    /// or `None` if the coordinate is out of bounds.
    ///
    /// Alias for [`Self::pixel`].
    #[must_use]
    pub fn get_pixel(&self, coord: PixelCoord) -> Option<Rgb8> {
        self.pixel(coord)
    }

    /// Returns the 3-byte RGB slice for the pixel at the given coordinate,
    /// or `None` if out of bounds.
    #[must_use]
    pub fn pixel_bytes(&self, coord: PixelCoord) -> Option<&[u8]> {
        self.pixel_bytes_inner(coord).ok()
    }

    /// Retrieves pixel bytes, distinguishing bounds from arithmetic errors.
    fn pixel_bytes_inner(&self, coord: PixelCoord) -> Result<&[u8], PixelOutOfBounds> {
        let width = self.dimensions.width.get().get();
        let height = self.dimensions.height.get().get();

        if coord.x >= width || coord.y >= height {
            return Err(PixelOutOfBounds);
        }

        let [byte_index, _, end_index] = self.channel_offsets(coord);
        self.data
            .get(byte_index..=end_index)
            .ok_or(PixelOutOfBounds)
    }

    /// Sets the pixel at the given coordinate to `color`.
    ///
    /// # Errors
    ///
    /// [`PixelOutOfBounds`] if the coordinate lies outside the frame.
    pub fn set_pixel(&mut self, coord: PixelCoord, color: Rgb8) -> Result<(), PixelOutOfBounds> {
        self.set_pixel_inner(coord, color)
    }

    /// Blends `color` into the pixel at the given coordinate with the given `alpha`.
    ///
    /// An `alpha` of 255 replaces the pixel entirely; 0 leaves it unchanged.
    ///
    /// # Errors
    ///
    /// [`PixelOutOfBounds`] if the coordinate lies outside the frame.
    pub fn blend_pixel(
        &mut self,
        coord: PixelCoord,
        color: Rgb8,
        alpha: u8,
    ) -> Result<(), PixelOutOfBounds> {
        self.blend_pixel_inner(coord, color, alpha)
    }

    /// Writes the pixel, or an error if the coordinate lies outside the frame or arithmetic overflows.
    fn set_pixel_inner(&mut self, coord: PixelCoord, color: Rgb8) -> Result<(), PixelOutOfBounds> {
        let width = self.dimensions.width.get().get();
        let height = self.dimensions.height.get().get();

        if coord.x >= width || coord.y >= height {
            return Err(PixelOutOfBounds);
        }

        let [r_idx, g_idx, b_idx] = self.channel_offsets(coord);

        self.data
            .get_mut(r_idx)
            .ok_or(PixelOutOfBounds)
            .map(|r| *r = color.r)?;
        self.data
            .get_mut(g_idx)
            .ok_or(PixelOutOfBounds)
            .map(|g| *g = color.g)?;
        self.data
            .get_mut(b_idx)
            .ok_or(PixelOutOfBounds)
            .map(|b| *b = color.b)?;

        Ok(())
    }

    /// Converts a byte index (u64) to a usize for buffer indexing.
    /// Returns `None` if the index exceeds `usize::MAX`.
    #[inline]
    #[must_use]
    /// Byte offsets of a pixel's three channels within [`Frame::data`].
    ///
    /// The caller must already have established `coord.x < width` and
    /// `coord.y < height`; every call site does so immediately above.
    ///
    /// The suppression below is discharged by the `_MAX_*` constants beside
    /// this function rather than by this comment. A `const` whose arithmetic
    /// overflows is a compile error, and those constants compute the same
    /// bounds in the same type this expression uses, for the target actually
    /// being compiled -- so the claim is checked where it matters rather than
    /// resting on an assumption about one developer's machine.
    ///
    /// The multiply by three is bounded by the allocation instead: `data` was
    /// built with `width * height * 3` bytes, already a valid `usize`, and
    /// these offsets are strictly below that length. That part is an invariant
    /// of construction, which the constants cannot state and which the slice
    /// accesses at each call site still check.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "bounds proved by _MAX_PIXEL_INDEX and _USIZE_AT_LEAST_32_BITS at compile time"
    )]
    fn channel_offsets(&self, coord: PixelCoord) -> [usize; 3] {
        let width = usize::from(self.dimensions.width.get().get());
        let red = (usize::from(coord.y) * width + usize::from(coord.x)) * 3;
        [red, red + 1, red + 2]
    }

    /// Fills the entire frame with a uniform [`Rgb8`] color.
    pub fn fill(&mut self, color: Rgb8) {
        let (chunks, _) = self.data.as_chunks_mut::<3>();
        for pixel in chunks {
            *pixel = [color.r, color.g, color.b];
        }
    }

    /// Blends a pixel at the given coordinate with an [`Rgb8`] color and alpha value.
    ///
    /// An `alpha` of 255 replaces the destination pixel entirely with `color`.
    /// An `alpha` of 0 leaves the destination pixel unchanged.
    /// Intermediate values linearly blend using integer arithmetic with symmetric rounding.
    ///
    /// Returns an error if the coordinate is out of bounds or arithmetic overflows.
    fn blend_pixel_inner(
        &mut self,
        coord: PixelCoord,
        color: Rgb8,
        alpha: u8,
    ) -> Result<(), PixelOutOfBounds> {
        let width = self.dimensions.width.get().get();
        let height = self.dimensions.height.get().get();

        if coord.x >= width || coord.y >= height {
            return Err(PixelOutOfBounds);
        }
        if alpha == 0 {
            return Ok(());
        }

        let [r_idx, g_idx, b_idx] = self.channel_offsets(coord);

        if alpha == 255 {
            *self.data.get_mut(r_idx).ok_or(PixelOutOfBounds)? = color.r;
            *self.data.get_mut(g_idx).ok_or(PixelOutOfBounds)? = color.g;
            *self.data.get_mut(b_idx).ok_or(PixelOutOfBounds)? = color.b;
            return Ok(());
        }

        let dst_red = *self.data.get(r_idx).ok_or(PixelOutOfBounds)?;
        let dst_green = *self.data.get(g_idx).ok_or(PixelOutOfBounds)?;
        let dst_blue = *self.data.get(b_idx).ok_or(PixelOutOfBounds)?;

        *self.data.get_mut(r_idx).ok_or(PixelOutOfBounds)? =
            blend_channel(color.r, dst_red, alpha).ok_or(PixelOutOfBounds)?;
        *self.data.get_mut(g_idx).ok_or(PixelOutOfBounds)? =
            blend_channel(color.g, dst_green, alpha).ok_or(PixelOutOfBounds)?;
        *self.data.get_mut(b_idx).ok_or(PixelOutOfBounds)? =
            blend_channel(color.b, dst_blue, alpha).ok_or(PixelOutOfBounds)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgb8;

    #[test]
    fn test_frame_buffer_length_invariant() {
        let w = Width::new(4).unwrap();
        let h = Height::new(3).unwrap();
        let dims = Dimensions {
            width: w,
            height: h,
        };
        // Expected buffer size: 4 * 3 * 3 = 36 bytes.
        let expected_len = required_buffer_len(dims).unwrap();
        assert_eq!(expected_len, 36, "4x3 RGB frame requires exactly 36 bytes");

        // Too short buffer must be rejected.
        let too_short = vec![0_u8; 35];
        assert!(
            Frame::new(dims, too_short).is_err(),
            "buffer shorter than required must return Err"
        );

        // Too long buffer must be rejected.
        let too_long = vec![0_u8; 37];
        assert!(
            Frame::new(dims, too_long).is_err(),
            "buffer longer than required must return Err"
        );

        // Empty buffer must be rejected.
        assert!(
            Frame::new(dims, Vec::new()).is_err(),
            "empty buffer must return Err"
        );

        // Exact length buffer must succeed.
        let exact = vec![0_u8; 36];
        let frame = Frame::new(dims, exact).expect("exact length buffer must return Ok(Frame)");
        assert_eq!(frame.len(), 36, "frame len must equal 36");
        assert!(!frame.is_empty(), "frame must not be empty");
        assert_eq!(frame.dimensions(), dims, "dimensions must match");
        assert_eq!(frame.width(), w, "width must match");
        assert_eq!(frame.height(), h, "height must match");
    }

    #[test]
    fn test_frame_zeroed_and_from_color() {
        let w = Width::new(2).unwrap();
        let h = Height::new(2).unwrap();
        let dims = Dimensions {
            width: w,
            height: h,
        };

        let zeroed = Frame::zeroed(dims).unwrap();
        assert_eq!(zeroed.len(), 12, "2x2 frame must have 12 bytes");
        for b in zeroed.data() {
            assert_eq!(*b, 0, "all bytes in zeroed frame must be 0");
        }

        let red = Rgb8::new(255, 0, 0);
        let color_frame = Frame::from_color(dims, red).unwrap();
        for y in 0..2_u16 {
            for x in 0..2_u16 {
                assert_eq!(
                    color_frame.pixel(PixelCoord::new(x, y)),
                    Some(red),
                    "pixel must match filled color"
                );
            }
        }
    }

    #[test]
    fn test_frame_bounds_checked_accessors() {
        let w = Width::new(3).unwrap();
        let h = Height::new(2).unwrap();
        let dims = Dimensions {
            width: w,
            height: h,
        };
        let mut frame = Frame::zeroed(dims).unwrap();

        // Within bounds
        let col = Rgb8::new(10, 20, 30);
        let coord_1_1 = PixelCoord::new(1, 1);
        assert!(
            frame.set_pixel(coord_1_1, col).is_ok(),
            "setting pixel within bounds must succeed"
        );
        assert_eq!(
            frame.pixel(coord_1_1),
            Some(col),
            "pixel at (1, 1) must return set color"
        );
        assert_eq!(
            frame.get_pixel(coord_1_1),
            Some(col),
            "get_pixel at (1, 1) must return set color"
        );
        assert_eq!(
            frame.pixel_bytes(coord_1_1),
            Some(&[10, 20, 30][..]),
            "pixel_bytes at (1, 1) must return [10, 20, 30]"
        );

        // Out of bounds: x == width
        let coord_3_0 = PixelCoord::new(3, 0);
        assert!(
            frame.pixel(coord_3_0).is_none(),
            "pixel at x == width must return None"
        );
        assert!(
            frame.pixel_bytes(coord_3_0).is_none(),
            "pixel_bytes at x == width must return None"
        );
        assert!(
            frame.set_pixel(coord_3_0, col).is_err(),
            "set_pixel at x == width must return None"
        );

        // Out of bounds: y == height
        let coord_0_2 = PixelCoord::new(0, 2);
        assert!(
            frame.pixel(coord_0_2).is_none(),
            "pixel at y == height must return None"
        );
        assert!(
            frame.set_pixel(coord_0_2, col).is_err(),
            "set_pixel at y == height must return None"
        );

        // Far out of bounds
        let coord_max = PixelCoord::new(u16::MAX, u16::MAX);
        assert!(
            frame.pixel(coord_max).is_none(),
            "pixel at u16::MAX must return None"
        );
        assert!(
            frame.pixel_bytes(coord_max).is_none(),
            "pixel_bytes at u16::MAX must return None"
        );
        assert!(
            frame.set_pixel(coord_max, col).is_err(),
            "set_pixel at u16::MAX must return None"
        );

        // Raw byte access
        assert_eq!(frame.get(0), Some(0), "byte 0 must be 0");
        assert!(
            frame.get(18).is_none(),
            "byte index 18 on 18-byte buffer must return None"
        );
    }

    #[test]
    fn test_frame_fill() {
        let w = Width::new(2).unwrap();
        let h = Height::new(2).unwrap();
        let dims = Dimensions {
            width: w,
            height: h,
        };
        let mut frame = Frame::zeroed(dims).unwrap();

        let green = Rgb8::new(0, 255, 0);
        frame.fill(green);

        for y in 0..2_u16 {
            for x in 0..2_u16 {
                assert_eq!(
                    frame.pixel(PixelCoord::new(x, y)),
                    Some(green),
                    "every pixel after fill must match green"
                );
            }
        }
    }

    #[test]
    fn test_frame_blend_pixel() {
        let w = Width::new(2).unwrap();
        let h = Height::new(2).unwrap();
        let dims = Dimensions {
            width: w,
            height: h,
        };
        let mut frame = Frame::zeroed(dims).unwrap();

        // Set pixel (0, 0) to white (255, 255, 255)
        let white = Rgb8::new(255, 255, 255);
        let coord_0_0 = PixelCoord::new(0, 0);
        assert!(
            frame.set_pixel(coord_0_0, white).is_ok(),
            "set_pixel(0, 0) must succeed"
        );

        // Blend with black (0, 0, 0) at alpha 0 -> must remain white
        let black = Rgb8::new(0, 0, 0);
        assert!(
            frame.blend_pixel(coord_0_0, black, 0).is_ok(),
            "blend at alpha 0 must succeed"
        );
        assert_eq!(
            frame.pixel(coord_0_0),
            Some(white),
            "pixel with alpha 0 blend must remain unchanged"
        );

        // Blend with red (255, 0, 0) at alpha 255 -> must become pure red
        let red = Rgb8::new(255, 0, 0);
        assert!(
            frame.blend_pixel(coord_0_0, red, 255).is_ok(),
            "blend at alpha 255 must succeed"
        );
        assert_eq!(
            frame.pixel(coord_0_0),
            Some(red),
            "pixel with alpha 255 blend must become source color"
        );

        // Set pixel (1, 1) to (100, 100, 100) and blend with (200, 200, 200) at alpha 128
        let gray100 = Rgb8::new(100, 100, 100);
        let coord_1_1 = PixelCoord::new(1, 1);
        assert!(
            frame.set_pixel(coord_1_1, gray100).is_ok(),
            "set_pixel(1, 1) must succeed"
        );
        let gray200 = Rgb8::new(200, 200, 200);
        assert!(
            frame.blend_pixel(coord_1_1, gray200, 128).is_ok(),
            "blend at alpha 128 must succeed"
        );
        // Blend formula: (200 * 128 + 100 * 127 + 127) / 255 = (25600 + 12700 + 127) / 255 = 38427 / 255 = 150
        let blended = frame
            .pixel(coord_1_1)
            .expect("pixel after blend must exist");
        assert_eq!(
            blended.r, 150,
            "blended channel at 50% opacity must equal 150"
        );
        assert_eq!(
            blended.g, 150,
            "blended channel at 50% opacity must equal 150"
        );
        assert_eq!(
            blended.b, 150,
            "blended channel at 50% opacity must equal 150"
        );

        // Out of bounds blend returns None
        let coord_2_0 = PixelCoord::new(2, 0);
        assert!(
            frame.blend_pixel(coord_2_0, red, 255).is_err(),
            "out of bounds blend must return None"
        );
        let coord_max = PixelCoord::new(u16::MAX, u16::MAX);
        assert!(
            frame.blend_pixel(coord_max, red, 255).is_err(),
            "out of bounds blend at u16::MAX must return None"
        );
    }
}
