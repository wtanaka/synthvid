//! Pixel coding and container writing.
//!
//! Every function here takes bytes or frames and returns bytes.
#![forbid(unsafe_code)]

pub mod ac_encoding;
pub mod bitstream;
pub mod color;
pub mod config;
pub mod dct;
pub mod huffman;
pub mod jpeg;
pub mod jpeg_markers;
pub mod quant;

pub use crate::bitstream::BitCount;
pub use crate::color::YCbCr;
pub use crate::config::{ChromaSampling, Quality};
pub use crate::huffman::Component;
pub use crate::jpeg::encode_jpeg;

#[cfg(test)]
mod tests {
    use synthvid_scene::{Dimensions, Frame, Height, Width};

    /// Pins the cross-crate invariant `encode_jpeg`'s pixel-plane
    /// construction relies on but does not itself check: `Frame::new`
    /// only ever hands out a `Frame` whose `data` length exactly matches
    /// `width * height * 3`. If that invariant were ever loosened,
    /// `build_ycbcr_planes` would silently pad a short buffer's tail
    /// with each plane's neutral fill value (or silently drop a long
    /// buffer's excess), producing a self-consistent, fully decodable,
    /// but silently corrupted JPEG -- the exact failure signature of
    /// several real bugs already found and fixed in this crate. This
    /// test does not exercise `synthvid-encode` code at all; it exists
    /// solely to fail loudly if `synthvid_scene::Frame` ever stops
    /// enforcing the length it currently enforces.
    #[test]
    fn test_frame_rejects_mismatched_data_length() {
        let width = Width::new(4).expect("valid width");
        let height = Height::new(4).expect("valid height");
        let dims = Dimensions::new(width, height);

        let correct_len = 4 * 4 * 3;
        let too_short = vec![0_u8; correct_len - 1];
        let too_long = vec![0_u8; correct_len + 1];
        let correct = vec![0_u8; correct_len];

        assert!(
            Frame::new(dims, too_short).is_err(),
            "Frame::new must reject a buffer shorter than width*height*3"
        );
        assert!(
            Frame::new(dims, too_long).is_err(),
            "Frame::new must reject a buffer longer than width*height*3"
        );
        assert!(
            Frame::new(dims, correct).is_ok(),
            "Frame::new must accept a buffer of exactly width*height*3 bytes"
        );
    }
}
