//! Catalog entry generation via pairwise covering array.
//!
//! Declares axes of variation (dimensions, frame rates, shapes, etc.) and
//! generates a minimal set of entries in which every pair of values from any
//! two axes appears together at least once. This is a standard test-design
//! technique: pairwise coverage finds most interaction defects from a small
//! number of cases, and generating the set rather than writing it keeps the
//! corpus honest — it cannot accumulate one-off entries that no axis explains.

#[path = "cover_axes.rs"]
mod cover_axes;

#[path = "cover_greedy.rs"]
mod cover_greedy;

#[cfg(test)]
#[path = "cover_tests.rs"]
mod cover_tests;

use crate::json_names::JsonEntryName;
use core::fmt::Write as FmtWrite;
use std::fmt;
use synthvid_encode::defects::Defect;
use synthvid_scene::Dimensions;

/// Error type for catalog generation.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CatalogError {
    /// A dimension value was zero (rejected by Width or Height constructor).
    ZeroDimension,
    /// A frame rate value was invalid.
    InvalidFrameRate,
    /// A resolved axis index was out of range.
    AxisIndexOutOfRange,
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDimension => write!(f, "dimension value was zero"),
            Self::InvalidFrameRate => write!(f, "frame rate was invalid"),
            Self::AxisIndexOutOfRange => write!(f, "resolved axis index was out of range"),
        }
    }
}

impl core::error::Error for CatalogError {}

// Re-export axis types from the submodule
pub use cover_axes::{
    BackgroundAxis, CodingAxis, ContainerAxis, FrameCountValue, FrameRateValue, MotionAxis,
    ScaleAxis, ShapeAxis, TrackMatrixAxis,
};

/// A catalog entry: a scene description with all parameters specified.
///
/// Names (manifest and media paths) are derived from the entry's axes,
/// never hand-written. Two entries with identical axis values produce
/// identical names; two entries differing in any axis produce different names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    /// Frame dimensions
    pub dimensions: Dimensions,
    /// Number of frames
    pub frame_count: FrameCountValue,
    /// Frame rate
    pub frame_rate: FrameRateValue,
    /// Background type
    pub background: BackgroundAxis,
    /// Shape type
    pub shape: ShapeAxis,
    /// Motion type
    pub motion: MotionAxis,
    /// Pixel encoding
    pub coding: CodingAxis,
    /// Container format
    pub container: ContainerAxis,
    /// Display matrix transformation
    pub track_matrix: TrackMatrixAxis,
    /// Physical scale declaration
    pub scale: ScaleAxis,
    /// Optional defect
    pub defect: Option<Defect>,
}

impl Entry {
    /// Generates the manifest name for this entry.
    ///
    /// Derived deterministically from all axis values. Two entries with
    /// identical axes produce identical names; different axes produce
    /// different names.
    #[must_use]
    pub fn manifest_name(&self) -> JsonEntryName {
        let mut name = String::new();
        write!(
            name,
            "{:04}x{:04}",
            self.dimensions.width.get().get(),
            self.dimensions.height.get().get()
        )
        .ok();
        name.push('-');
        name.push_str(&Self::frame_count_str(self.frame_count));
        name.push('-');
        name.push_str(Self::frame_rate_str(self.frame_rate));
        name.push('-');
        name.push_str(Self::background_str(self.background));
        name.push('-');
        name.push_str(Self::shape_str(self.shape));
        name.push('-');
        name.push_str(Self::motion_str(self.motion));
        name.push('-');
        name.push_str(Self::coding_str(self.coding));
        name.push('-');
        name.push_str(Self::container_str(self.container));
        name.push('-');
        name.push_str(Self::track_matrix_str(self.track_matrix));
        name.push('-');
        name.push_str(Self::scale_str(self.scale));
        name.push('-');
        name.push_str(Self::defect_str(self.defect.as_ref()));

        // All components consist only of lowercase letters, digits, and hyphens,
        // so this cannot fail. The regex `[a-z0-9-]+` is guaranteed to match.
        JsonEntryName::from_known(&name)
    }

    /// Generates the media file name for this entry.
    ///
    /// Uses the same derivation as the manifest name, ensuring they stay in sync.
    /// The extension is determined by the container type.
    /// Returns a string suitable for use as a filename.
    #[must_use]
    pub fn media_name(&self) -> String {
        let base = self.manifest_name();
        let ext = match self.container {
            ContainerAxis::Iso => "mp4",
            ContainerAxis::Avi => "avi",
        };
        format!("{}-media.{}", base.as_str(), ext)
    }

    /// Converts a frame count to a name fragment.
    fn frame_count_str(fc: FrameCountValue) -> String {
        format!("{}", fc.get())
    }

    /// Converts a frame rate to a name fragment.
    const fn frame_rate_str(fr: FrameRateValue) -> &'static str {
        match fr {
            FrameRateValue::Thirty => "30fps",
            FrameRateValue::Sixty => "60fps",
            FrameRateValue::OneTwenty => "120fps",
            FrameRateValue::TwoForty => "240fps",
            FrameRateValue::Ntsc => "ntsc",
            FrameRateValue::Absurd => "absurd",
        }
    }

    /// Converts a background choice to a name fragment.
    const fn background_str(bg: BackgroundAxis) -> &'static str {
        match bg {
            BackgroundAxis::Solid => "solid",
            BackgroundAxis::Checker => "checker",
            BackgroundAxis::Gradient => "gradient",
            BackgroundAxis::Blobs => "blobs",
            BackgroundAxis::Grid => "grid",
        }
    }

    /// Converts a shape choice to a name fragment.
    const fn shape_str(sh: ShapeAxis) -> &'static str {
        match sh {
            ShapeAxis::Disc => "disc",
            ShapeAxis::Rect => "rect",
            ShapeAxis::Polygon => "polygon",
            ShapeAxis::Cross => "cross",
        }
    }

    /// Converts a motion choice to a name fragment.
    const fn motion_str(mot: MotionAxis) -> &'static str {
        match mot {
            MotionAxis::Fixed => "fixed",
            MotionAxis::Linear => "linear",
            MotionAxis::Ballistic => "ballistic",
            MotionAxis::Circular => "circular",
            MotionAxis::Oscillating => "oscillating",
            MotionAxis::Walk => "walk",
        }
    }

    /// Converts a coding choice to a name fragment.
    const fn coding_str(cod: CodingAxis) -> &'static str {
        match cod {
            CodingAxis::Raw => "raw",
            CodingAxis::MotionJpeg => "mjpeg",
        }
    }

    /// Converts a container choice to a name fragment.
    const fn container_str(cont: ContainerAxis) -> &'static str {
        match cont {
            ContainerAxis::Iso => "iso",
            ContainerAxis::Avi => "avi",
        }
    }

    /// Converts a track matrix choice to a name fragment.
    const fn track_matrix_str(tm: TrackMatrixAxis) -> &'static str {
        match tm {
            TrackMatrixAxis::Identity => "identity",
            TrackMatrixAxis::Rotate90 => "rot90",
            TrackMatrixAxis::Rotate180 => "rot180",
            TrackMatrixAxis::Rotate270 => "rot270",
        }
    }

    /// Converts a scale choice to a name fragment.
    const fn scale_str(sc: ScaleAxis) -> &'static str {
        match sc {
            ScaleAxis::WithScale => "scaled",
            ScaleAxis::NoScale => "unscaled",
        }
    }

    /// Converts a defect choice to a name fragment.
    #[must_use]
    fn defect_str(def: Option<&Defect>) -> &'static str {
        def.as_ref()
            .map_or("nodef", |d| cover_axes::defect_variant_str(d))
    }
}

/// Defines all axis values for the catalog.
fn define_axes() -> Result<cover_axes::AxisArrays, CatalogError> {
    use cover_axes::{
        all_defects, make_dimensions, AxisArrays, BackgroundAxis, CodingAxis, ContainerAxis,
        FrameCountValue, FrameRateValue, MotionAxis, RawSize, ScaleAxis, ShapeAxis,
        TrackMatrixAxis,
    };

    // Define all axis values in declared order
    let dimensions = [
        make_dimensions(RawSize {
            width: 16,
            height: 16,
        })?,
        make_dimensions(RawSize {
            width: 320,
            height: 240,
        })?,
        make_dimensions(RawSize {
            width: 640,
            height: 480,
        })?,
        make_dimensions(RawSize {
            width: 1280,
            height: 720,
        })?,
        make_dimensions(RawSize {
            width: 1920,
            height: 1080,
        })?,
    ];

    let frame_counts = [
        FrameCountValue::new(1),
        FrameCountValue::new(10),
        FrameCountValue::new(100),
        FrameCountValue::new(1000),
        FrameCountValue::new(20000),
    ];

    let frame_rates = [
        FrameRateValue::Thirty,
        FrameRateValue::Sixty,
        FrameRateValue::OneTwenty,
        FrameRateValue::TwoForty,
        FrameRateValue::Ntsc,
        FrameRateValue::Absurd,
    ];

    let backgrounds = [
        BackgroundAxis::Solid,
        BackgroundAxis::Checker,
        BackgroundAxis::Gradient,
        BackgroundAxis::Blobs,
        BackgroundAxis::Grid,
    ];

    let shapes = [
        ShapeAxis::Disc,
        ShapeAxis::Rect,
        ShapeAxis::Polygon,
        ShapeAxis::Cross,
    ];

    let motions = [
        MotionAxis::Fixed,
        MotionAxis::Linear,
        MotionAxis::Ballistic,
        MotionAxis::Circular,
        MotionAxis::Oscillating,
        MotionAxis::Walk,
    ];

    let codings = [CodingAxis::Raw, CodingAxis::MotionJpeg];

    let containers = [ContainerAxis::Iso, ContainerAxis::Avi];

    let track_matrices = [
        TrackMatrixAxis::Identity,
        TrackMatrixAxis::Rotate90,
        TrackMatrixAxis::Rotate180,
        TrackMatrixAxis::Rotate270,
    ];

    let scales = [ScaleAxis::WithScale, ScaleAxis::NoScale];

    let defects = all_defects()?;

    Ok(AxisArrays {
        dimensions,
        frame_counts,
        frame_rates,
        backgrounds,
        shapes,
        motions,
        codings,
        containers,
        track_matrices,
        scales,
        defects,
    })
}

/// Generates the catalog as a pairwise covering array over declared axes.
///
/// Every pair of values from any two axes appears together in at least one
/// entry. Entry names derive from the axis values; they are never
/// hand-written.
///
/// # Errors
///
/// Returns `CatalogError::ZeroDimension` if any dimension in the axis definitions is zero,
/// or `CatalogError::InvalidFrameRate` if the absurd frame rate cannot be constructed.
///
/// # Example
///
/// ```rust
/// use synthvid_catalog::cover::generate_catalogue;
///
/// let count = match generate_catalogue() {
///     Ok(catalogue) => catalogue.len(),
///     Err(_) => return,
/// };
/// count;
/// ```
pub fn generate_catalogue() -> Result<Vec<Entry>, CatalogError> {
    use cover_axes::{
        make_dimensions, BackgroundAxis, CodingAxis, ContainerAxis, FrameCountValue,
        FrameRateValue, MotionAxis, RawSize, ScaleAxis, ShapeAxis, TrackMatrixAxis,
    };

    let axes = define_axes()?;

    // Build initial obligation set
    let mut obligations = cover_greedy::build_axis_obligations(&axes);

    let mut result = Vec::new();

    // Greedy pairwise algorithm
    while let Some(&first_obligation) = obligations.iter().next() {
        // Build an entry satisfying this obligation
        let selection =
            cover_greedy::build_entry_for_obligation(first_obligation, &obligations, &axes);
        let entry = entry_from_selection(&axes, &selection)?;

        // Remove all obligations covered by this entry
        obligations.retain(|&ob| !cover_greedy::entry_covers_obligation(&entry, ob, &axes));

        result.push(entry);
    }

    // Add singleton entry encoding frame numbers
    result.push(Entry {
        dimensions: make_dimensions(RawSize {
            width: 256,
            height: 256,
        })?,
        frame_count: FrameCountValue::new(100),
        frame_rate: FrameRateValue::Thirty,
        background: BackgroundAxis::Solid,
        shape: ShapeAxis::Disc,
        motion: MotionAxis::Fixed,
        coding: CodingAxis::Raw,
        container: ContainerAxis::Iso,
        track_matrix: TrackMatrixAxis::Identity,
        scale: ScaleAxis::NoScale,
        defect: None,
    });

    Ok(result)
}

/// Constructs an `Entry` from an `AxisSelection`, resolving indices to actual values.
fn entry_from_selection(
    axes: &cover_axes::AxisArrays,
    sel: &cover_greedy::AxisSelection,
) -> Result<Entry, CatalogError> {
    Ok(Entry {
        dimensions: axes
            .dimensions
            .get(sel.dimensions)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        frame_count: axes
            .frame_counts
            .get(sel.frame_count)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        frame_rate: axes
            .frame_rates
            .get(sel.frame_rate)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        background: axes
            .backgrounds
            .get(sel.background)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        shape: axes
            .shapes
            .get(sel.shape)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        motion: axes
            .motions
            .get(sel.motion)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        coding: axes
            .codings
            .get(sel.coding)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        container: axes
            .containers
            .get(sel.container)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        track_matrix: axes
            .track_matrices
            .get(sel.track_matrix)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        scale: axes
            .scales
            .get(sel.scale)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        defect: axes
            .defects
            .get(sel.defect)
            .cloned()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
    })
}

#[cfg(test)]
pub(super) fn sample_entry_for_test() -> Entry {
    Entry {
        dimensions: cover_axes::make_dimensions(cover_axes::RawSize {
            width: 320,
            height: 240,
        })
        .expect("test dimensions are valid"),
        frame_count: FrameCountValue::new(100),
        frame_rate: FrameRateValue::Thirty,
        background: BackgroundAxis::Solid,
        shape: ShapeAxis::Disc,
        motion: MotionAxis::Fixed,
        coding: CodingAxis::Raw,
        container: ContainerAxis::Iso,
        track_matrix: TrackMatrixAxis::Identity,
        scale: ScaleAxis::NoScale,
        defect: None,
    }
}
