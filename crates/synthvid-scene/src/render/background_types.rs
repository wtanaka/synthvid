//! Typed inputs for backdrop painting.
//!
//! Each struct bundles the parameters of one backdrop variant. Bundling keeps
//! two same-typed colours from sitting side by side in a parameter list, and
//! the sizes are `NonZeroU16`, so a zero cell or spacing cannot reach the
//! painters at all.

use core::num::NonZeroU16;

use crate::color::Rgb8;

/// The tiling of a checker backdrop.
pub(super) struct CheckerPattern {
    /// Side length of one square, in pixels. Never zero.
    pub(super) cell: NonZeroU16,
    /// Colour of the square at the origin.
    pub(super) even: Rgb8,
    /// Colour of the squares beside it.
    pub(super) odd: Rgb8,
}

/// The two end colours of a gradient backdrop.
pub(super) struct GradientEnds {
    /// Colour at the start edge.
    pub(super) from: Rgb8,
    /// Colour at the far edge.
    pub(super) to: Rgb8,
}

/// The line layout of a grid backdrop.
pub(super) struct GridPattern {
    /// Distance between lines, in pixels. Never zero.
    pub(super) spacing: NonZeroU16,
    /// Colour of the lines.
    pub(super) line: Rgb8,
    /// Colour between the lines.
    pub(super) ground: Rgb8,
}
