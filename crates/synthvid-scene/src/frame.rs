//! Owned RGB pixel buffers.
//!
//! [`Frame`] pairs [`Dimensions`] with a byte buffer whose
//! length is guaranteed to be exactly `width * height * 3`. Every accessor is
//! bounds-checked and returns [`Option`], so no operation on a frame panics.

use crate::color::{blend_channel, required_buffer_len, Rgb8};
use crate::units::{Dimensions, Height, Width};

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

impl Frame {
    /// Creates a new [`Frame`] from [`Dimensions`] and an owned pixel buffer.
    ///
    /// Returns `None` if `data.len()` does not exactly match `width * height * 3` bytes,
    /// or if the required buffer length calculation overflows `usize`.
    #[must_use]
    pub fn new(dimensions: Dimensions, data: Vec<u8>) -> Option<Self> {
        let expected_len = required_buffer_len(dimensions)?;
        if data.len() != expected_len {
            return None;
        }
        Some(Self { dimensions, data })
    }

    /// Creates a new [`Frame`] from [`Dimensions`] and an owned vector of bytes.
    ///
    /// Alias for [`Self::new`].
    #[must_use]
    pub fn from_vec(dimensions: Dimensions, data: Vec<u8>) -> Option<Self> {
        Self::new(dimensions, data)
    }

    /// Creates a new [`Frame`] from [`Dimensions`] and an owned buffer of bytes.
    ///
    /// Alias for [`Self::new`].
    #[must_use]
    pub fn from_buffer(dimensions: Dimensions, data: Vec<u8>) -> Option<Self> {
        Self::new(dimensions, data)
    }

    /// Creates a zero-initialized (black) [`Frame`] with the given dimensions.
    ///
    /// Returns `None` if the required buffer length calculation overflows `usize`.
    #[must_use]
    pub fn zeroed(dimensions: Dimensions) -> Option<Self> {
        let len = required_buffer_len(dimensions)?;
        let data = vec![0_u8; len];
        Some(Self { dimensions, data })
    }

    /// Creates a new [`Frame`] filled uniformly with the given [`Rgb8`] color.
    ///
    /// Returns `None` if the required buffer length calculation overflows `usize`.
    #[must_use]
    pub fn from_color(dimensions: Dimensions, color: Rgb8) -> Option<Self> {
        let mut frame = Self::zeroed(dimensions)?;
        frame.fill(color);
        Some(frame)
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

    /// Returns the [`Rgb8`] pixel color at the given `(x, y)` coordinate,
    /// or `None` if the coordinate is out of bounds.
    #[must_use]
    pub fn pixel(&self, x: u16, y: u16) -> Option<Rgb8> {
        if x >= self.dimensions.width.get().get() || y >= self.dimensions.height.get().get() {
            return None;
        }

        let x_usize = usize::from(x);
        let y_usize = usize::from(y);
        let width_usize = usize::from(self.dimensions.width.get().get());

        // checked_mul and checked_add calculate row and pixel offsets safely.
        let row_offset = y_usize.checked_mul(width_usize)?;
        let pixel_index = row_offset.checked_add(x_usize)?;
        let byte_index = pixel_index.checked_mul(3)?;

        let r_idx = byte_index;
        let g_idx = byte_index.checked_add(1)?;
        let b_idx = byte_index.checked_add(2)?;

        let red = *self.data.get(r_idx)?;
        let green = *self.data.get(g_idx)?;
        let blue = *self.data.get(b_idx)?;

        Some(Rgb8::new(red, green, blue))
    }

    /// Returns the [`Rgb8`] pixel color at the given `(x, y)` coordinate,
    /// or `None` if the coordinate is out of bounds.
    ///
    /// Alias for [`Self::pixel`].
    #[must_use]
    pub fn get_pixel(&self, x: u16, y: u16) -> Option<Rgb8> {
        self.pixel(x, y)
    }

    /// Returns the 3-byte RGB slice for the pixel at `(x, y)`,
    /// or `None` if out of bounds.
    #[must_use]
    pub fn pixel_bytes(&self, x: u16, y: u16) -> Option<&[u8]> {
        if x >= self.dimensions.width.get().get() || y >= self.dimensions.height.get().get() {
            return None;
        }

        let x_usize = usize::from(x);
        let y_usize = usize::from(y);
        let width_usize = usize::from(self.dimensions.width.get().get());

        let row_offset = y_usize.checked_mul(width_usize)?;
        let pixel_index = row_offset.checked_add(x_usize)?;
        let byte_index = pixel_index.checked_mul(3)?;
        let end_index = byte_index.checked_add(3)?;

        self.data.get(byte_index..end_index)
    }

    /// Sets the pixel at `(x, y)` to `color`.
    ///
    /// Returns `None` if `(x, y)` is out of bounds, or `Some(())` on success.
    pub fn set_pixel(&mut self, x: u16, y: u16, color: Rgb8) -> Option<()> {
        if x >= self.dimensions.width.get().get() || y >= self.dimensions.height.get().get() {
            return None;
        }

        let x_usize = usize::from(x);
        let y_usize = usize::from(y);
        let width_usize = usize::from(self.dimensions.width.get().get());

        let row_offset = y_usize.checked_mul(width_usize)?;
        let pixel_index = row_offset.checked_add(x_usize)?;
        let byte_index = pixel_index.checked_mul(3)?;

        let r_idx = byte_index;
        let g_idx = byte_index.checked_add(1)?;
        let b_idx = byte_index.checked_add(2)?;

        *self.data.get_mut(r_idx)? = color.r;
        *self.data.get_mut(g_idx)? = color.g;
        *self.data.get_mut(b_idx)? = color.b;

        Some(())
    }

    /// Fills the entire frame with a uniform [`Rgb8`] color.
    pub fn fill(&mut self, color: Rgb8) {
        let (chunks, _) = self.data.as_chunks_mut::<3>();
        for pixel in chunks {
            *pixel = [color.r, color.g, color.b];
        }
    }

    /// Blends a pixel at `(x, y)` with an [`Rgb8`] color and alpha value.
    ///
    /// An `alpha` of 255 replaces the destination pixel entirely with `color`.
    /// An `alpha` of 0 leaves the destination pixel unchanged.
    /// Intermediate values linearly blend using integer arithmetic with symmetric rounding.
    ///
    /// Returns `None` if `(x, y)` is out of bounds, or `Some(())` if successfully blended.
    pub fn blend_pixel(&mut self, x: u16, y: u16, color: Rgb8, alpha: u8) -> Option<()> {
        if x >= self.dimensions.width.get().get() || y >= self.dimensions.height.get().get() {
            return None;
        }
        if alpha == 0 {
            return Some(());
        }

        let x_usize = usize::from(x);
        let y_usize = usize::from(y);
        let width_usize = usize::from(self.dimensions.width.get().get());

        let row_offset = y_usize.checked_mul(width_usize)?;
        let pixel_index = row_offset.checked_add(x_usize)?;
        let byte_index = pixel_index.checked_mul(3)?;

        let r_idx = byte_index;
        let g_idx = byte_index.checked_add(1)?;
        let b_idx = byte_index.checked_add(2)?;

        if alpha == 255 {
            *self.data.get_mut(r_idx)? = color.r;
            *self.data.get_mut(g_idx)? = color.g;
            *self.data.get_mut(b_idx)? = color.b;
            return Some(());
        }

        let dst_red = *self.data.get(r_idx)?;
        let dst_green = *self.data.get(g_idx)?;
        let dst_blue = *self.data.get(b_idx)?;

        *self.data.get_mut(r_idx)? = blend_channel(color.r, dst_red, alpha);
        *self.data.get_mut(g_idx)? = blend_channel(color.g, dst_green, alpha);
        *self.data.get_mut(b_idx)? = blend_channel(color.b, dst_blue, alpha);

        Some(())
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
            Frame::new(dims, too_short).is_none(),
            "buffer shorter than required must return None"
        );

        // Too long buffer must be rejected.
        let too_long = vec![0_u8; 37];
        assert!(
            Frame::new(dims, too_long).is_none(),
            "buffer longer than required must return None"
        );

        // Empty buffer must be rejected.
        assert!(
            Frame::new(dims, Vec::new()).is_none(),
            "empty buffer must return None"
        );

        // Exact length buffer must succeed.
        let exact = vec![0_u8; 36];
        let frame = Frame::new(dims, exact).expect("exact length buffer must return Some(Frame)");
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
                    color_frame.pixel(x, y),
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
        assert!(
            frame.set_pixel(1, 1, col).is_some(),
            "setting pixel within bounds must succeed"
        );
        assert_eq!(
            frame.pixel(1, 1),
            Some(col),
            "pixel at (1, 1) must return set color"
        );
        assert_eq!(
            frame.get_pixel(1, 1),
            Some(col),
            "get_pixel at (1, 1) must return set color"
        );
        assert_eq!(
            frame.pixel_bytes(1, 1),
            Some(&[10, 20, 30][..]),
            "pixel_bytes at (1, 1) must return [10, 20, 30]"
        );

        // Out of bounds: x == width
        assert!(
            frame.pixel(3, 0).is_none(),
            "pixel at x == width must return None"
        );
        assert!(
            frame.pixel_bytes(3, 0).is_none(),
            "pixel_bytes at x == width must return None"
        );
        assert!(
            frame.set_pixel(3, 0, col).is_none(),
            "set_pixel at x == width must return None"
        );

        // Out of bounds: y == height
        assert!(
            frame.pixel(0, 2).is_none(),
            "pixel at y == height must return None"
        );
        assert!(
            frame.set_pixel(0, 2, col).is_none(),
            "set_pixel at y == height must return None"
        );

        // Far out of bounds
        assert!(
            frame.pixel(u16::MAX, u16::MAX).is_none(),
            "pixel at u16::MAX must return None"
        );
        assert!(
            frame.pixel_bytes(u16::MAX, u16::MAX).is_none(),
            "pixel_bytes at u16::MAX must return None"
        );
        assert!(
            frame.set_pixel(u16::MAX, u16::MAX, col).is_none(),
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
                    frame.pixel(x, y),
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
        let _ = frame.set_pixel(0, 0, white);

        // Blend with black (0, 0, 0) at alpha 0 -> must remain white
        let black = Rgb8::new(0, 0, 0);
        assert!(
            frame.blend_pixel(0, 0, black, 0).is_some(),
            "blend at alpha 0 must succeed"
        );
        assert_eq!(
            frame.pixel(0, 0),
            Some(white),
            "pixel with alpha 0 blend must remain unchanged"
        );

        // Blend with red (255, 0, 0) at alpha 255 -> must become pure red
        let red = Rgb8::new(255, 0, 0);
        assert!(
            frame.blend_pixel(0, 0, red, 255).is_some(),
            "blend at alpha 255 must succeed"
        );
        assert_eq!(
            frame.pixel(0, 0),
            Some(red),
            "pixel with alpha 255 blend must become source color"
        );

        // Set pixel (1, 1) to (100, 100, 100) and blend with (200, 200, 200) at alpha 128
        let gray100 = Rgb8::new(100, 100, 100);
        let _ = frame.set_pixel(1, 1, gray100);
        let gray200 = Rgb8::new(200, 200, 200);
        assert!(
            frame.blend_pixel(1, 1, gray200, 128).is_some(),
            "blend at alpha 128 must succeed"
        );
        // Blend formula: (200 * 128 + 100 * 127 + 127) / 255 = (25600 + 12700 + 127) / 255 = 38427 / 255 = 150
        let blended = frame.pixel(1, 1).expect("pixel after blend must exist");
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
        assert!(
            frame.blend_pixel(2, 0, red, 255).is_none(),
            "out of bounds blend must return None"
        );
        assert!(
            frame.blend_pixel(u16::MAX, u16::MAX, red, 255).is_none(),
            "out of bounds blend at u16::MAX must return None"
        );
    }
}
