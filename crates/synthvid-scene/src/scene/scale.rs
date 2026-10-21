//! Scale: how many scene units correspond to one world unit.

use core::error::Error;
use core::fmt;

use crate::ratio::Ratio;

/// Error returned when a [`Scale`] cannot be constructed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ScaleError {
    /// Scale must be strictly positive.
    NonPositive,
}

impl fmt::Display for ScaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositive => write!(f, "scale must be strictly positive"),
        }
    }
}

impl Error for ScaleError {}

/// Declares how many scene units correspond to one world unit.
///
/// A scene is laid out in scene units, which coincide with pixel units. The
/// optional [`crate::scene::Scene::scale`] gives the layout an exact physical
/// interpretation, so a manifest can report real distances rather than only
/// pixels. The value must be strictly positive.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Scale(Ratio);

impl Scale {
    /// Creates a scale from a [`Ratio`], rejecting zero and negative values.
    ///
    /// # Errors
    ///
    /// Returns `Err(ScaleError::NonPositive)` when `units_per_world_unit` is zero or negative.
    pub const fn new(units_per_world_unit: Ratio) -> Result<Self, ScaleError> {
        if units_per_world_unit.numer().0 > 0 {
            Ok(Self(units_per_world_unit))
        } else {
            Err(ScaleError::NonPositive)
        }
    }

    /// Creates a scale from a [`Ratio`], rejecting zero and negative values.
    ///
    /// # Errors
    ///
    /// Returns `Err(ScaleError::NonPositive)` when `units_per_world_unit` is zero or negative.
    pub const fn from_ratio(units_per_world_unit: Ratio) -> Result<Self, ScaleError> {
        Self::new(units_per_world_unit)
    }

    /// Returns the underlying [`Ratio`] of scene units per world unit.
    #[must_use]
    pub const fn get(self) -> Ratio {
        self.0
    }

    /// Returns the underlying [`Ratio`] of scene units per world unit.
    #[must_use]
    pub const fn ratio(self) -> Ratio {
        self.0
    }
}

impl TryFrom<Ratio> for Scale {
    type Error = ScaleError;

    fn try_from(units_per_world_unit: Ratio) -> Result<Self, Self::Error> {
        Self::new(units_per_world_unit)
    }
}
