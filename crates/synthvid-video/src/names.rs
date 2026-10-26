//! The closed sets of names a manifest may record.
//!
//! Each is an enum rather than a string because the set of legal values is
//! finite and known. As a `String` the field admitted every value except the
//! ones that are legal, and the serializer interpolated it into the document
//! without escaping, so a stray quote produced invalid JSON.

use core::fmt;

use crate::writer::ClosedName;

/// The text of a closed name, without quotes.
///
/// The field is private to this module, which holds the closed name enums and the literal text of
/// each variant. Code elsewhere in the crate displays the text but cannot
/// build a `ClosedText` from an arbitrary string, so text that needs an escape sequence never
/// reaches the writer.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClosedText(&'static str);

/// Writes the text without quotes.
impl fmt::Display for ClosedText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Which backdrop a scene declared.
///
/// A closed set of five, so it is an enum rather than a string. As a `String`
/// the field admitted every value except the five that are legal, and the
/// serializer interpolated it unescaped.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum BackgroundName {
    /// A single flat color.
    Solid,
    /// A two-color checkerboard.
    Checker,
    /// A linear ramp between two colors.
    Gradient,
    /// Scattered discs, seeded from the scene.
    Blobs,
    /// A ruled grid over a ground color.
    Grid,
}

/// Writes the name as it appears in a manifest.
impl fmt::Display for BackgroundName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Solid => "solid",
            Self::Checker => "checker",
            Self::Gradient => "gradient",
            Self::Blobs => "blobs",
            Self::Grid => "grid",
        })
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

/// Writes the name as it appears in a manifest.
impl fmt::Display for ShapeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Disc => "disc",
            Self::Rect => "rect",
            Self::Polygon => "polygon",
            Self::Cross => "cross",
        })
    }
}

/// Which kind a camera's medium or lens is.
///
/// A closed set of four. The manifest writes it as the `kind` of the object that describes the
/// medium or the optics.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum KindName {
    /// A digital sensor.
    Sensor,
    /// Photographic film.
    Film,
    /// A described lens.
    Lens,
    /// A flat camera's vignette.
    Vignette,
}

impl ClosedName for KindName {
    fn as_str(self) -> ClosedText {
        match self {
            Self::Sensor => ClosedText("sensor"),
            Self::Film => ClosedText("film"),
            Self::Lens => ClosedText("lens"),
            Self::Vignette => ClosedText("vignette"),
        }
    }
}

/// Writes the name as it appears in a manifest.
impl fmt::Display for KindName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(ClosedName::as_str(*self).0)
    }
}

/// A standard f-number of a lens aperture, as the manifest writes it.
///
/// The closed set of nine stops of the conventional series, each spelled with an underscore in
/// place of the decimal point.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ApertureName {
    /// f/1.4.
    F1_4,
    /// f/2.
    F2,
    /// f/2.8.
    F2_8,
    /// f/4.
    F4,
    /// f/5.6.
    F5_6,
    /// f/8.
    F8,
    /// f/11.
    F11,
    /// f/16.
    F16,
    /// f/22.
    F22,
}

impl ClosedName for ApertureName {
    fn as_str(self) -> ClosedText {
        match self {
            Self::F1_4 => ClosedText("f1_4"),
            Self::F2 => ClosedText("f2"),
            Self::F2_8 => ClosedText("f2_8"),
            Self::F4 => ClosedText("f4"),
            Self::F5_6 => ClosedText("f5_6"),
            Self::F8 => ClosedText("f8"),
            Self::F11 => ClosedText("f11"),
            Self::F16 => ClosedText("f16"),
            Self::F22 => ClosedText("f22"),
        }
    }
}

/// Writes the stop as it appears in a manifest.
impl fmt::Display for ApertureName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(ClosedName::as_str(*self).0)
    }
}

/// Whether a lens aperture is a pinhole or an iris.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ApertureKindName {
    /// A pinhole, which admits light through a single point.
    Pinhole,
    /// An iris, with a stop, an outline, and a focus.
    Iris,
}

impl ClosedName for ApertureKindName {
    fn as_str(self) -> ClosedText {
        match self {
            Self::Pinhole => ClosedText("pinhole"),
            Self::Iris => ClosedText("iris"),
        }
    }
}

/// Writes the kind as it appears in a manifest.
impl fmt::Display for ApertureKindName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(ClosedName::as_str(*self).0)
    }
}

/// The outline kind of a lens aperture: a round iris, or a bladed one.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ApertureShapeKindName {
    /// A round iris.
    Disc,
    /// A bladed iris.
    Polygon,
}

impl ClosedName for ApertureShapeKindName {
    fn as_str(self) -> ClosedText {
        match self {
            Self::Disc => ClosedText("disc"),
            Self::Polygon => ClosedText("polygon"),
        }
    }
}

/// Writes the kind as it appears in a manifest.
impl fmt::Display for ApertureShapeKindName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(ClosedName::as_str(*self).0)
    }
}

/// Where a lens is focused, when it is focused at infinity rather than at a distance.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum FocusName {
    /// Focused at infinity.
    Infinity,
}

impl ClosedName for FocusName {
    fn as_str(self) -> ClosedText {
        match self {
            Self::Infinity => ClosedText("infinity"),
        }
    }
}

/// Writes the focus as it appears in a manifest.
impl fmt::Display for FocusName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(ClosedName::as_str(*self).0)
    }
}

/// The flat outline of a deep shot's layer: a disc, a rectangle, or a polygon.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum CutoutName {
    /// A disc.
    Disc,
    /// An axis-aligned rectangle.
    Rect,
    /// A closed polygon.
    Polygon,
}

impl ClosedName for CutoutName {
    fn as_str(self) -> ClosedText {
        match self {
            Self::Disc => ClosedText("disc"),
            Self::Rect => ClosedText("rect"),
            Self::Polygon => ClosedText("polygon"),
        }
    }
}

/// Writes the outline as it appears in a manifest.
impl fmt::Display for CutoutName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(ClosedName::as_str(*self).0)
    }
}

/// The kind of a lens's vignette when the lens has none. A vignette with a strength is written
/// as [`KindName::Vignette`], as a flat camera's vignette is.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum VignetteKindName {
    /// No falloff.
    None,
}

impl ClosedName for VignetteKindName {
    fn as_str(self) -> ClosedText {
        match self {
            Self::None => ClosedText("none"),
        }
    }
}

/// Writes the kind as it appears in a manifest.
impl fmt::Display for VignetteKindName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(ClosedName::as_str(*self).0)
    }
}

/// The projection a deep shot is rendered with.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ProjectionName {
    /// A perspective projection from a pinhole-like camera.
    Perspective,
}

impl ClosedName for ProjectionName {
    fn as_str(self) -> ClosedText {
        match self {
            Self::Perspective => ClosedText("perspective"),
        }
    }
}

/// Writes the projection as it appears in a manifest.
impl fmt::Display for ProjectionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(ClosedName::as_str(*self).0)
    }
}

#[cfg(test)]
mod tests;
