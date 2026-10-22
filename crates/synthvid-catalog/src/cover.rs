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

#[path = "cover_names.rs"]
mod cover_names;

#[path = "cover_values.rs"]
mod cover_values;

#[path = "cover_capture_pairs.rs"]
mod cover_capture_pairs;

#[path = "cover_error.rs"]
mod cover_error;

#[path = "cover_container.rs"]
mod cover_container;

use cover_capture_pairs::{CapturePair, CAPTURE_PAIRS};
pub use cover_error::CatalogError;

#[cfg(test)]
#[path = "cover_tests.rs"]
mod cover_tests;

#[cfg(test)]
#[path = "cover_test_entries.rs"]
pub mod cover_test_entries;

#[cfg(test)]
pub(crate) use cover_test_entries::{avi_truncated_entry, entry_like, AxisValues, EntrySize};

use crate::scene_builder::{frame_rate_from_axis, SceneBuildError};
use core::num::{NonZeroU16, NonZeroU32};
use cover_container::EntryContainer;
use cover_names::{CaptureName, DefectName};
use synthvid_encode::defects::Defect;
use synthvid_scene::{Dimensions, DistinctCapture, FrameRatesError, RatePair};
use synthvid_video::json_names::JsonEntryName;
use synthvid_video::{CaptureRate, Container, DisplayRotation, Encoding, VideoSpec};

// Re-export axis types from the submodule
pub use cover_axes::{
    BackgroundAxis, CodingAxis, ContainerAxis, FrameCountValue, FrameRateValue, MotionAxis,
    ScaleAxis, ShapeAxis, TrackMatrixAxis,
};
pub use cover_names::{ManifestName, MediaName};

/// A catalog entry: a scene description with all parameters specified.
///
/// Names (manifest and media paths) are derived from the entry's axes,
/// never hand-written. Two entries with identical axis values produce
/// identical names; two entries differing in any axis produce different names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    /// Frame dimensions.
    dimensions: Dimensions,
    /// Number of frames.
    frame_count: FrameCountValue,
    /// Frame rate.
    frame_rate: FrameRateValue,
    /// Background type.
    background: BackgroundAxis,
    /// Shape type.
    shape: ShapeAxis,
    /// Motion type.
    motion: MotionAxis,
    /// Pixel encoding.
    coding: CodingAxis,
    /// Container format, with the capture rate only an ISO container can record.
    container: EntryContainer,
    /// Display matrix transformation.
    track_matrix: TrackMatrixAxis,
    /// Physical scale declaration.
    scale: ScaleAxis,
    /// Optional defect.
    defect: Option<Defect>,
}

impl Entry {
    /// Frame dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    /// Number of frames.
    #[must_use]
    pub const fn frame_count(&self) -> FrameCountValue {
        self.frame_count
    }

    /// Frame rate.
    #[must_use]
    pub const fn frame_rate(&self) -> FrameRateValue {
        self.frame_rate
    }

    /// Background type.
    #[must_use]
    pub const fn background(&self) -> BackgroundAxis {
        self.background
    }

    /// Shape type.
    #[must_use]
    pub const fn shape(&self) -> ShapeAxis {
        self.shape
    }

    /// Motion type.
    #[must_use]
    pub const fn motion(&self) -> MotionAxis {
        self.motion
    }

    /// Pixel encoding.
    #[must_use]
    pub const fn coding(&self) -> CodingAxis {
        self.coding
    }

    /// Container format.
    #[must_use]
    pub const fn container(&self) -> ContainerAxis {
        self.container.axis()
    }

    /// Display matrix transformation.
    #[must_use]
    pub const fn track_matrix(&self) -> TrackMatrixAxis {
        self.track_matrix
    }

    /// Physical scale declaration.
    #[must_use]
    pub const fn scale(&self) -> ScaleAxis {
        self.scale
    }

    /// Capture rate declared in the metadata.
    #[must_use]
    pub const fn capture(&self) -> CaptureRate {
        self.container.capture()
    }

    /// Optional defect.
    #[must_use]
    pub const fn defect(&self) -> Option<&Defect> {
        self.defect.as_ref()
    }

    /// Generates the manifest name for this entry.
    ///
    /// Derived deterministically from all axis values. Two entries with
    /// identical axes produce identical names; different axes produce
    /// different names.
    #[must_use]
    pub fn manifest_name(&self) -> ManifestName {
        ManifestName(format!(
            "{:04}x{:04}-{}-{}-{}-{}-{}-{}-{}-{}-{}{}-{}",
            self.dimensions.width.get().get(),
            self.dimensions.height.get().get(),
            self.frame_count,
            self.frame_rate,
            self.background,
            self.shape,
            self.motion,
            self.coding,
            self.container.axis(),
            self.track_matrix,
            self.scale,
            CaptureName(self.container.capture()),
            DefectName(self.defect.as_ref()),
        ))
    }

    /// Generates the media file name for this entry.
    ///
    /// Uses the same derivation as the manifest name, ensuring they stay in sync.
    /// The extension is determined by the container type, and the name's `Display`
    /// gives the text suitable for use as a filename.
    #[must_use]
    pub fn media_name(&self) -> MediaName {
        MediaName::new(self.manifest_name(), self.container.axis())
    }

    /// Converts this entry to a `VideoSpec` with explicit parameters.
    ///
    /// # Errors
    ///
    /// Returns a `SceneBuildError` if scene construction fails.
    pub fn to_video_spec(&self) -> Result<VideoSpec, SceneBuildError> {
        use crate::generator::build_scene;

        let scene = build_scene(self)?;

        let encoding = match self.coding {
            CodingAxis::Raw => Encoding::Raw,
            CodingAxis::MotionJpeg => Encoding::Mjpeg {
                quality: synthvid_encode::Quality::Q75,
                chroma: synthvid_encode::ChromaSampling::Yuv420,
            },
        };

        let rotation = match self.track_matrix {
            TrackMatrixAxis::Identity => DisplayRotation::Identity,
            TrackMatrixAxis::Rotate90 => DisplayRotation::Rot90,
            TrackMatrixAxis::Rotate180 => DisplayRotation::Rot180,
            TrackMatrixAxis::Rotate270 => DisplayRotation::Rot270,
        };

        // The catalogue crosses every container with every rotation, but only
        // ISO can record one; the AVI entries keep the axis in their name alone.
        let container = match self.container {
            EntryContainer::Iso { capture } => Container::Iso { rotation, capture },
            EntryContainer::Avi => Container::Avi,
        };

        let name = JsonEntryName::new(self.manifest_name().as_str())
            .map_err(SceneBuildError::EntryName)?;

        VideoSpec::new(name, scene, encoding, container, self.defect.clone())
            .map_err(SceneBuildError::VideoSpec)
    }
}

/// The dimensions axis values, in declared order.
fn axis_dimensions() -> Result<[Dimensions; 5], CatalogError> {
    use cover_axes::{make_dimensions, RawSize};

    Ok([
        make_dimensions(RawSize {
            width: NonZeroU16::new(16),
            height: NonZeroU16::new(16),
        })?,
        make_dimensions(RawSize {
            width: NonZeroU16::new(320),
            height: NonZeroU16::new(240),
        })?,
        make_dimensions(RawSize {
            width: NonZeroU16::new(640),
            height: NonZeroU16::new(480),
        })?,
        make_dimensions(RawSize {
            width: NonZeroU16::new(1280),
            height: NonZeroU16::new(720),
        })?,
        make_dimensions(RawSize {
            width: NonZeroU16::new(1920),
            height: NonZeroU16::new(1080),
        })?,
    ])
}

/// The frame count axis values, in declared order.
fn axis_frame_counts() -> Result<[FrameCountValue; 5], CatalogError> {
    Ok([
        FrameCountValue::new(1)?,
        FrameCountValue::new(10)?,
        FrameCountValue::new(100)?,
        FrameCountValue::new(1000)?,
        FrameCountValue::new(20000)?,
    ])
}

/// Defines all axis values for the catalog.
fn define_axes() -> Result<cover_axes::AxisArrays, CatalogError> {
    use cover_axes::{all_defects, AxisArrays};

    // Define all axis values in declared order
    let dimensions = axis_dimensions()?;
    let frame_counts = axis_frame_counts()?;

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

    let scales = [ScaleAxis::Declared, ScaleAxis::Undeclared];

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
/// or `CatalogError::InvalidFrameRate` if the absurd frame rate cannot be constructed,
/// or `CatalogError::UnsatisfiableObligation` if a required pair of values cannot
/// be combined in any entry.
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
    use cover_axes::{make_dimensions, RawSize};

    let axes = define_axes()?;

    // Build initial obligation set
    let mut obligations = cover_greedy::build_axis_obligations(&axes);

    let mut result = Vec::new();

    // Greedy pairwise algorithm
    while let Some(&first_obligation) = obligations.iter().next() {
        // Build an entry satisfying this obligation
        let selection =
            cover_greedy::build_entry_for_obligation(first_obligation, &obligations, &axes)?;
        let entry = entry_from_selection(&axes, &selection)?;
        // Every pass removes at least the obligation it started from, so the
        // loop ends.
        if cover_greedy::entry_covers_obligation(&entry, first_obligation, &axes)
            == cover_values::Coverage::Uncovered
        {
            return Err(CatalogError::UnsatisfiableObligation);
        }

        // Remove all obligations covered by this entry
        obligations.retain(|&ob| {
            cover_greedy::entry_covers_obligation(&entry, ob, &axes)
                == cover_values::Coverage::Uncovered
        });

        result.push(entry);
    }

    // Add singleton entry encoding frame numbers
    result.push(Entry {
        dimensions: make_dimensions(RawSize {
            width: NonZeroU16::new(256),
            height: NonZeroU16::new(256),
        })?,
        frame_count: FrameCountValue::new(100)?,
        frame_rate: FrameRateValue::Thirty,
        background: BackgroundAxis::Solid,
        shape: ShapeAxis::Disc,
        motion: MotionAxis::Fixed,
        coding: CodingAxis::Raw,
        container: EntryContainer::Iso {
            capture: CaptureRate::PlaybackOnly,
        },
        track_matrix: TrackMatrixAxis::Identity,
        scale: ScaleAxis::Undeclared,
        defect: None,
    });

    // Singleton entry whose container records no size: a reader without a
    // motion-JPEG decoder finds none, and one with it reads the frames'.
    result.push(Entry {
        dimensions: make_dimensions(RawSize {
            width: NonZeroU16::new(16),
            height: NonZeroU16::new(16),
        })?,
        frame_count: FrameCountValue::new(10)?,
        frame_rate: FrameRateValue::Thirty,
        background: BackgroundAxis::Solid,
        shape: ShapeAxis::Disc,
        motion: MotionAxis::Fixed,
        coding: CodingAxis::MotionJpeg,
        container: EntryContainer::Iso {
            capture: CaptureRate::PlaybackOnly,
        },
        track_matrix: TrackMatrixAxis::Identity,
        scale: ScaleAxis::Undeclared,
        defect: Some(Defect::NoContainerSize),
    });

    result.extend(capture_singletons()?);

    Ok(result)
}

/// The nine ISO singletons whose metadata declares a capture rate, each at a playback rate from
/// [`CAPTURE_PAIRS`]. A reader that looks only at the container's playback rate gets every
/// time-derived quantity wrong by the ratio between the two rates.
///
/// # Errors
///
/// Returns every error [`capture_singleton`] returns for any pair in [`CAPTURE_PAIRS`]:
/// `CatalogError::InvalidFrameRate`, `CatalogError::CaptureEqualsPlayback`,
/// `CatalogError::ZeroDimension` and `CatalogError::ZeroFrameCount`.
fn capture_singletons() -> Result<Vec<Entry>, CatalogError> {
    CAPTURE_PAIRS.iter().map(capture_singleton).collect()
}

/// A 16 by 16 ten-frame ISO singleton played at the pair's playback rate and captured at
/// its capture fraction, frames per second, in the pair's carrier.
///
/// # Errors
///
/// Returns `CatalogError::InvalidFrameRate` if the capture or playback rate is not a positive
/// frame rate, `CatalogError::CaptureEqualsPlayback` if the pair's capture rate equals its
/// playback rate, `CatalogError::ZeroDimension` if the dimensions cannot be made, and
/// `CatalogError::ZeroFrameCount` if the frame count is zero.
fn capture_singleton(pair: &CapturePair) -> Result<Entry, CatalogError> {
    use cover_axes::{make_dimensions, RawSize};

    let denom = NonZeroU32::new(pair.capture.denom.0).ok_or(CatalogError::InvalidFrameRate)?;
    let capture = synthvid_scene::FrameRate::from_fraction(pair.capture.numer.0, denom)
        .map_err(|synthvid_scene::FrameRateError::NonPositive| CatalogError::InvalidFrameRate)?;
    let playback =
        frame_rate_from_axis(pair.playback).map_err(|_error| CatalogError::InvalidFrameRate)?;
    let distinct = DistinctCapture::checked(RatePair { playback, capture }, pair.carrier)
        .map_err(|FrameRatesError::CaptureEqualsPlayback| CatalogError::CaptureEqualsPlayback)?;
    Ok(Entry {
        dimensions: make_dimensions(RawSize {
            width: NonZeroU16::new(16),
            height: NonZeroU16::new(16),
        })?,
        frame_count: FrameCountValue::new(10)?,
        frame_rate: pair.playback,
        background: BackgroundAxis::Solid,
        shape: ShapeAxis::Disc,
        motion: MotionAxis::Fixed,
        coding: CodingAxis::Raw,
        container: EntryContainer::Iso {
            capture: CaptureRate::Distinct(distinct),
        },
        track_matrix: TrackMatrixAxis::Identity,
        scale: ScaleAxis::Undeclared,
        defect: None,
    })
}

/// The container an entry takes from its selected axis value. A selected ISO container
/// starts with no capture rate beyond the playback rate.
fn entry_container(
    axes: &cover_axes::AxisArrays,
    sel: &cover_greedy::AxisSelection,
) -> Result<EntryContainer, CatalogError> {
    let value = axes
        .containers
        .get(sel.container.0)
        .copied()
        .ok_or(CatalogError::AxisIndexOutOfRange)?;
    Ok(match value {
        ContainerAxis::Iso => EntryContainer::Iso {
            capture: CaptureRate::PlaybackOnly,
        },
        ContainerAxis::Avi => EntryContainer::Avi,
    })
}

/// Constructs an `Entry` from an `AxisSelection`, resolving indices to actual values.
fn entry_from_selection(
    axes: &cover_axes::AxisArrays,
    sel: &cover_greedy::AxisSelection,
) -> Result<Entry, CatalogError> {
    Ok(Entry {
        dimensions: axes
            .dimensions
            .get(sel.dimensions.0)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        frame_count: axes
            .frame_counts
            .get(sel.frame_count.0)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        frame_rate: axes
            .frame_rates
            .get(sel.frame_rate.0)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        background: axes
            .backgrounds
            .get(sel.background.0)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        shape: axes
            .shapes
            .get(sel.shape.0)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        motion: axes
            .motions
            .get(sel.motion.0)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        coding: axes
            .codings
            .get(sel.coding.0)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        container: entry_container(axes, sel)?,
        track_matrix: axes
            .track_matrices
            .get(sel.track_matrix.0)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        scale: axes
            .scales
            .get(sel.scale.0)
            .copied()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
        defect: axes
            .defects
            .get(sel.defect.0)
            .cloned()
            .ok_or(CatalogError::AxisIndexOutOfRange)?,
    })
}

#[cfg(test)]
pub(super) fn sample_entry_for_test() -> Result<Entry, synthvid_testkit::Fail> {
    use synthvid_testkit::OrFail as _;
    Ok(Entry {
        dimensions: cover_axes::make_dimensions(cover_axes::RawSize {
            width: NonZeroU16::new(320),
            height: NonZeroU16::new(240),
        })
        .or_fail()?,
        frame_count: FrameCountValue::new(100).or_fail()?,
        frame_rate: FrameRateValue::Thirty,
        background: BackgroundAxis::Solid,
        shape: ShapeAxis::Disc,
        motion: MotionAxis::Fixed,
        coding: CodingAxis::Raw,
        container: EntryContainer::Iso {
            capture: CaptureRate::PlaybackOnly,
        },
        track_matrix: TrackMatrixAxis::Identity,
        scale: ScaleAxis::Undeclared,
        defect: None,
    })
}

#[cfg(test)]
impl Entry {
    /// Replaces every axis value of this entry, including its defect. The container axis
    /// goes through [`Entry::set_container_axis`], so an existing ISO capture rate is kept
    /// (through `with_axis`).
    pub(crate) fn set_axes(&mut self, axes: AxisValues) {
        self.frame_rate = axes.frame_rate;
        self.background = axes.background;
        self.shape = axes.shape;
        self.motion = axes.motion;
        self.coding = axes.coding;
        self.set_container_axis(axes.container);
        self.track_matrix = axes.track_matrix;
        self.scale = axes.scale;
        self.defect = axes.defect;
    }

    /// Replaces the container axis, keeping the capture rate an ISO container already had.
    pub(crate) fn set_container_axis(&mut self, axis: ContainerAxis) {
        self.container = self.container.with_axis(axis);
    }

    /// Replaces the defect of this entry.
    pub(crate) fn set_defect(&mut self, defect: Option<Defect>) {
        self.defect = defect;
    }
}
