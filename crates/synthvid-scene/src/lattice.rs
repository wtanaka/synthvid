//! Exact coverage on a lattice of 1/256 of a sensel.
//!
//! A sensel is one cell of the camera's output grid. Its center sits at lattice
//! coordinates `(256 * column + 128, 256 * row + 128)`, and every coordinate is
//! an integer count of 1/256 of a sensel. [`covers`] decides in exact integer
//! arithmetic whether a lattice point lies inside a flat shape, following the
//! same rules as the exact coverage predicates in [`crate::raster`]: a point
//! exactly on an edge counts as inside, a polygon is decided by the even-odd rule
//! with the half-open vertex rule, and a disc contains the points whose squared
//! distance from its center is at most its squared radius.
//!
//! [`sample_point`] places the samples of one sensel on a regular grid inside it.

mod covers;
mod error;
mod point;
mod sample;
mod shape;
mod wide;

#[cfg(test)]
mod tests;

pub use covers::{covers, Containment};
pub use error::LatticeError;
pub use point::{LatticePoint, LatticeShift, SubSensel};
pub use sample::{sample_point, PixelGrid, SampleIndex, SampleSlot};
pub use shape::{DiscRadius, LatticeBounds, LatticePolygon, LatticeShape};
