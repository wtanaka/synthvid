//! An iris paired with the kept points of its own aperture.
//!
//! The points of an aperture are computed from the outline and the sampling grid of one iris. An
//! `IrisPoints` keeps that iris and those points together, and its fields are private, so the
//! only way to build one is `IrisPoints::new`, which computes the points from the iris it is
//! given. The points therefore always name the aperture of the iris they are paired with.

use crate::optics::Iris;

use super::aperture::{aperture_points, ApertureError, AperturePoints};

/// An iris and the kept points of its aperture, computed on the iris's own sampling grid.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct IrisPoints {
    /// The iris that the points were taken from.
    iris: Iris,
    /// The kept points of the aperture, in their fixed order. The points are boxed so that the
    /// size of a lens admission stays within the workspace limit on how far its variants may
    /// differ in size.
    points: Box<AperturePoints>,
}

impl IrisPoints {
    /// Pairs an iris with the kept points of its aperture, computed on the iris's outline and
    /// sampling grid.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`aperture_points`] for the outline and grid of the iris.
    pub(super) fn new(iris: Iris) -> Result<Self, ApertureError> {
        let points = aperture_points(iris.shape(), iris.samples())?;
        Ok(Self {
            iris,
            points: Box::new(points),
        })
    }

    /// Returns the iris that the points were taken from.
    #[must_use]
    pub(super) const fn iris(&self) -> Iris {
        self.iris
    }

    /// Returns the kept points of the aperture, which were computed on the iris's own grid.
    #[must_use]
    pub(super) fn points(&self) -> &AperturePoints {
        &self.points
    }
}

#[cfg(test)]
mod tests;
