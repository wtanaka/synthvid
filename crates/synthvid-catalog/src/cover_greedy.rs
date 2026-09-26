//! Greedy pairwise covering array algorithm for catalog generation.

use super::cover_axes::AxisArrays;
use std::collections::BTreeSet;

/// Represents resolved axis values for an entry: the index into each axis's values.
#[derive(Clone, Copy, Debug)]
pub(super) struct AxisSelection {
    /// Dimension axis index
    pub(super) dimensions: usize,
    /// Frame count axis index
    pub(super) frame_count: usize,
    /// Frame rate axis index
    pub(super) frame_rate: usize,
    /// Background axis index
    pub(super) background: usize,
    /// Shape axis index
    pub(super) shape: usize,
    /// Motion axis index
    pub(super) motion: usize,
    /// Coding axis index
    pub(super) coding: usize,
    /// Container axis index
    pub(super) container: usize,
    /// Track matrix axis index
    pub(super) track_matrix: usize,
    /// Scale axis index
    pub(super) scale: usize,
    /// Defect axis index
    pub(super) defect: usize,
}

/// Pairwise obligation: a pair of distinct axes and the indices of one value from each.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct PairObligation {
    /// First axis index
    axis_a: usize,
    /// Value index in first axis
    value_a: usize,
    /// Second axis index
    axis_b: usize,
    /// Value index in second axis
    value_b: usize,
}

impl PairObligation {
    /// Gets the first axis index.
    pub(super) const fn axis_a(&self) -> usize {
        self.axis_a
    }

    /// Gets the first value index.
    pub(super) const fn value_a(&self) -> usize {
        self.value_a
    }

    /// Gets the second axis index.
    pub(super) const fn axis_b(&self) -> usize {
        self.axis_b
    }

    /// Gets the second value index.
    pub(super) const fn value_b(&self) -> usize {
        self.value_b
    }
}

/// Helper to get or set an `AxisSelection` field by axis number.
#[must_use]
const fn axis_value_mut(sel: &mut AxisSelection, axis_num: usize) -> Option<&mut usize> {
    match axis_num {
        0 => Some(&mut sel.dimensions),
        1 => Some(&mut sel.frame_count),
        2 => Some(&mut sel.frame_rate),
        3 => Some(&mut sel.background),
        4 => Some(&mut sel.shape),
        5 => Some(&mut sel.motion),
        6 => Some(&mut sel.coding),
        7 => Some(&mut sel.container),
        8 => Some(&mut sel.track_matrix),
        9 => Some(&mut sel.scale),
        10 => Some(&mut sel.defect),
        _ => None,
    }
}

/// Helper to get an `AxisSelection` field by axis number.
#[must_use]
const fn axis_value(sel: &AxisSelection, axis_num: usize) -> Option<usize> {
    match axis_num {
        0 => Some(sel.dimensions),
        1 => Some(sel.frame_count),
        2 => Some(sel.frame_rate),
        3 => Some(sel.background),
        4 => Some(sel.shape),
        5 => Some(sel.motion),
        6 => Some(sel.coding),
        7 => Some(sel.container),
        8 => Some(sel.track_matrix),
        9 => Some(sel.scale),
        10 => Some(sel.defect),
        _ => None,
    }
}

/// Returns true if an entry covers a given obligation.
pub(super) fn entry_covers_obligation(
    entry: &super::Entry,
    obligation: PairObligation,
    axes: &AxisArrays,
) -> bool {
    match (
        get_entry_axis_value(entry, obligation.axis_a(), axes),
        get_entry_axis_value(entry, obligation.axis_b(), axes),
    ) {
        (Some(val_a), Some(val_b)) => {
            val_a == obligation.value_a() && val_b == obligation.value_b()
        }
        _ => false,
    }
}

/// Gets the axis-index value for an entry given an axis number.
/// Axis order (0-indexed):
/// 0: `dimensions`, 1: `frame_count`, 2: `frame_rate`, 3: `background`,
/// 4: `shape`, 5: `motion`, 6: `coding`, 7: `container`, 8: `track_matrix`,
/// 9: `scale`, 10: `defect`
///
/// Returns `Some(index)` where index is the position of the entry's value within
/// that axis's value list. Returns `None` if the value cannot be found (which
/// should not happen if the entry was constructed from the same `AxisArrays`).
pub(super) fn get_entry_axis_value(
    entry: &super::Entry,
    axis_num: usize,
    axes: &AxisArrays,
) -> Option<usize> {
    use super::cover_axes::{
        get_background_index, get_coding_index, get_container_index, get_defect_index,
        get_frame_rate_index, get_motion_index, get_scale_index, get_shape_index,
        get_track_matrix_index,
    };

    match axis_num {
        0 => axes.dimensions.iter().position(|&d| d == entry.dimensions),
        1 => axes
            .frame_counts
            .iter()
            .position(|&f| f == entry.frame_count),
        2 => Some(get_frame_rate_index(entry.frame_rate)),
        3 => Some(get_background_index(entry.background)),
        4 => Some(get_shape_index(entry.shape)),
        5 => Some(get_motion_index(entry.motion)),
        6 => Some(get_coding_index(entry.coding)),
        7 => Some(get_container_index(entry.container)),
        8 => Some(get_track_matrix_index(entry.track_matrix)),
        9 => Some(get_scale_index(entry.scale)),
        10 => Some(get_defect_index(entry.defect.as_ref())),
        _ => None,
    }
}

/// Build the complete obligation set.
pub(super) fn build_axis_obligations(axes: &AxisArrays) -> BTreeSet<PairObligation> {
    let mut obligations = BTreeSet::new();

    let all_axis_lens = [
        axes.dimensions.len(),
        axes.frame_counts.len(),
        axes.frame_rates.len(),
        axes.backgrounds.len(),
        axes.shapes.len(),
        axes.motions.len(),
        axes.codings.len(),
        axes.containers.len(),
        axes.track_matrices.len(),
        axes.scales.len(),
        axes.defects.len(),
    ];

    // Generate all pairwise combinations using enumerate to avoid direct indexing
    for (a, len_a) in all_axis_lens.iter().copied().enumerate() {
        for (b, len_b) in all_axis_lens.iter().copied().enumerate() {
            if b <= a {
                continue;
            }
            for va in 0..len_a {
                for vb in 0..len_b {
                    if a == 7
                        && b == 10
                        && !super::cover_axes::container_defect_compatible(axes, va, vb)
                    {
                        continue;
                    }
                    obligations.insert(PairObligation {
                        axis_a: a,
                        value_a: va,
                        axis_b: b,
                        value_b: vb,
                    });
                }
            }
        }
    }

    obligations
}

/// Build one entry satisfying the first obligation and maximizing coverage of remaining obligations.
pub(super) fn build_entry_for_obligation(
    obligation: PairObligation,
    obligations: &BTreeSet<PairObligation>,
    axes: &AxisArrays,
) -> AxisSelection {
    // Start with values satisfying the obligation; all axes start at index 0
    let mut selection = AxisSelection {
        dimensions: 0,
        frame_count: 0,
        frame_rate: 0,
        background: 0,
        shape: 0,
        motion: 0,
        coding: 0,
        container: 0,
        track_matrix: 0,
        scale: 0,
        defect: 0,
    };

    // Obligation axes are always valid: 0..11 by construction (from build_axis_obligations)
    if let Some(slot) = axis_value_mut(&mut selection, obligation.axis_a()) {
        *slot = obligation.value_a();
    }
    if let Some(slot) = axis_value_mut(&mut selection, obligation.axis_b()) {
        *slot = obligation.value_b();
    }

    // For each axis (except those from obligation), pick the value that covers the most obligations
    for axis in 0..11 {
        if axis == obligation.axis_a() || axis == obligation.axis_b() {
            continue;
        }

        let len_this_axis = get_axis_length(axes, axis);
        let mut best_value = 0;
        let mut best_coverage = 0;

        for value in 0..len_this_axis {
            // Axis is always valid: 0..11 from outer loop and always < 11
            if let Some(slot) = axis_value_mut(&mut selection, axis) {
                *slot = value;
            }

            if axis == 7
                && !super::cover_axes::container_defect_compatible(axes, value, selection.defect)
            {
                continue;
            }
            if axis == 10
                && !super::cover_axes::container_defect_compatible(axes, selection.container, value)
            {
                continue;
            }
            if (axis == 0 || axis == 1 || axis == 6 || axis == 7)
                && !super::cover_axes::raw_avi_size_compatible(
                    axes,
                    selection.dimensions,
                    selection.frame_count,
                    selection.coding,
                    selection.container,
                )
            {
                continue;
            }

            // Count how many obligations this would cover
            let coverage = count_covered_obligations(&selection, obligations);

            if coverage > best_coverage {
                best_coverage = coverage;
                best_value = value;
            }
        }

        // Axis is always valid: 0..11 from outer loop and always < 11
        if let Some(slot) = axis_value_mut(&mut selection, axis) {
            *slot = best_value;
        }
    }

    selection
}

/// Gets the length of an axis, indexed 0..11.
const fn get_axis_length(axes: &AxisArrays, axis_idx: usize) -> usize {
    match axis_idx {
        0 => axes.dimensions.len(),
        1 => axes.frame_counts.len(),
        2 => axes.frame_rates.len(),
        3 => axes.backgrounds.len(),
        4 => axes.shapes.len(),
        5 => axes.motions.len(),
        6 => axes.codings.len(),
        7 => axes.containers.len(),
        8 => axes.track_matrices.len(),
        9 => axes.scales.len(),
        10 => axes.defects.len(),
        _ => 0, // Unreachable by construction
    }
}

/// Count how many outstanding obligations would be covered by the given partial entry.
fn count_covered_obligations(
    selection: &AxisSelection,
    obligations: &BTreeSet<PairObligation>,
) -> usize {
    obligations
        .iter()
        .filter(|&obligation| {
            if let (Some(val_a), Some(val_b)) = (
                axis_value(selection, obligation.axis_a()),
                axis_value(selection, obligation.axis_b()),
            ) {
                val_a == obligation.value_a() && val_b == obligation.value_b()
            } else {
                false
            }
        })
        .count()
}
