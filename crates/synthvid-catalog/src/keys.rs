//! The closed set of keys a manifest document uses.
//!
//! Every key this crate writes is one of these 33. Modelling them as an
//! enum rather than validating a string at the point of use removes the only
//! runtime failure the serialiser had: a hardcoded literal was checked against
//! the key alphabet and the check was unwrapped, so a typo in a literal was a
//! panic in a library crate. There is nothing left to check, because there is
//! nothing else a caller can pass.

use crate::json_names::JsonKey;

/// A key in a manifest document.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ManifestKey {
    /// The `a` key.
    A,
    /// The `b` key.
    B,
    /// The `background` key.
    Background,
    /// The `bbox` key.
    Bbox,
    /// The `c` key.
    C,
    /// The `camera` key.
    Camera,
    /// The `centre_screen` key.
    CentreScreen,
    /// The `centre_world` key.
    CentreWorld,
    /// The `d` key.
    D,
    /// The `dimensions` key.
    Dimensions,
    /// The `end` key.
    End,
    /// The `frame_count` key.
    FrameCount,
    /// The `frame_rate` key.
    FrameRate,
    /// The `frames` key.
    Frames,
    /// The `height` key.
    Height,
    /// The `index` key.
    Index,
    /// The `max_x` key.
    MaxX,
    /// The `max_y` key.
    MaxY,
    /// The `min_x` key.
    MinX,
    /// The `min_y` key.
    MinY,
    /// The `name` key.
    Name,
    /// The `objects` key.
    Objects,
    /// The `on_screen` key.
    OnScreen,
    /// The `scale` key.
    Scale,
    /// The `schema_version` key.
    SchemaVersion,
    /// The `shape` key.
    Shape,
    /// The `start` key.
    Start,
    /// The `tx` key.
    Tx,
    /// The `ty` key.
    Ty,
    /// The `visible` key.
    Visible,
    /// The `width` key.
    Width,
    /// The `x` key.
    X,
    /// The `y` key.
    Y,
}

impl ManifestKey {
    /// Returns the key as it appears in the document.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::Background => "background",
            Self::Bbox => "bbox",
            Self::C => "c",
            Self::Camera => "camera",
            Self::CentreScreen => "centre_screen",
            Self::CentreWorld => "centre_world",
            Self::D => "d",
            Self::Dimensions => "dimensions",
            Self::End => "end",
            Self::FrameCount => "frame_count",
            Self::FrameRate => "frame_rate",
            Self::Frames => "frames",
            Self::Height => "height",
            Self::Index => "index",
            Self::MaxX => "max_x",
            Self::MaxY => "max_y",
            Self::MinX => "min_x",
            Self::MinY => "min_y",
            Self::Name => "name",
            Self::Objects => "objects",
            Self::OnScreen => "on_screen",
            Self::Scale => "scale",
            Self::SchemaVersion => "schema_version",
            Self::Shape => "shape",
            Self::Start => "start",
            Self::Tx => "tx",
            Self::Ty => "ty",
            Self::Visible => "visible",
            Self::Width => "width",
            Self::X => "x",
            Self::Y => "y",
        }
    }

    /// Every key, for the test that checks each one against the key alphabet.
    #[cfg(test)]
    const ALL: [Self; 33] = [
        Self::A,
        Self::B,
        Self::Background,
        Self::Bbox,
        Self::C,
        Self::Camera,
        Self::CentreScreen,
        Self::CentreWorld,
        Self::D,
        Self::Dimensions,
        Self::End,
        Self::FrameCount,
        Self::FrameRate,
        Self::Frames,
        Self::Height,
        Self::Index,
        Self::MaxX,
        Self::MaxY,
        Self::MinX,
        Self::MinY,
        Self::Name,
        Self::Objects,
        Self::OnScreen,
        Self::Scale,
        Self::SchemaVersion,
        Self::Shape,
        Self::Start,
        Self::Tx,
        Self::Ty,
        Self::Visible,
        Self::Width,
        Self::X,
        Self::Y,
    ];
}

impl From<ManifestKey> for JsonKey {
    /// Converts without the possibility of failure.
    ///
    /// [`JsonKey::new`] is fallible because it accepts arbitrary text. These
    /// do not, so this conversion cannot fail and nothing here can abort.
    fn from(k: ManifestKey) -> Self {
        Self::from_known(k.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key really does satisfy the alphabet its own validator enforces.
    ///
    /// The conversion above skips that check, so this is what stands behind
    /// it. `as_str` is a match, so a new variant cannot be added without an
    /// arm; adding it here as well is what keeps this honest.
    #[test]
    fn test_every_key_satisfies_the_key_alphabet() {
        for k in ManifestKey::ALL {
            let s = k.as_str();
            assert!(
                JsonKey::new(s).is_ok(),
                "manifest key {s:?} does not match the key alphabet"
            );
            assert_eq!(
                JsonKey::from(k).as_str(),
                s,
                "conversion must preserve the key exactly"
            );
        }
    }

    /// No two keys collide, which a duplicated literal would cause.
    #[test]
    fn test_keys_are_distinct() {
        let mut seen = std::collections::BTreeSet::new();
        for k in ManifestKey::ALL {
            assert!(
                seen.insert(k.as_str()),
                "duplicate manifest key {:?}",
                k.as_str()
            );
        }
        assert_eq!(seen.len(), 33, "ALL must list every key exactly once");
    }
}
