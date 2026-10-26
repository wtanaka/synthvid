//! Flat shapes on the lattice and their bounding boxes.

use crate::ratio::Overflow;

use super::error::LatticeError;
use super::point::{LatticePoint, SubSensel};

/// The radius of a disc in lattice units.
///
/// Every `u64` is a legal radius, so the field is public. A radius above
/// `i64::MAX` makes [`LatticeShape::bounds`] fail, since the bounding box is
/// held in the lattice's `i64` coordinates. Containment is exact while the
/// square of the radius fits in `i128`, and reports an overflow beyond that.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct DiscRadius(pub u64);

/// A closed polygon with at least three vertices, listed in boundary order.
///
/// The first three vertices are held apart from the rest, so a polygon with
/// fewer than three vertices has no value to be built from. The fields are
/// private, so the only way to build one is [`LatticeShape::polygon`]. The
/// region it encloses is decided by the even-odd rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatticePolygon {
    /// The first three vertices, in boundary order.
    head: [LatticePoint; 3],
    /// Every vertex after the third, in boundary order.
    tail: Vec<LatticePoint>,
}

impl LatticePolygon {
    /// Returns every vertex in boundary order.
    pub(super) fn vertices(&self) -> impl Iterator<Item = LatticePoint> + '_ {
        self.head.iter().chain(self.tail.iter()).copied()
    }

    /// Returns the smallest inclusive box that contains every vertex.
    fn bounds(&self) -> LatticeBounds {
        let [first, ..] = self.head;
        let mut low = first;
        let mut high = first;
        for vertex in self.vertices() {
            low = LatticePoint {
                across: low.across.min(vertex.across),
                down: low.down.min(vertex.down),
            };
            high = LatticePoint {
                across: high.across.max(vertex.across),
                down: high.down.max(vertex.down),
            };
        }
        LatticeBounds { low, high }
    }
}

/// The inclusive, axis-aligned box of lattice points that contains a shape.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct LatticeBounds {
    /// The corner with the smallest across and down coordinates.
    pub low: LatticePoint,
    /// The corner with the largest across and down coordinates.
    pub high: LatticePoint,
}

/// A flat shape on the lattice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LatticeShape {
    /// A filled disc: the points within `radius` of `center`.
    Disc {
        /// The center of the disc.
        center: LatticePoint,
        /// The radius of the disc.
        radius: DiscRadius,
    },
    /// A filled closed polygon, decided by the even-odd rule.
    Polygon(LatticePolygon),
}

impl LatticeShape {
    /// Builds a disc from its center and radius.
    #[must_use]
    pub const fn disc(center: LatticePoint, radius: DiscRadius) -> Self {
        Self::Disc { center, radius }
    }

    /// Builds a polygon from its first three vertices and every later vertex.
    ///
    /// The vertices are taken in boundary order, and the last one is joined
    /// back to the first.
    #[must_use]
    pub const fn polygon(first: [LatticePoint; 3], rest: Vec<LatticePoint>) -> Self {
        Self::Polygon(LatticePolygon {
            head: first,
            tail: rest,
        })
    }

    /// Returns the smallest inclusive box that contains the shape.
    ///
    /// # Errors
    ///
    /// Returns `Err(LatticeError::Overflow)` when a disc's radius or box leaves
    /// the `i64` range.
    pub fn bounds(&self) -> Result<LatticeBounds, LatticeError> {
        match self {
            Self::Disc { center, radius } => {
                let reach = SubSensel(i64::try_from(radius.0).map_err(Overflow::from)?);
                Ok(LatticeBounds {
                    low: LatticePoint {
                        across: center.across.checked_sub(reach)?,
                        down: center.down.checked_sub(reach)?,
                    },
                    high: LatticePoint {
                        across: center.across.checked_add(reach)?,
                        down: center.down.checked_add(reach)?,
                    },
                })
            }
            Self::Polygon(polygon) => Ok(polygon.bounds()),
        }
    }
}
