//! Greedy pairwise covering array algorithm for catalog generation.

use super::cover_axes::AxisArrays;
use super::CatalogError;
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
    const fn value_count(self, axes: &AxisArrays) -> usize {
        match self {
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
        }
    }
}

impl AxisSelection {
    /// The index chosen on `axis`.
    const fn get(&self, axis: CoverAxis) -> usize {
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
    const fn set(&mut self, axis: CoverAxis, index: usize) {
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
    /// First axis
    axis_a: CoverAxis,
    /// Value index in first axis
    value_a: usize,
    /// Second axis
    axis_b: CoverAxis,
    /// Value index in second axis
    value_b: usize,
}

impl PairObligation {
    /// Gets the first axis.
    pub(super) const fn axis_a(&self) -> CoverAxis {
        self.axis_a
    }

    /// Gets the first value index.
    pub(super) const fn value_a(&self) -> usize {
        self.value_a
    }

    /// Gets the second axis.
    pub(super) const fn axis_b(&self) -> CoverAxis {
        self.axis_b
    }

    /// Gets the second value index.
    pub(super) const fn value_b(&self) -> usize {
        self.value_b
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
        (Ok(val_a), Ok(val_b)) => val_a == obligation.value_a() && val_b == obligation.value_b(),
        _ => false,
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
) -> Result<usize, CatalogError> {
    use super::cover_axes::{
        get_background_index, get_coding_index, get_container_index, get_frame_rate_index,
        get_motion_index, get_scale_index, get_shape_index, get_track_matrix_index,
    };

    match column {
        CoverAxis::Dimensions => axes
            .dimensions
            .iter()
            .position(|&d| d == entry.dimensions)
            .ok_or(CatalogError::AxisIndexOutOfRange),
        CoverAxis::FrameCount => axes
            .frame_counts
            .iter()
            .position(|&f| f == entry.frame_count)
            .ok_or(CatalogError::AxisIndexOutOfRange),
        CoverAxis::FrameRate => Ok(get_frame_rate_index(entry.frame_rate)),
        CoverAxis::Background => Ok(get_background_index(entry.background)),
        CoverAxis::Shape => Ok(get_shape_index(entry.shape)),
        CoverAxis::Motion => Ok(get_motion_index(entry.motion)),
        CoverAxis::Coding => Ok(get_coding_index(entry.coding)),
        CoverAxis::Container => Ok(get_container_index(entry.container)),
        CoverAxis::TrackMatrix => Ok(get_track_matrix_index(entry.track_matrix)),
        CoverAxis::Scale => Ok(get_scale_index(entry.scale)),
        CoverAxis::Defect => axes
            .defects
            .iter()
            .position(|slot| *slot == entry.defect)
            .ok_or(CatalogError::AxisIndexOutOfRange),
    }
}

/// Build the complete obligation set.
pub(super) fn build_axis_obligations(axes: &AxisArrays) -> BTreeSet<PairObligation> {
    use super::cover_axes::{container_defect_compatible, volume_compatible};

    let mut obligations = BTreeSet::new();

    for (position, axis_a) in CoverAxis::ALL.into_iter().enumerate() {
        for axis_b in CoverAxis::ALL.into_iter().skip(position.saturating_add(1)) {
            for value_a in 0..axis_a.value_count(axes) {
                for value_b in 0..axis_b.value_count(axes) {
                    let compatible = match (axis_a, axis_b) {
                        (CoverAxis::Dimensions, CoverAxis::FrameCount) => {
                            volume_compatible(axes, value_a, value_b)
                        }
                        (CoverAxis::Container, CoverAxis::Defect) => {
                            container_defect_compatible(axes, value_a, value_b)
                        }
                        _ => true,
                    };
                    if compatible {
                        obligations.insert(PairObligation {
                            axis_a,
                            value_a,
                            axis_b,
                            value_b,
                        });
                    }
                }
            }
        }
    }

    obligations
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

    selection.set(obligation.axis_a(), obligation.value_a());
    selection.set(obligation.axis_b(), obligation.value_b());

    // For each axis (except those from obligation), pick the value that covers the most obligations
    for axis in CoverAxis::ALL {
        if axis == obligation.axis_a() || axis == obligation.axis_b() {
            continue;
        }

        let len_this_axis = axis.value_count(axes);
        // The first compatible value wins until one covers more; zero is not
        // a safe default because it may be incompatible with the pinned axes.
        let mut best: Result<(usize, usize), CatalogError> =
            Err(CatalogError::UnsatisfiableObligation);

        for value in 0..len_this_axis {
            selection.set(axis, value);

            if axis == CoverAxis::Container
                && !super::cover_axes::container_defect_compatible(axes, value, selection.defect)
            {
                continue;
            }
            if axis == CoverAxis::Defect
                && !super::cover_axes::container_defect_compatible(axes, selection.container, value)
            {
                continue;
            }
            if matches!(axis, CoverAxis::Dimensions | CoverAxis::FrameCount)
                && !super::cover_axes::volume_compatible(
                    axes,
                    selection.dimensions,
                    selection.frame_count,
                )
            {
                continue;
            }
            if matches!(
                axis,
                CoverAxis::Dimensions
                    | CoverAxis::FrameCount
                    | CoverAxis::Coding
                    | CoverAxis::Container
            ) && !super::cover_axes::raw_avi_size_compatible(
                axes,
                selection.dimensions,
                selection.frame_count,
                selection.coding,
                selection.container,
            ) {
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
) -> usize {
    obligations
        .iter()
        .filter(|&obligation| {
            selection.get(obligation.axis_a()) == obligation.value_a()
                && selection.get(obligation.axis_b()) == obligation.value_b()
        })
        .count()
}
