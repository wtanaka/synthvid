//! Sample points inside one sensel.

use core::num::NonZeroU32;

use crate::frame::{PixelColumn, PixelRow};
use crate::ratio::{column_start, row_start, sample_axis, GridShift};

use super::point::LatticePoint;

/// The denominator 1.
const ONE_PER_AXIS: NonZeroU32 = NonZeroU32::MIN;

/// The denominator 2.
const TWO_PER_AXIS: NonZeroU32 = match NonZeroU32::new(2) {
    Some(value) => value,
    // Unreachable: 2 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// The denominator 4.
const FOUR_PER_AXIS: NonZeroU32 = match NonZeroU32::new(4) {
    Some(value) => value,
    // Unreachable: 4 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// The denominator 8.
const EIGHT_PER_AXIS: NonZeroU32 = match NonZeroU32::new(8) {
    Some(value) => value,
    // Unreachable: 8 is a fixed nonzero literal.
    None => NonZeroU32::MIN,
};

/// The indices of a grid with one sample per axis.
const ONE_INDICES: [SampleIndex; 1] = [SampleIndex(0)];

/// The indices of a grid with two samples per axis.
const TWO_INDICES: [SampleIndex; 2] = [SampleIndex(0), SampleIndex(1)];

/// The indices of a grid with four samples per axis.
const FOUR_INDICES: [SampleIndex; 4] = [
    SampleIndex(0),
    SampleIndex(1),
    SampleIndex(2),
    SampleIndex(3),
];

/// The indices of a grid with eight samples per axis.
const EIGHT_INDICES: [SampleIndex; 8] = [
    SampleIndex(0),
    SampleIndex(1),
    SampleIndex(2),
    SampleIndex(3),
    SampleIndex(4),
    SampleIndex(5),
    SampleIndex(6),
    SampleIndex(7),
];

/// The number of samples along each axis inside one sensel.
///
/// Each value divides 128, so every sample position is an exact lattice point.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum PixelGrid {
    /// One sample at the center of the sensel.
    One,
    /// Two samples along each axis.
    Two,
    /// Four samples along each axis.
    Four,
    /// Eight samples along each axis.
    Eight,
}

impl PixelGrid {
    /// Returns the number of samples along each axis of a sensel.
    #[must_use]
    pub const fn per_axis(self) -> NonZeroU32 {
        match self {
            Self::One => ONE_PER_AXIS,
            Self::Two => TWO_PER_AXIS,
            Self::Four => FOUR_PER_AXIS,
            Self::Eight => EIGHT_PER_AXIS,
        }
    }

    /// Returns the base-2 logarithm of the samples per axis, which is the shift that divides a
    /// span by the number of samples along it.
    pub(crate) const fn log2_per_axis(self) -> GridShift {
        match self {
            Self::One => GridShift(0),
            Self::Two => GridShift(1),
            Self::Four => GridShift(2),
            Self::Eight => GridShift(3),
        }
    }

    /// Returns every sample slot of the grid, with the across index counted
    /// outside the down index.
    ///
    /// Each slot carries this grid, so a slot cannot be used with a grid other
    /// than the one that produced it.
    pub fn slots(self) -> impl Iterator<Item = SampleSlot> {
        let indices: &'static [SampleIndex] = match self {
            Self::One => &ONE_INDICES,
            Self::Two => &TWO_INDICES,
            Self::Four => &FOUR_INDICES,
            Self::Eight => &EIGHT_INDICES,
        };
        indices.iter().flat_map(move |&across| {
            indices.iter().map(move |&down| SampleSlot {
                grid: self,
                across,
                down,
            })
        })
    }
}

/// The index of one sample along an axis of a sensel, counted from zero.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SampleIndex(pub u8);

/// The position of one sample within its sensel, across and down.
///
/// Slots are produced by [`PixelGrid::slots`], so every slot lies inside the
/// grid it was produced from. The fields are private for that reason.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SampleSlot {
    /// The grid that produced the slot.
    grid: PixelGrid,
    /// The sample's index across the sensel.
    across: SampleIndex,
    /// The sample's index down the sensel.
    down: SampleIndex,
}

impl SampleSlot {
    /// Returns the sample's index across the sensel.
    #[must_use]
    pub const fn across(self) -> SampleIndex {
        self.across
    }

    /// Returns the sample's index down the sensel.
    #[must_use]
    pub const fn down(self) -> SampleIndex {
        self.down
    }
}

/// Returns the lattice point of one sample inside a sensel.
///
/// Sample `(i, j)` of sensel `(column, row)` sits at
/// `(256 * column + 128 * (2 * i + 1) / G, 256 * row + 128 * (2 * j + 1) / G)`,
/// where `G` is the grid's samples per axis. Because `G` divides 128, each
/// quotient is exact.
#[must_use]
pub fn sample_point(column: PixelColumn, row: PixelRow, slot: SampleSlot) -> LatticePoint {
    LatticePoint {
        across: sample_axis(column_start(column), slot.across, slot.grid),
        down: sample_axis(row_start(row), slot.down, slot.grid),
    }
}
