//! The exact values a manifest records: ratios, points, boxes, transforms
//! and the on-screen fraction.
//!
//! Each keeps its fields private behind a constructor that takes a quantity
//! already validated by `synthvid-scene`, for the same reason as the rest of
//! the manifest: a state that cannot occur in a scene must not become
//! expressible merely by being copied into one of these structs.

use crate::keys::ManifestKey;
use crate::manifest::key;
use crate::writer::JsonObject;
use core::error::Error;
use core::fmt;
use synthvid_scene::{Affine, Bounds, Numerator, Point, Ratio};

/// A rational number as it appears in a manifest.
///
/// Always normalized with `gcd(|numer|, denom) == 1` and `denom > 0`, because
/// the only way to build one is from a [`Ratio`], which guarantees both.
///
/// Written as a JSON object `{"den":D,"num":N}`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestRatio(Ratio);

impl ManifestRatio {
    /// Creates a manifest ratio from a normalized [`Ratio`].
    #[must_use]
    pub const fn from_ratio(ratio: Ratio) -> Self {
        Self(ratio)
    }

    /// Returns the exact ratio.
    pub(crate) const fn ratio(self) -> Ratio {
        self.0
    }
}

/// Error returned when attempting to construct an invalid [`OnScreen`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OnScreenError {
    /// On-screen fraction must be within [0, 1].
    OutOfRange,
}

impl fmt::Display for OnScreenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange => write!(f, "on-screen fraction must be within [0, 1]"),
        }
    }
}

impl Error for OnScreenError {}

/// The fraction of an object's bounding box that lies inside the frame.
///
/// Strictly within `[0, 1]`. It is its own type because `1` and `0` are the
/// only interesting values and every number outside the interval is
/// meaningless; a bare ratio would let a manifest claim an object was five
/// times on screen.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct OnScreen(ManifestRatio);

impl OnScreen {
    /// Creates a visible fraction, rejecting anything outside `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns `Err(OnScreenError::OutOfRange)` when `value` is negative or greater than one.
    ///
    /// # Example
    ///
    /// ```rust
    /// use synthvid_video::manifest::OnScreen;
    /// use synthvid_scene::{Numerator, Ratio};
    ///
    /// let ok = OnScreen::new(Ratio::from_integer(Numerator(1))).is_ok();
    /// let bad = OnScreen::new(Ratio::from_integer(Numerator(2))).is_err();
    /// (ok, bad);
    /// ```
    pub fn new(value: Ratio) -> Result<Self, OnScreenError> {
        let zero = Ratio::from_integer(Numerator(0));
        let one = Ratio::from_integer(Numerator(1));
        (value >= zero && value <= one)
            .then(|| Self(ManifestRatio::from_ratio(value)))
            .ok_or(OnScreenError::OutOfRange)
    }

    /// Returns the exact fraction.
    pub(crate) const fn ratio(self) -> Ratio {
        self.0.ratio()
    }
}

/// A point in a manifest (screen or world space).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestPoint {
    /// Horizontal coordinate.
    horizontal: ManifestRatio,
    /// Vertical coordinate.
    vertical: ManifestRatio,
}

impl ManifestPoint {
    /// Creates a manifest point from an exact point.
    #[must_use]
    pub const fn from_point(point: Point) -> Self {
        Self {
            horizontal: ManifestRatio::from_ratio(point.across),
            vertical: ManifestRatio::from_ratio(point.down),
        }
    }

    /// Serializes this point as a JSON object for inclusion in a manifest.
    pub(crate) fn to_json_object(self) -> JsonObject {
        let mut obj = JsonObject::new();
        obj.insert_ratio(key(ManifestKey::PointX), self.horizontal.0);
        obj.insert_ratio(key(ManifestKey::PointY), self.vertical.0);
        obj
    }
}

/// An axis-aligned bounding box in a manifest.
///
/// Built only from a [`Bounds`], whose constructor has already rejected an
/// inverted box. Public corner fields would have let one back in.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestBounds {
    /// Smallest horizontal coordinate.
    min_x: ManifestRatio,
    /// Smallest vertical coordinate.
    min_y: ManifestRatio,
    /// Largest horizontal coordinate.
    max_x: ManifestRatio,
    /// Largest vertical coordinate.
    max_y: ManifestRatio,
}

impl ManifestBounds {
    /// Creates manifest bounds from an already-ordered [`Bounds`].
    #[must_use]
    pub const fn from_bounds(bounds: Bounds) -> Self {
        Self {
            min_x: ManifestRatio::from_ratio(bounds.min_x),
            min_y: ManifestRatio::from_ratio(bounds.min_y),
            max_x: ManifestRatio::from_ratio(bounds.max_x),
            max_y: ManifestRatio::from_ratio(bounds.max_y),
        }
    }

    /// Serializes these bounds as a JSON object for inclusion in a manifest.
    pub(crate) fn to_json_object(self) -> JsonObject {
        let mut obj = JsonObject::new();
        obj.insert_ratio(key(ManifestKey::MaxX), self.max_x.0);
        obj.insert_ratio(key(ManifestKey::MaxY), self.max_y.0);
        obj.insert_ratio(key(ManifestKey::MinX), self.min_x.0);
        obj.insert_ratio(key(ManifestKey::MinY), self.min_y.0);
        obj
    }
}

/// A scene-to-screen transform in a manifest.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestAffine {
    /// Linear entry mapping input `x` to output `x`.
    x_from_x: ManifestRatio,
    /// Linear entry mapping input `y` to output `x`.
    x_from_y: ManifestRatio,
    /// Translation added to output `x`.
    tx: ManifestRatio,
    /// Linear entry mapping input `x` to output `y`.
    y_from_x: ManifestRatio,
    /// Linear entry mapping input `y` to output `y`.
    y_from_y: ManifestRatio,
    /// Translation added to output `y`.
    ty: ManifestRatio,
}

impl ManifestAffine {
    /// Creates a manifest transform from an [`Affine`].
    #[must_use]
    pub const fn from_affine(affine: &Affine) -> Self {
        Self {
            x_from_x: ManifestRatio::from_ratio(affine.xx),
            x_from_y: ManifestRatio::from_ratio(affine.xy),
            tx: ManifestRatio::from_ratio(affine.tx),
            y_from_x: ManifestRatio::from_ratio(affine.yx),
            y_from_y: ManifestRatio::from_ratio(affine.yy),
            ty: ManifestRatio::from_ratio(affine.ty),
        }
    }

    /// Serializes this transform as a JSON object for inclusion in a manifest.
    pub(crate) fn to_json_object(self) -> JsonObject {
        let mut obj = JsonObject::new();
        obj.insert_ratio(key(ManifestKey::AffineA), self.x_from_x.0);
        obj.insert_ratio(key(ManifestKey::AffineB), self.x_from_y.0);
        obj.insert_ratio(key(ManifestKey::AffineC), self.y_from_x.0);
        obj.insert_ratio(key(ManifestKey::AffineD), self.y_from_y.0);
        obj.insert_ratio(key(ManifestKey::Tx), self.tx.0);
        obj.insert_ratio(key(ManifestKey::Ty), self.ty.0);
        obj
    }
}
