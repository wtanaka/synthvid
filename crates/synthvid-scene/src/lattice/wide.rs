//! Exact 128-bit intermediates for the lattice predicates.
//!
//! A lattice coordinate is an `i64`, so a difference of two coordinates needs
//! 65 bits, and a product of two such differences can need 130 bits. Every
//! operation here is checked on `i128`, and a result that does not fit becomes
//! [`Overflow`] rather than a wrapped value.

use crate::ratio::Overflow;

use super::point::{LatticePoint, LatticeShift, SubSensel};
use super::shape::DiscRadius;

/// An exact intermediate of lattice arithmetic, held in 128 bits.
///
/// Every `i128` is a legal intermediate, so the field is public to the module.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct Wide(pub(super) i128);

impl Wide {
    /// Widens a lattice coordinate without loss.
    pub(super) fn of(coordinate: SubSensel) -> Self {
        Self(i128::from(coordinate.0))
    }

    /// Widens a disc radius without loss.
    pub(super) fn of_radius(radius: DiscRadius) -> Self {
        Self(i128::from(radius.0))
    }

    /// Returns the zero intermediate.
    pub(super) const fn zero() -> Self {
        Self(0)
    }

    /// Returns `self - other`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if the difference leaves the `i128` range.
    pub(super) fn difference(self, other: Self) -> Result<Self, Overflow> {
        Ok(Self(self.0.checked_sub(other.0).ok_or(Overflow)?))
    }

    /// Returns `self + other`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if the sum leaves the `i128` range.
    pub(super) fn sum(self, other: Self) -> Result<Self, Overflow> {
        Ok(Self(self.0.checked_add(other.0).ok_or(Overflow)?))
    }

    /// Returns `self * other`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if the product leaves the `i128` range.
    pub(super) fn product(self, other: Self) -> Result<Self, Overflow> {
        Ok(Self(self.0.checked_mul(other.0).ok_or(Overflow)?))
    }
}

/// A displacement between two lattice points, with 128-bit components.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct WideVector {
    /// The across component.
    pub(super) across: Wide,
    /// The down component.
    pub(super) down: Wide,
}

impl WideVector {
    /// Returns the two-dimensional cross product of `self` and `other`.
    ///
    /// The result is `self.across * other.down - self.down * other.across`.
    /// Its sign is the side of `self` on which `other` lies, and it is zero
    /// exactly when the two displacements are parallel.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if an intermediate product leaves the `i128` range.
    pub(super) fn cross(self, other: Self) -> Result<Wide, Overflow> {
        let forward = self.across.product(other.down)?;
        let backward = self.down.product(other.across)?;
        forward.difference(backward)
    }
}

impl LatticePoint {
    /// Returns the point moved back by `shift` on both axes, which is `self - shift`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if either coordinate leaves the `i64` range.
    pub fn less(self, shift: LatticeShift) -> Result<Self, Overflow> {
        Ok(Self {
            across: self.across.checked_sub(shift.across)?,
            down: self.down.checked_sub(shift.down)?,
        })
    }

    /// Returns the displacement from this point to `end`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if a component leaves the `i128` range.
    pub(super) fn displacement_to(self, end: Self) -> Result<WideVector, Overflow> {
        Ok(WideVector {
            across: Wide::of(end.across).difference(Wide::of(self.across))?,
            down: Wide::of(end.down).difference(Wide::of(self.down))?,
        })
    }
}
