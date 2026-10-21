//! The parts a render produces, each named by what it is.
//!
//! The manifest text, the media bytes and the encoded frames are all byte
//! collections. Wrapping each in its own type stops one from being handed where
//! another is expected, and keeps the rendering pipeline's signatures free of
//! bare `String` and `Vec<u8>`.

use synthvid_encode::{ContainerFile, EncodedFrame};

/// The canonical JSON text of a manifest.
///
/// Built only inside this crate, by serializing a [`Manifest`](crate::Manifest):
/// text from anywhere else cannot pose as a manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestJson(pub(crate) String);

impl ManifestJson {
    /// Returns the manifest text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The bytes of a media container.
///
/// Built only inside this crate, by a render.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaBytes(pub(crate) ContainerFile);

impl MediaBytes {
    /// Returns the container's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

/// The encoded bytes of each frame, in frame order.
#[derive(Clone, Debug)]
pub(crate) struct EncodedFrames(pub(crate) Vec<EncodedFrame>);
