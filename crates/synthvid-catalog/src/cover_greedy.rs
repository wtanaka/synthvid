//! Greedy pairwise covering array algorithm for catalog generation.

use super::cover_axes::{
    container_defect_compatible, raw_avi_size_compatible, volume_compatible, AxisArrays,
};
use super::cover_values::{Compatibility, Coverage, CoverageCount, ValueCount, ValueIndex};
use super::CatalogError;
extern crate alloc;
use alloc::collections::BTreeSet;

/// Represents resolved axis values for an entry: the index into each axis's values.
#[derive(Clone, Copy, Debug)]
pub(super) struct AxisSelection {
    /// Dimension axis index.
    pub(super) dimensions: ValueIndex,
    /// Frame count axis index.
    pub(super) frame_count: ValueIndex,
    /// Frame rate axis index.
    pub(super) frame_rate: ValueIndex,
    /// Background axis index.
    pub(super) background: ValueIndex,
    /// Shape axis index.
    pub(super) shape: ValueIndex,
    /// Motion axis index.
    pub(super) motion: ValueIndex,
    /// Coding axis index.
    pub(super) coding: ValueIndex,
    /// Container axis index.
    pub(super) container: ValueIndex,
    /// Track matrix axis index.
    pub(super) track_matrix: ValueIndex,
    /// Scale axis index.
    pub(super) scale: ValueIndex,
    /// Defect axis index.
    pub(super) defect: ValueIndex,
}

/// One axis of the covering array, in the order pairs are enumerated.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum CoverAxis {
    /// Frame dimensions.
    Dimensions,
    /// Number of frames.
    FrameCount,
    /// Frame rate.
    FrameRate,
    /// Background type.
    Background,
    /// Shape type.
    Shape,
    /// Motion type.
    Motion,
    /// Pixel encoding.
    Coding,
    /// Container format.
    Container,
    /// Display matrix transformation.
    TrackMatrix,
    /// Physical scale declaration.
    Scale,
    /// Optional defect.
    Defect,
}

impl CoverAxis {
    /// Every axis, in enumeration order.
    pub(super) const ALL: [Self; 11] = [
        Self::Dimensions,
        Self::FrameCount,
        Self::FrameRate,
        Self::Background,
        Self::Shape,
        Self::Motion,
        Self::Coding,
        Self::Container,
        Self::TrackMatrix,
        Self::Scale,
        Self::Defect,
    ];

    /// How many values this axis has.
    const fn value_count(self, axes: &AxisArrays) -> ValueCount {
        ValueCount(match self {
            Self::Dimensions => axes.dimensions.len(),
            Self::FrameCount => axes.frame_counts.len(),
            Self::FrameRate => axes.frame_rates.len(),
            Self::Background => axes.backgrounds.len(),
            Self::Shape => axes.shapes.len(),
            Self::Motion => axes.motions.len(),
            Self::Coding => axes.codings.len(),
            Self::Container => axes.containers.len(),
            Self::TrackMatrix => axes.track_matrices.len(),
            Self::Scale => axes.scales.len(),
            Self::Defect => axes.defects.len(),
        })
    }
}

impl AxisSelection {
    /// The selection that sets only the two values of `obligation`; every other
    /// axis is at index 0.
    const fn for_obligation(obligation: PairObligation) -> Self {
        let zero = ValueIndex(0);
        let mut selection = Self {
            dimensions: zero,
            frame_count: zero,
            frame_rate: zero,
            background: zero,
            shape: zero,
            motion: zero,
            coding: zero,
            container: zero,
            track_matrix: zero,
            scale: zero,
            defect: zero,
        };
        selection.set(obligation.axis_a(), obligation.value_a());
        selection.set(obligation.axis_b(), obligation.value_b());
        selection
    }

    /// The index chosen on `axis`.
    const fn get(&self, axis: CoverAxis) -> ValueIndex {
        match axis {
            CoverAxis::Dimensions => self.dimensions,
            CoverAxis::FrameCount => self.frame_count,
            CoverAxis::FrameRate => self.frame_rate,
            CoverAxis::Background => self.background,
            CoverAxis::Shape => self.shape,
            CoverAxis::Motion => self.motion,
            CoverAxis::Coding => self.coding,
            CoverAxis::Container => self.container,
            CoverAxis::TrackMatrix => self.track_matrix,
            CoverAxis::Scale => self.scale,
            CoverAxis::Defect => self.defect,
        }
    }

    /// Chooses `index` on `axis`.
    const fn set(&mut self, axis: CoverAxis, index: ValueIndex) {
        match axis {
            CoverAxis::Dimensions => self.dimensions = index,
            CoverAxis::FrameCount => self.frame_count = index,
            CoverAxis::FrameRate => self.frame_rate = index,
            CoverAxis::Background => self.background = index,
            CoverAxis::Shape => self.shape = index,
            CoverAxis::Motion => self.motion = index,
            CoverAxis::Coding => self.coding = index,
            CoverAxis::Container => self.container = index,
            CoverAxis::TrackMatrix => self.track_matrix = index,
            CoverAxis::Scale => self.scale = index,
            CoverAxis::Defect => self.defect = index,
        }
    }
}

/// Pairwise obligation: a pair of distinct axes and the indices of one value from each.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct PairObligation {
    /// First axis.
    axis_a: CoverAxis,
    /// Value index in first axis.
    value_a: ValueIndex,
    /// Second axis.
    axis_b: CoverAxis,
    /// Value index in second axis.
    value_b: ValueIndex,
}

impl PairObligation {
    /// Gets the first axis.
    pub(super) const fn axis_a(&self) -> CoverAxis {
        self.axis_a
    }

    /// Gets the first value index.
    pub(super) const fn value_a(&self) -> ValueIndex {
        self.value_a
    }

    /// Gets the second axis.
    pub(super) const fn axis_b(&self) -> CoverAxis {
        self.axis_b
    }

    /// Gets the second value index.
    pub(super) const fn value_b(&self) -> ValueIndex {
        self.value_b
    }
}

/// Returns whether an entry covers a given obligation.
pub(super) fn entry_covers_obligation(
    entry: &super::Entry,
    obligation: PairObligation,
    axes: &AxisArrays,
) -> Coverage {
    match (
        get_entry_axis_value(entry, obligation.axis_a(), axes),
        get_entry_axis_value(entry, obligation.axis_b(), axes),
    ) {
        (Ok(val_a), Ok(val_b))
            if val_a == obligation.value_a() && val_b == obligation.value_b() =>
        {
            Coverage::Covered
        }
        _ => Coverage::Uncovered,
    }
}

/// Gets the axis-index value for an entry on the axis `column`: the position of
/// the entry's value within that axis's value list.
///
/// # Errors
///
/// Returns `CatalogError::AxisIndexOutOfRange` when the value is not in the
/// list, which cannot happen if the entry was built from the same `AxisArrays`.
pub(super) fn get_entry_axis_value(
    entry: &super::Entry,
    column: CoverAxis,
    axes: &AxisArrays,
) -> Result<ValueIndex, CatalogError> {
    match column {
        CoverAxis::Dimensions => position_of(&axes.dimensions, &entry.dimensions),
        CoverAxis::FrameCount => position_of(&axes.frame_counts, &entry.frame_count),
        CoverAxis::FrameRate => position_of(&axes.frame_rates, &entry.frame_rate),
        CoverAxis::Background => position_of(&axes.backgrounds, &entry.background),
        CoverAxis::Shape => position_of(&axes.shapes, &entry.shape),
        CoverAxis::Motion => position_of(&axes.motions, &entry.motion),
        CoverAxis::Coding => position_of(&axes.codings, &entry.coding),
        CoverAxis::Container => position_of(&axes.containers, &entry.container()),
        CoverAxis::TrackMatrix => position_of(&axes.track_matrices, &entry.track_matrix),
        CoverAxis::Scale => position_of(&axes.scales, &entry.scale),
        CoverAxis::Defect => position_of(&axes.defects, &entry.defect),
    }
}

/// The position of `wanted` within an axis's value list.
///
/// # Errors
///
/// Returns `CatalogError::AxisIndexOutOfRange` when the list does not hold it.
fn position_of<Value>(values: &[Value], wanted: &Value) -> Result<ValueIndex, CatalogError>
where
    Value: PartialEq,
{
    values
        .iter()
        .position(|value| value == wanted)
        .map(ValueIndex)
        .ok_or(CatalogError::AxisIndexOutOfRange)
}

/// Build the complete obligation set.
pub(super) fn build_axis_obligations(axes: &AxisArrays) -> BTreeSet<PairObligation> {
    let mut obligations = BTreeSet::new();

    for (position, axis_a) in CoverAxis::ALL.into_iter().enumerate() {
        for (_, axis_b) in CoverAxis::ALL
            .into_iter()
            .enumerate()
            .filter(|(later, _)| *later > position)
        {
            obligations.extend(axis_pair_obligations(
                axes,
                AxisPair {
                    earlier: axis_a,
                    later: axis_b,
                },
            ));
        }
    }

    obligations
}

/// Two distinct axes, the first declared before the second.
#[derive(Copy, Clone, Debug)]
struct AxisPair {
    /// The axis declared first.
    earlier: CoverAxis,
    /// The axis declared second.
    later: CoverAxis,
}

/// Every compatible pair of values from the two axes of `pair`.
fn axis_pair_obligations(
    axes: &AxisArrays,
    pair: AxisPair,
) -> impl Iterator<Item = PairObligation> + '_ {
    let AxisPair {
        earlier: axis_a,
        later: axis_b,
    } = pair;
    (0..axis_a.value_count(axes).0)
        .flat_map(move |value_a| {
            (0..axis_b.value_count(axes).0).map(move |value_b| PairObligation {
                axis_a,
                value_a: ValueIndex(value_a),
                axis_b,
                value_b: ValueIndex(value_b),
            })
        })
        .filter(|obligation| pair_compatibility(axes, *obligation) == Compatibility::Compatible)
}

/// Whether the two values an obligation names may appear together.
///
/// Only two pairs of axes constrain each other; every other pair of values
/// may appear together.
fn pair_compatibility(axes: &AxisArrays, obligation: PairObligation) -> Compatibility {
    let pair = AxisSelection::for_obligation(obligation);
    if obligation.axis_a == CoverAxis::Dimensions && obligation.axis_b == CoverAxis::FrameCount {
        volume_compatible(axes, &pair)
    } else if obligation.axis_a == CoverAxis::Container && obligation.axis_b == CoverAxis::Defect {
        container_defect_compatible(axes, &pair)
    } else {
        Compatibility::Compatible
    }
}

/// Build one entry satisfying the first obligation and maximizing coverage of remaining obligations.
///
/// # Errors
///
/// Returns `CatalogError::UnsatisfiableObligation` when an axis has no value
/// compatible with the values already chosen.
pub(super) fn build_entry_for_obligation(
    obligation: PairObligation,
    obligations: &BTreeSet<PairObligation>,
    axes: &AxisArrays,
) -> Result<AxisSelection, CatalogError> {
    // Start with values satisfying the obligation; all other axes start at index 0
    let mut selection = AxisSelection::for_obligation(obligation);

    // For each axis (except those from obligation), pick the value that covers the most obligations
    for axis in CoverAxis::ALL {
        if axis == obligation.axis_a() || axis == obligation.axis_b() {
            continue;
        }

        let len_this_axis = axis.value_count(axes);
        // Which size checks apply to a value on this axis.
        let (checks_volume, checks_raw_size) = match axis {
            CoverAxis::Dimensions | CoverAxis::FrameCount => (true, true),
            CoverAxis::Coding | CoverAxis::Container => (false, true),
            CoverAxis::FrameRate
            | CoverAxis::Background
            | CoverAxis::Shape
            | CoverAxis::Motion
            | CoverAxis::TrackMatrix
            | CoverAxis::Scale
            | CoverAxis::Defect => (false, false),
        };
        // The first compatible value wins until one covers more; zero is not
        // a safe default because it may be incompatible with the pinned axes.
        let mut best: Result<(ValueIndex, CoverageCount), CatalogError> =
            Err(CatalogError::UnsatisfiableObligation);

        for index in 0..len_this_axis.0 {
            let value = ValueIndex(index);
            selection.set(axis, value);

            if axis == CoverAxis::Container
                && container_defect_compatible(axes, &selection) == Compatibility::Incompatible
            {
                continue;
            }
            if axis == CoverAxis::Defect
                && container_defect_compatible(axes, &selection) == Compatibility::Incompatible
            {
                continue;
            }
            if checks_volume && volume_compatible(axes, &selection) == Compatibility::Incompatible {
                continue;
            }
            if checks_raw_size
                && raw_avi_size_compatible(axes, &selection) == Compatibility::Incompatible
            {
                continue;
            }

            // Count how many obligations this would cover
            let coverage = count_covered_obligations(&selection, obligations);

            best = match best {
                Ok((_, best_coverage)) if best_coverage >= coverage => best,
                _ => Ok((value, coverage)),
            };
        }
        let (best_value, _) = best?;

        selection.set(axis, best_value);
    }

    Ok(selection)
}

/// Count how many outstanding obligations would be covered by the given partial entry.
fn count_covered_obligations(
    selection: &AxisSelection,
    obligations: &BTreeSet<PairObligation>,
) -> CoverageCount {
    CoverageCount(
        obligations
            .iter()
            .filter(|&obligation| {
                selection.get(obligation.axis_a()) == obligation.value_a()
                    && selection.get(obligation.axis_b()) == obligation.value_b()
            })
            .count(),
    )
}
