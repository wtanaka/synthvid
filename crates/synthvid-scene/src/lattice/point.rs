//! Lattice coordinates and translations.

use crate::ratio::Overflow;

/// A coordinate in units of 1/256 of a sensel.
///
/// Every `i64` is a legal coordinate, so the field is public.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SubSensel(pub i64);

impl SubSensel {
    /// Adds two coordinates.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if the sum leaves the `i64` range.
    pub(super) fn checked_add(self, other: Self) -> Result<Self, Overflow> {
        Ok(Self(self.0.checked_add(other.0).ok_or(Overflow)?))
    }

    /// Subtracts one coordinate from another.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if the difference leaves the `i64` range.
    pub(super) fn checked_sub(self, other: Self) -> Result<Self, Overflow> {
        Ok(Self(self.0.checked_sub(other.0).ok_or(Overflow)?))
    }
}

/// A point on the lattice, with its across and down coordinates.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LatticePoint {
    /// The coordinate to the right of the origin.
    pub across: SubSensel,
    /// The coordinate below the origin.
    pub down: SubSensel,
}

/// A translation of the lattice, applied to both axes.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LatticeShift {
    /// The displacement to the right.
    pub across: SubSensel,
    /// The displacement downward.
    pub down: SubSensel,
}
