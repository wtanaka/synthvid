//! The closed sets of names a manifest may record.
//!
//! Each is an enum rather than a string because the set of legal values is
//! finite and known. As a `String` the field admitted every value except the
//! ones that are legal, and the serialiser interpolated it into the document
//! without escaping, so a stray quote produced invalid JSON.

/// Which backdrop a scene declared.
///
/// A closed set of five, so it is an enum rather than a string. As a `String`
/// the field admitted every value except the five that are legal, and the
/// serialiser interpolated it unescaped.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum BackgroundName {
    /// A single flat colour.
    Solid,
    /// A two-colour chequerboard.
    Checker,
    /// A linear ramp between two colours.
    Gradient,
    /// Scattered discs, seeded from the scene.
    Blobs,
    /// A ruled grid over a ground colour.
    Grid,
}

impl BackgroundName {
    /// Returns the name as it appears in a manifest.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Checker => "checker",
            Self::Gradient => "gradient",
            Self::Blobs => "blobs",
            Self::Grid => "grid",
        }
    }
}

/// Which shape an object is.
///
/// A closed set of four, for the same reason as [`BackgroundName`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ShapeName {
    /// A disc.
    Disc,
    /// An axis-aligned rectangle.
    Rect,
    /// A closed polygon.
    Polygon,
    /// An axis-aligned cross.
    Cross,
}

impl ShapeName {
    /// Returns the name as it appears in a manifest.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disc => "disc",
            Self::Rect => "rect",
            Self::Polygon => "polygon",
            Self::Cross => "cross",
        }
    }
}
