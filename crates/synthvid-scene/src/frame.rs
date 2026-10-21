//! Owned RGB pixel buffers.
//!
//! [`Frame`] pairs [`Dimensions`] with a byte buffer whose
//! length is guaranteed to be exactly `width * height * 3`. Every accessor is
//! bounds-checked and returns [`Option`], so no operation on a frame panics.

use core::error::Error;
use core::fmt;

use crate::color::{blend_rgb, required_buffer_len, Alpha, Channel, Rgb8};
use crate::ratio::Overflow;
use crate::units::{Dimensions, Height, Width};

/// A pixel coordinate with x and y components.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PixelCoord {
    /// The column of the pixel.
    column: PixelColumn,
    /// The row of the pixel.
    row: PixelRow,
}

/// The horizontal position of a pixel, kept apart from its row.
///
/// Every `u16` names a column; whether it lies inside a given frame is
/// checked where the frame is indexed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PixelColumn(pub u16);

/// The vertical position of a pixel, kept apart from its column.
///
/// Every `u16` names a row; whether it lies inside a given frame is
/// checked where the frame is indexed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PixelRow(pub u16);

impl PixelCoord {
    /// Creates a new pixel coordinate from its column and row.
    #[must_use]
    pub const fn new(column: PixelColumn, row: PixelRow) -> Self {
        Self { column, row }
    }
}

/// A flat index into a `Frame`'s pixels -- specifically, an index into the
/// `[u8; 3]` chunks `Frame::data.as_chunks::<3>()` produces, not a byte
/// offset into `Frame::data` itself. Kept distinct from a bare `usize` (and
/// private, so nothing outside this module can construct one except via
/// [`Frame::pixel_index`]) so that indexing `Frame::data` directly with one
/// -- which would read three times too far into the buffer -- is a type
/// error rather than a silent off-by-a-factor-of-3 bug.
#[derive(Debug, Clone, Copy)]
struct PixelIndex(usize);

/// One pixel's raw 24-bit RGB bytes, in on-buffer order (red, green, blue).
///
/// This is the [`Frame::data`] wire-format encoding of one pixel, distinct
/// from [`Rgb8`] (the decoded color value with named `red`/`green`/`blue` fields):
/// specifically "3 bytes as they sit in the buffer," which is also why
/// [`Frame::pixel_bytes`] returns this rather than a bare `[u8; 3]` that
/// reads identically to any other 3 bytes.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PixelBytes([u8; 3]);

impl PixelBytes {
    /// The underlying bytes, in on-buffer order (red, green, blue).
    #[must_use]
    pub const fn get(self) -> [u8; 3] {
        self.0
    }
}

impl From<PixelBytes> for [u8; 3] {
    fn from(bytes: PixelBytes) -> Self {
        bytes.0
    }
}

impl From<Rgb8> for PixelBytes {
    fn from(color: Rgb8) -> Self {
        Self([color.red.0, color.green.0, color.blue.0])
    }
}

impl From<PixelBytes> for Rgb8 {
    fn from(bytes: PixelBytes) -> Self {
        let [red, green, blue] = bytes.0;
        Self {
            red: Channel(red),
            green: Channel(green),
            blue: Channel(blue),
        }
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
    data: PixelBuffer,
}

/// The raw 24-bit RGB bytes of a [`Frame`], three per pixel in raster order
/// (red, green, blue).
///
/// Any byte vector can be held here; [`Frame::new`] is where its length is
/// checked against the frame's dimensions.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PixelBuffer(pub Vec<u8>);

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

impl Error for PixelOutOfBounds {}

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

impl Error for FrameError {}

// This crate addresses pixel buffers through `usize`, so it requires a
// target whose `usize` is at least 32 bits, true of every platform Rust
// supports as a hosted (`std`) target. A 16-bit target fails the build
// here rather than silently addressing the wrong byte.
#[cfg(target_pointer_width = "16")]
compile_error!("synthvid-scene assumes usize is at least 32 bits wide");

impl Frame {
    /// Creates a new [`Frame`] from [`Dimensions`] and an owned pixel buffer.
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameError::InvalidBufferSize)` if `data` does not hold exactly
    /// `width * height * 3` bytes, or if the required buffer length calculation overflows `usize`.
    pub fn new(dimensions: Dimensions, data: PixelBuffer) -> Result<Self, FrameError> {
        let expected_len = required_buffer_len(dimensions).ok_or(FrameError::InvalidBufferSize)?;
        if data.0.len() != expected_len.get() {
            return Err(FrameError::InvalidBufferSize);
        }
        Ok(Self { dimensions, data })
    }

    /// Creates a zero-initialized (black) [`Frame`] with the given dimensions.
    ///
    /// # Errors
    ///
    /// Returns `Err(FrameError::InvalidBufferSize)` if the required buffer length
    /// calculation overflows `usize`.
    pub fn zeroed(dimensions: Dimensions) -> Result<Self, FrameError> {
        let len = required_buffer_len(dimensions).ok_or(FrameError::InvalidBufferSize)?;
        let data = vec![0_u8; len.get()];
        Ok(Self {
            dimensions,
            data: PixelBuffer(data),
        })
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

    /// Returns the underlying raw RGB byte buffer.
    #[must_use]
    pub const fn data(&self) -> &PixelBuffer {
        &self.data
    }

    /// Returns the [`Rgb8`] pixel color at the given coordinate,
    /// or `None` if the coordinate is out of bounds.
    #[must_use]
    pub fn pixel(&self, coord: PixelCoord) -> Option<Rgb8> {
        self.pixel_bytes(coord).map(Rgb8::from)
    }

    /// Returns the [`Rgb8`] pixel color at the given coordinate,
    /// or `None` if the coordinate is out of bounds.
    ///
    /// Alias for [`Self::pixel`].
    #[must_use]
    pub fn get_pixel(&self, coord: PixelCoord) -> Option<Rgb8> {
        self.pixel(coord)
    }

    /// Returns the pixel's raw bytes at the given coordinate, or `None` if
    /// out of bounds.
    #[must_use]
    pub fn pixel_bytes(&self, coord: PixelCoord) -> Option<PixelBytes> {
        let index = self.pixel_index(coord)?;
        self.pixel_chunk(index)
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
        alpha: Alpha,
    ) -> Result<(), PixelOutOfBounds> {
        self.blend_pixel_inner(coord, color, alpha)
    }

    /// Writes the pixel, or an error if the coordinate lies outside the frame.
    fn set_pixel_inner(&mut self, coord: PixelCoord, color: Rgb8) -> Result<(), PixelOutOfBounds> {
        let index = self.pixel_index(coord).ok_or(PixelOutOfBounds)?;
        self.write_pixel(index, PixelBytes::from(color))
    }

    /// The flat index of `coord`'s pixel among this frame's `[u8; 3]`
    /// chunks, or `None` if `coord` lies outside the frame. Total: bounds
    /// are checked just below, and `width`/`height` are each at most
    /// `u16::MAX`, so `y * width + x` is at most `4_294_967_295`, which
    /// fits any `usize` this crate builds for (see the guard above).
    #[must_use]
    fn pixel_index(&self, coord: PixelCoord) -> Option<PixelIndex> {
        let width_u16 = self.dimensions.width.get().get();
        let height = self.dimensions.height.get().get();
        if coord.column.0 >= width_u16 || coord.row.0 >= height {
            return None;
        }
        let width = usize::from(width_u16);
        let row_start = match usize::from(coord.row.0).checked_mul(width) {
            Some(row_offset) => row_offset,
            None if width == 0 => 0,
            None => usize::MAX,
        };
        let index = match row_start.checked_add(usize::from(coord.column.0)) {
            Some(offset) => offset,
            None if coord.column.0 == 0 => row_start,
            None => usize::MAX,
        };
        Some(PixelIndex(index))
    }

    /// The pixel at `index`, as its raw bytes, or `None` if out of bounds.
    /// A `PixelIndex` counts `[u8; 3]` chunks, not bytes, so it is usable
    /// only here and in `write_pixel`.
    #[must_use]
    fn pixel_chunk(&self, index: PixelIndex) -> Option<PixelBytes> {
        let (pixels, _) = self.data.0.as_chunks::<3>();
        pixels.get(index.0).copied().map(PixelBytes)
    }

    /// Overwrites the pixel at `index` with `bytes`, or returns an error if the
    /// index is out of bounds. See [`Self::pixel_chunk`].
    fn write_pixel(
        &mut self,
        index: PixelIndex,
        bytes: PixelBytes,
    ) -> Result<(), PixelOutOfBounds> {
        let (pixels, _) = self.data.0.as_chunks_mut::<3>();
        let pixel = pixels.get_mut(index.0).ok_or(PixelOutOfBounds)?;
        *pixel = bytes.get();
        Ok(())
    }

    /// Fills the entire frame with a uniform [`Rgb8`] color.
    pub fn fill(&mut self, color: Rgb8) {
        let bytes = PixelBytes::from(color).get();
        let (chunks, _) = self.data.0.as_chunks_mut::<3>();
        for pixel in chunks {
            *pixel = bytes;
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
        alpha: Alpha,
    ) -> Result<(), PixelOutOfBounds> {
        let index = self.pixel_index(coord).ok_or(PixelOutOfBounds)?;
        if alpha.0 == 0 {
            return Ok(());
        }

        let blended = if alpha.0 == 255 {
            color
        } else {
            let dst = self.pixel_chunk(index).ok_or(PixelOutOfBounds)?;
            blend_rgb(color, alpha, Rgb8::from(dst)).map_err(|Overflow| PixelOutOfBounds)?
        };
        self.write_pixel(index, PixelBytes::from(blended))
    }
}

#[cfg(test)]
mod tests;
