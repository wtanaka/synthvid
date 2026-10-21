//! The byte collections this crate hands back: one encoded frame, and one
//! whole container file.
//!
//! Each is its own type so a frame cannot be passed where a container is
//! expected, and so no public signature names a bare `Vec<u8>`.

use synthvid_scene::Frame;

/// One frame's payload as a container stores it: a JPEG image, or the raw
/// RGB samples of a [`Frame`].
///
/// Built only by [`crate::encode_jpeg`] or [`Self::raw`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedFrame(pub(crate) Vec<u8>);

impl EncodedFrame {
    /// The frame's raw RGB samples, uncompressed.
    #[must_use]
    pub fn raw(frame: &Frame) -> Self {
        Self(frame.data().0.clone())
    }

    /// Returns the payload's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// The bytes of a whole container file, as a writer produced it or as a
/// defect left it.
///
/// Built only by [`crate::encode_avi`], [`crate::encode_iso`] or
/// [`crate::apply`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerFile(pub(crate) Vec<u8>);

impl ContainerFile {
    /// Returns the file's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
