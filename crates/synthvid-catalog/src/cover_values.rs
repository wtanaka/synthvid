//! Typed quantities used by the greedy covering algorithm.

/// The position of a value within one axis's value list.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct ValueIndex(pub(super) usize);

/// How many values one axis has.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct ValueCount(pub(super) usize);

/// How many outstanding obligations a partial entry covers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct CoverageCount(pub(super) usize);

/// Whether two axis values may appear together in one entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Compatibility {
    /// The values may appear together.
    Compatible,
    /// The values may not appear together.
    Incompatible,
}

/// Whether an entry covers a pairwise obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Coverage {
    /// The entry covers the obligation.
    Covered,
    /// The entry does not cover the obligation.
    Uncovered,
}
