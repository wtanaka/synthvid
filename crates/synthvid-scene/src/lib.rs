//! Scene description, motion, geometry, and rendering.
//!
//! This crate performs no input or output. It takes values and returns values.
//!
//! The crate is pre-release (version `0.0.x`): its public API may change in any
//! release without notice.
//!
//! # Layout
//!
//! This file is a façade. It declares the modules and re-exports their public
//! items so that callers depend on `synthvid_scene::Item` and never on the
//! module a given item happens to live in. It holds no code of its own, and
//! nothing should be added to it but a module declaration and its re-exports.
//!
//! Each module owns exactly one concern, and is listed here roughly in
//! dependency order:
//!
//! | Module     | Owns                                                    |
//! | ---------- | ------------------------------------------------------- |
//! | [`camera`] | camera rotation and magnification over frames         |
//! | [`ratio`]  | exact rational arithmetic                               |
//! | [`render`] | closed-form frame rendering                             |
//! | [`units`]  | frame indices and counts, dimensions, frame rate, seed  |
//! | [`rng`]    | deterministic pseudo-random number generation           |
//! | [`color`]  | pixel color, buffer sizing, channel blending            |
//! | [`frame`]  | owned RGB pixel buffers with a checked length invariant |
//! | [`frame_rates`] | playback and optional distinct capture frame rates      |
//! | [`trig`]   | platform-independent sine and cosine                    |
//! | [`geom`]   | points, vectors, and exact affine transforms            |
//! | [`motion`] | closed-form evaluation of motion over frames            |
//! | [`raster`] | drawing geometry onto a frame                           |
//! | [`scene`]  | declarative scene description                           |
//!
//! New work belongs in the module that owns its concern. A concern that fits
//! none of them is a new module, declared here alongside the others.
//! `ci/check-module-size.sh` enforces both halves of this: it caps every
//! source file's length, and rejects anything in this file but the
//! declarations and re-exports below.
//!
//! # Example
//!
//! ```rust
//! use synthvid_scene::{Numerator, Ratio, cos_turns, sin_turns};
//!
//! let zero = Ratio::from_integer(Numerator(0));
//! let one = Ratio::from_integer(Numerator(1));
//! let sum = match zero.checked_add(one) {
//!     Ok(value) => value,
//!     Err(_) => return,
//! };
//! let sine = match sin_turns(zero) {
//!     Ok(value) => value,
//!     Err(_) => return,
//! };
//! let cosine = match cos_turns(zero) {
//!     Ok(value) => value,
//!     Err(_) => return,
//! };
//! (sum, sine, cosine);
//! ```
#![forbid(unsafe_code)]

pub mod camera;
pub mod color;
pub mod frame;
pub mod frame_rates;
pub mod geom;
pub mod motion;
pub mod raster;
pub mod ratio;
pub mod render;
pub mod rng;
pub mod scene;
pub mod trig;
pub mod units;

#[cfg(test)]
pub mod testutil;

pub use crate::camera::{
    rotation_at, zoom_at, Camera, Magnification, MagnificationError, Rotation, Turns, Zoom,
};
pub use crate::color::{required_buffer_len, Alpha, Channel, Rgb8};
pub use crate::frame::{Frame, FrameError, PixelBuffer};
pub use crate::frame_rates::{
    CaptureCarrier, DistinctCapture, FrameRates, FrameRatesError, RatePair,
};
pub use crate::geom::{Affine, Point, Similarity, Vector};
pub use crate::motion::position_at;
pub use crate::raster::{
    cross_extent, disc_extent, draw_cross, draw_line, fill_disc, fill_polygon, fill_rect,
    line_extent, polygon_extent, rect_extent, Bounds, BoundsError, BoundsOrEmpty,
};
pub use crate::ratio::{Numerator, Overflow, Ratio};
pub use crate::render::{object_states, render_frame, render_into, ObjectState, RenderError};
pub use crate::rng::{BoundedDraw, DrawBound, RandomWord, Rng};
pub use crate::scene::{
    Background, BlobCount, BlobRadii, BlobRadius, Direction, FrameSpan, FrameSpanError, Motion,
    Object, Scale, ScaleError, Scene, Shape,
};
pub use crate::trig::{cos_turns, sin_turns};
pub use crate::units::{
    Dimensions, FrameCount, FrameCountError, FrameIndex, FrameRate, FrameRateError, Height,
    HeightError, ObjectIndex, Seed, Width, WidthError,
};
