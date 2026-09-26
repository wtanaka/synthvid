//! Intentional structural defects in video containers.
//!
//! This module applies one declared corruption to an encoded ISO/MP4 or
//! AVI/RIFF container, so the corpus holds malformed files whose fault is
//! known exactly.
//!
//! # Box Path Format
//!
//! `BoxPath` represents a hierarchical path through a container's box tree:
//! - For ISO boxes: e.g., `["moov", "trak", "tkhd"]`
//! - For AVI chunks: e.g., `["hdrl", "avih"]` or `["strl", "strh"]`
//! - For AVI LIST chunks: the path walks through the subtype as a child (e.g., `["hdrl"]` targets the LIST hdrl)

use core::fmt;

/// A 4-byte box or chunk tag (e.g. `moov`, `ftyp`, `hdrl`).
///
/// Appears literally in the file and may contain arbitrary byte values.
/// Using a named type instead of bare `[u8; 4]` ensures type safety
/// across multiple box-tag storage locations and provides a shared
/// `Display` implementation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BoxTag([u8; 4]);

impl BoxTag {
    /// Creates a tag from its 4 raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    /// Gets the tag's 4 raw bytes.
    #[must_use]
    pub const fn get(self) -> [u8; 4] {
        self.0
    }
}

impl fmt::Display for BoxTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}

#[path = "defects_apply.rs"]
mod defects_apply;
#[path = "defects_box.rs"]
mod defects_box;
#[path = "defects_reorder.rs"]
mod defects_reorder;

use defects_apply::{
    apply_absurd_frame_rate, apply_declared_count_mismatch, apply_duplicate_box,
    apply_non_monotonic_timestamps, apply_offset_past_end, apply_oversized_box_length,
    apply_reordered_boxes, apply_truncate_at, apply_truncate_box, apply_undersized_box_length,
    apply_unknown_box, apply_zero_dimension, apply_zero_frame_rate, apply_zero_length_payload,
};
use defects_box::ContainerBytes;

/// A path through a box tree from file root, e.g. `["moov", "trak", "tkhd"]`.
///
/// Ensures paths are built compositionally and never hold invalid tag values.
/// Paths work for both ISO and AVI formats: the format is auto-detected and the path
/// is interpreted according to the format's structure.
///
/// # Examples
///
/// ISO paths (big-endian format):
/// - `["moov", "trak", "tkhd"]` for track header
/// - `["moov", "trak", "mdia", "minf", "stbl", "stsz"]` for sample sizes
///
/// AVI paths (little-endian RIFF format with LIST chunks as containers):
/// - `["hdrl", "avih"]` for AVI header (hdrl is a LIST chunk subtype)
/// - `["hdrl", "strl", "strh"]` for stream header (both hdrl and strl are LIST subtypes)
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BoxPath {
    /// Tags in the path from root to target
    tags: Vec<BoxTag>,
}

impl BoxPath {
    /// Creates a new path starting from the file root.
    #[must_use]
    pub const fn root() -> Self {
        Self { tags: Vec::new() }
    }

    /// Appends a child box tag to this path.
    #[must_use]
    pub fn child(mut self, tag: BoxTag) -> Self {
        self.tags.push(tag);
        self
    }

    /// Creates a path directly from tags (for testing and internal use).
    #[must_use]
    pub const fn from_tags(tags: Vec<BoxTag>) -> Self {
        Self { tags }
    }

    /// Returns the tags in this path.
    pub(crate) fn tags(&self) -> &[BoxTag] {
        &self.tags
    }
}

impl fmt::Display for BoxPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/")?;
        for (i, tag) in self.tags.iter().enumerate() {
            if i > 0 {
                write!(f, "/")?;
            }
            write!(f, "{tag}")?;
        }
        Ok(())
    }
}

/// A byte offset within a file.
///
/// Wraps a `usize` for memory addressing, valid by construction.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct ByteOffset(usize);

impl ByteOffset {
    /// Creates a byte offset.
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Gets the byte offset value.
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Dimensions axis: width or height.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Axis {
    /// Frame width
    Width,
    /// Frame height
    Height,
}

/// A defect to introduce into a container.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Defect {
    /// Truncate the file at a specific byte offset.
    TruncateAt(ByteOffset),
    /// Truncate the file right after the box at this path.
    TruncateBox(BoxPath),
    /// Make the payload of a box zero-length by writing size=8.
    ZeroLengthPayload(BoxPath),
    /// Write an oversized box length (size + 256).
    OversizedBoxLength(BoxPath),
    /// Write an undersized box length (size - 1).
    UndersizedBoxLength(BoxPath),
    /// Write an offset that points past the end of the file.
    OffsetPastEnd(BoxPath),
    /// Write the same box tag twice (box duplication).
    DuplicateBox(BoxPath),
    /// Introduce an unknown box tag after a specific parent box.
    UnknownBox {
        /// Parent box path
        after: BoxPath,
        /// Unknown tag bytes
        tag: BoxTag,
    },
    /// Set frame rate numerator to zero.
    ZeroFrameRate,
    /// Set a frame dimension (width or height) to zero.
    ZeroDimension(Axis),
    /// Set frame rate to an unreasonable value (numerator > 1000000).
    AbsurdFrameRate(synthvid_scene::FrameRate),
    /// Make timestamps non-monotonically increasing.
    NonMonotonicTimestamps,
    /// Mismatch between declared frame count and actual frames.
    DeclaredCountMismatch {
        /// Declared frame count
        declared: u32,
    },
    /// Write boxes in wrong order.
    ReorderedBoxes(BoxPath, BoxPath),
}

/// Error type for defect application.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DefectError {
    /// Box not found at the given path.
    BoxNotFound(BoxPath),
    /// Offset points past the end of the file.
    OffsetOutOfRange,
    /// Invalid box structure or format.
    InvalidBoxStructure,
    /// Cannot apply defect to this file type.
    IncompatibleFileType,
    /// Size calculation overflow.
    SizeOverflow,
}

impl fmt::Display for DefectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoxNotFound(path) => write!(f, "box not found at {path}"),
            Self::OffsetOutOfRange => write!(f, "offset points past end of file"),
            Self::InvalidBoxStructure => write!(f, "invalid box structure"),
            Self::IncompatibleFileType => write!(f, "incompatible file type"),
            Self::SizeOverflow => write!(f, "size calculation overflow"),
        }
    }
}

impl core::error::Error for DefectError {}

/// Applies a defect to the input buffer.
///
/// Takes ownership of the input, applies the defect, and returns the modified buffer.
/// Returns an error if the defect cannot be applied.
///
/// # Errors
///
/// Returns `DefectError` if the defect targets a structure that doesn't exist
/// or if the file format is not recognized.
pub fn apply(defect: &Defect, input: &[u8]) -> Result<Vec<u8>, DefectError> {
    let container = ContainerBytes::new(input);
    match defect {
        Defect::TruncateAt(offset) => apply_truncate_at(*offset, container),
        Defect::TruncateBox(path) => apply_truncate_box(path, container),
        Defect::ZeroLengthPayload(path) => apply_zero_length_payload(path, container),
        Defect::OversizedBoxLength(path) => apply_oversized_box_length(path, container),
        Defect::UndersizedBoxLength(path) => apply_undersized_box_length(path, container),
        Defect::OffsetPastEnd(path) => apply_offset_past_end(path, container),
        Defect::DuplicateBox(path) => apply_duplicate_box(path, container),
        Defect::UnknownBox { after, tag } => apply_unknown_box(after, *tag, container),
        Defect::ZeroFrameRate => apply_zero_frame_rate(container),
        Defect::ZeroDimension(axis) => apply_zero_dimension(*axis, container),
        Defect::AbsurdFrameRate(frame_rate) => apply_absurd_frame_rate(frame_rate, container),
        Defect::NonMonotonicTimestamps => apply_non_monotonic_timestamps(container),
        Defect::DeclaredCountMismatch { declared } => {
            apply_declared_count_mismatch(*declared, container)
        }
        Defect::ReorderedBoxes(path1, path2) => apply_reordered_boxes(path1, path2, container),
    }
}

#[cfg(test)]
#[path = "defects_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "defects_tests2.rs"]
mod tests2;
