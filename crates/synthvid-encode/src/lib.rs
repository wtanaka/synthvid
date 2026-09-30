//! Pixel coding and container writing.
//!
//! Every function here takes bytes or frames and returns bytes. Nothing here
//! touches the filesystem, the clock, or the environment; the same inputs
//! produce the same bytes on every platform.
//!
//! # Layout
//!
//! | Module | Owns                                                |
//! | ------ | --------------------------------------------------- |
//! | `jpeg` | motion-JPEG frame coding (`encode_jpeg`)            |
//! | `avi` | AVI container writing (`encode_avi`)                |
//! | `iso` | ISO base media container writing (`encode_iso`)     |
//! | `config` | JPEG coding parameters (`Quality`, `ChromaSampling`) |
//! | `defects` | single-fault mutations (`apply`)                 |
//! | `capacity` | classic-container size check                    |
//! | `color` | RGB to YCbCr conversion                             |
//!
//! Lower-level helpers (`bitstream`, `dct`, `huffman`, `quant`,
//! `sample_table`, `jpeg_markers`, `avi_helpers`, `iso_helpers`) are public
//! modules so the layers above can share them, but most callers only need
//! the re-exports below.
//!
//! # Example
//!
//! ```rust
//! use synthvid_encode::{ChromaSampling, Quality, encode_jpeg};
//! use synthvid_scene::{Dimensions, Frame, Height, Width};
//!
//! let dims = match (Width::new(16), Height::new(16)) {
//!     (Ok(width), Ok(height)) => Dimensions::new(width, height),
//!     _ => return,
//! };
//! let frame = match Frame::zeroed(dims) {
//!     Ok(frame) => frame,
//!     Err(_) => return,
//! };
//! let quality = match Quality::new(75) {
//!     Some(quality) => quality,
//!     None => return,
//! };
//! let bytes: Vec<u8> = encode_jpeg(&frame, quality, ChromaSampling::Yuv420);
//! bytes.len();
//! ```
#![forbid(unsafe_code)]

pub mod ac_encoding;
pub mod avi;
pub mod avi_helpers;
pub mod bitstream;
pub mod capacity;
pub mod color;
pub mod config;
pub mod dct;
pub mod defects;
pub mod huffman;
pub mod iso;
pub mod iso_helpers;
pub mod jpeg;
pub mod jpeg_markers;
pub mod quant;
pub mod sample_table;

pub use crate::avi::{encode_avi, AviCodec, AviError};
pub use crate::bitstream::BitCount;
pub use crate::capacity::raw_frames_fit_classic_container;
pub use crate::color::YCbCr;
pub use crate::config::{ChromaSampling, Quality};
pub use crate::defects::{apply, Axis, BoxPath, BoxTag, ByteOffset, Defect, DefectError};
pub use crate::huffman::Component;
pub use crate::iso::{encode_iso, IsoCodec, IsoError, TrackMatrix};
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
