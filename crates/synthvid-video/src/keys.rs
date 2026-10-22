//! The closed set of keys a manifest document uses.
//!
//! Every key this crate writes is one of these 34. Modelling them as an
//! enum rather than validating a string at the point of use removes the only
//! runtime failure the serializer had: a hardcoded literal was checked against
//! the key alphabet and the check was unwrapped, so a typo in a literal was a
//! panic in a library crate. There is nothing left to check, because there is
//! nothing else a caller can pass.

use crate::json_names::JsonKey;

/// A key in a manifest document.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ManifestKey {
    /// The `a` key.
    AffineA,
    /// The `b` key.
    AffineB,
    /// The `background` key.
    Background,
    /// The `bbox` key.
    Bbox,
    /// The `c` key.
    AffineC,
    /// The `camera` key.
    Camera,
    /// The `capture_carrier` key.
    CaptureCarrier,
    /// The `capture_rate` key.
    CaptureRate,
    /// The `centre_screen` key.
    CentreScreen,
    /// The `centre_world` key.
    CentreWorld,
    /// The `d` key.
    AffineD,
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
    PointX,
    /// The `y` key.
    PointY,
}

impl ManifestKey {
    /// Every key, for the test that checks each one against the key alphabet.
    #[cfg(test)]
    const ALL: [Self; 35] = [
        Self::AffineA,
        Self::AffineB,
        Self::Background,
        Self::Bbox,
        Self::AffineC,
        Self::Camera,
        Self::CaptureCarrier,
        Self::CaptureRate,
        Self::CentreScreen,
        Self::CentreWorld,
        Self::AffineD,
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
        Self::PointX,
        Self::PointY,
    ];
}

impl From<ManifestKey> for JsonKey {
    /// Converts without the possibility of failure.
    ///
    /// [`JsonKey::new`] is fallible because it accepts arbitrary text. These
    /// do not, so this conversion cannot fail and nothing here can abort.
    fn from(key: ManifestKey) -> Self {
        Self::from_known(match key {
            ManifestKey::AffineA => "a",
            ManifestKey::AffineB => "b",
            ManifestKey::Background => "background",
            ManifestKey::Bbox => "bbox",
            ManifestKey::AffineC => "c",
            ManifestKey::Camera => "camera",
            ManifestKey::CaptureCarrier => "capture_carrier",
            ManifestKey::CaptureRate => "capture_rate",
            ManifestKey::CentreScreen => "centre_screen",
            ManifestKey::CentreWorld => "centre_world",
            ManifestKey::AffineD => "d",
            ManifestKey::Dimensions => "dimensions",
            ManifestKey::End => "end",
            ManifestKey::FrameCount => "frame_count",
            ManifestKey::FrameRate => "frame_rate",
            ManifestKey::Frames => "frames",
            ManifestKey::Height => "height",
            ManifestKey::Index => "index",
            ManifestKey::MaxX => "max_x",
            ManifestKey::MaxY => "max_y",
            ManifestKey::MinX => "min_x",
            ManifestKey::MinY => "min_y",
            ManifestKey::Name => "name",
            ManifestKey::Objects => "objects",
            ManifestKey::OnScreen => "on_screen",
            ManifestKey::Scale => "scale",
            ManifestKey::SchemaVersion => "schema_version",
            ManifestKey::Shape => "shape",
            ManifestKey::Start => "start",
            ManifestKey::Tx => "tx",
            ManifestKey::Ty => "ty",
            ManifestKey::Visible => "visible",
            ManifestKey::Width => "width",
            ManifestKey::PointX => "x",
            ManifestKey::PointY => "y",
        })
    }
}

#[cfg(test)]
mod tests;
