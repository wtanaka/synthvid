//! The parameters of a blob backdrop: how many discs, and how large.

/// Number of discs a blob backdrop places.
///
/// Every `u16` is a valid count; zero leaves the bare ground. Kept apart from
/// [`BlobRadius`] so a count cannot be passed where a radius is expected.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BlobCount(pub u16);

/// Radius of one blob disc, in pixels.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BlobRadius(pub u16);

/// The inclusive range of blob radii, smallest first by construction.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct BlobRadii {
    /// Smallest radius a disc may take.
    min: BlobRadius,
    /// Largest radius a disc may take, never below `min`.
    max: BlobRadius,
}

impl BlobRadii {
    /// The range spanned by two radii, given in either order.
    ///
    /// The ends are an unordered pair: the smaller becomes the minimum and the
    /// larger the maximum, so no inverted range can be built.
    #[must_use]
    pub const fn spanning(ends: [BlobRadius; 2]) -> Self {
        let [first, second] = ends;
        if first.0 <= second.0 {
            Self {
                min: first,
                max: second,
            }
        } else {
            Self {
                min: second,
                max: first,
            }
        }
    }

    /// The smallest radius a disc may take.
    #[must_use]
    pub const fn min(self) -> BlobRadius {
        self.min
    }

    /// The largest radius a disc may take.
    #[must_use]
    pub const fn max(self) -> BlobRadius {
        self.max
    }
}

#[cfg(test)]
mod tests;
