//! Pixel coding and container writing.
//!
//! Every function here takes bytes or frames and returns bytes. Nothing here
//! touches the filesystem, the clock, or the environment; the same inputs
//! produce the same bytes on every platform within one release.
//!
//! The crate is pre-release (version `0.0.x`): its public API, including how a
//! defect is addressed, and the bytes it writes may change in any release
//! without notice.
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
//! | `media` | encoded frames and container files (`EncodedFrame`, `ContainerFile`) |
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
//! use synthvid_encode::{ChromaSampling, EncodedFrame, Quality, encode_jpeg};
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
//! let jpeg: EncodedFrame = encode_jpeg(&frame, quality, ChromaSampling::Yuv420);
//! jpeg.as_bytes().len();
//! ```
#![forbid(unsafe_code)]

pub mod ac_encoding;
pub mod avi;
pub mod avi_helpers;
pub mod bitstream;
pub mod byte_views;
pub mod capacity;
pub mod color;
pub mod config;
pub mod dct;
pub mod defects;
pub mod huffman;
pub mod iso;
pub mod iso_helpers;
pub mod iso_types;
pub mod jpeg;
pub mod jpeg_markers;
pub mod media;
pub mod quant;
pub mod sample_table;

pub use crate::avi::{encode_avi, AviCodec, AviError};
pub use crate::bitstream::BitCount;
pub use crate::capacity::{raw_frames_fit_classic_container, ContainerFit};
pub use crate::color::YCbCr;
pub use crate::config::{ChromaSampling, Quality};
pub use crate::defects::{
    apply, Axis, BoxAddress, BoxPath, BoxStep, BoxTag, ContainerKind, DeclaredFrameCount, Defect,
    DefectError,
};
pub use crate::huffman::Component;
pub use crate::iso::{encode_iso, IsoCodec, IsoError, TrackMatrix};
pub use crate::iso_helpers::TrackParams;
pub use crate::jpeg::encode_jpeg;
pub use crate::media::{ContainerFile, EncodedFrame};

#[cfg(test)]
mod iso_capture_tests;
#[cfg(test)]
mod tests;
