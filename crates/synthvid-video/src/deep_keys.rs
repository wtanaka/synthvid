//! The closed set of keys that a deep shot's manifest adds to those of `ManifestKey`.
//!
//! A deep manifest writes its backdrop, its layers' colors and outlines, its camera roll, and
//! the depth of each position with keys that the shared keys do not spell. They are kept here,
//! apart from [`ManifestKey`](crate::keys::ManifestKey), so that each closed set stays small
//! enough to read at once. Like those keys, every one converts to its spelling without any
//! possibility of failure.

use crate::json_names::JsonKey;

/// A key in a deep shot's manifest that is not a [`ManifestKey`](crate::keys::ManifestKey).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum DeepKey {
    /// The `backdrop` key.
    Backdrop,
    /// The `blue` key.
    Blue,
    /// The `fill` key.
    Fill,
    /// The `frame_height` key.
    FrameHeight,
    /// The `green` key.
    Green,
    /// The `half_height` key.
    HalfHeight,
    /// The `half_width` key.
    HalfWidth,
    /// The `pixel_samples` key.
    PixelSamples,
    /// The `projection` key.
    Projection,
    /// The `red` key.
    Red,
    /// The `roll` key.
    Roll,
    /// The `samples` key.
    Samples,
    /// The `stop` key.
    Stop,
    /// The `vertices` key.
    Vertices,
    /// The `z` key.
    PointZ,
}

impl DeepKey {
    /// Every key, for the test that checks each one against the key alphabet.
    #[cfg(test)]
    const ALL: [Self; 15] = [
        Self::Backdrop,
        Self::Blue,
        Self::Fill,
        Self::FrameHeight,
        Self::Green,
        Self::HalfHeight,
        Self::HalfWidth,
        Self::PixelSamples,
        Self::Projection,
        Self::Red,
        Self::Roll,
        Self::Samples,
        Self::Stop,
        Self::Vertices,
        Self::PointZ,
    ];
}

impl From<DeepKey> for JsonKey {
    /// Converts without the possibility of failure.
    ///
    /// [`JsonKey::new`] is fallible because it accepts arbitrary text. These
    /// do not, so this conversion cannot fail and nothing here can abort.
    fn from(key: DeepKey) -> Self {
        Self::from_known(match key {
            DeepKey::Backdrop => "backdrop",
            DeepKey::Blue => "blue",
            DeepKey::Fill => "fill",
            DeepKey::FrameHeight => "frame_height",
            DeepKey::Green => "green",
            DeepKey::HalfHeight => "half_height",
            DeepKey::HalfWidth => "half_width",
            DeepKey::PixelSamples => "pixel_samples",
            DeepKey::Projection => "projection",
            DeepKey::Red => "red",
            DeepKey::Roll => "roll",
            DeepKey::Samples => "samples",
            DeepKey::Stop => "stop",
            DeepKey::Vertices => "vertices",
            DeepKey::PointZ => "z",
        })
    }
}

#[cfg(test)]
mod tests;
