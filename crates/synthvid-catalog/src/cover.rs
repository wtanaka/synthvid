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

#[path = "cover_recording.rs"]
mod cover_recording;

#[path = "cover_depth.rs"]
mod cover_depth;

#[path = "cover_scene.rs"]
mod cover_scene;

use cover_capture_pairs::{CapturePair, CAPTURE_PAIRS};
pub use cover_error::CatalogError;
use cover_recording::RECORDED_PRESETS;

#[cfg(test)]
#[path = "cover_tests.rs"]
mod cover_tests;

#[cfg(test)]
#[path = "depth_const_tests.rs"]
mod depth_const_tests;

#[cfg(test)]
#[path = "cover_test_entries.rs"]
pub mod cover_test_entries;

#[cfg(test)]
pub(crate) use cover_test_entries::{avi_truncated_entry, entry_like, AxisValues, EntrySize};

use crate::scene_builder::{frame_rate_from_axis, SceneBuildError};
use core::num::{NonZeroU16, NonZeroU32};
use cover_container::{entry_container, EntryContainer};
use cover_names::{CaptureName, DefectName, DepthName, RecordingName, SceneName};
use synthvid_encode::defects::Defect;
use synthvid_scene::{Dimensions, DistinctCapture, FrameRatesError, RatePair};
use synthvid_video::json_names::JsonEntryName;
use synthvid_video::{CaptureRate, Container, Format, VideoSpec};

// Re-export axis types from the submodule
pub use cover_axes::{
    BackgroundAxis, CodingAxis, ContainerAxis, FrameCountValue, FrameRateValue, MotionAxis,
    RecordingAxis, ScaleAxis, ShapeAxis, TrackMatrixAxis,
};
pub use cover_names::{ManifestName, MediaName};
pub use cover_scene::{DeepAxis, EntryScene, FlatScene};

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
    /// The scene: a flat scene's background, shape and motion, or a deep shot.
    scene: EntryScene,
    /// Pixel encoding.
    coding: CodingAxis,
    /// Container format, with the capture rate only an ISO container can record.
    container: EntryContainer,
    /// Display matrix transformation.
    track_matrix: TrackMatrixAxis,
    /// Physical scale declaration.
    scale: ScaleAxis,
    /// Recording medium the camera records through.
    recording: RecordingAxis,
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

    /// The scene: a flat scene, or a deep shot, which has no flat axes.
    #[must_use]
    pub const fn scene(&self) -> EntryScene {
        self.scene
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

    /// Recording medium the camera records through.
    #[must_use]
    pub const fn recording(&self) -> RecordingAxis {
        self.recording
    }

    /// Depth effect of the scene: `None` for a flat entry, and the effect of a deep shot.
    #[must_use]
    pub const fn depth(&self) -> Option<DeepAxis> {
        self.scene.depth()
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
            "{:04}x{:04}-{}-{}{}-{}-{}-{}-{}{}-{}{}{}",
            self.dimensions.width.get().get(),
            self.dimensions.height.get().get(),
            self.frame_count,
            self.frame_rate,
            SceneName(self.scene),
            self.coding,
            self.container.axis(),
            self.track_matrix,
            self.scale,
            CaptureName(self.container.capture()),
            DefectName(self.defect.as_ref()),
            RecordingName(self.recording),
            DepthName(self.depth()),
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
    /// Returns a `SceneBuildError` if scene, camera or deep shot construction fails.
    pub fn to_video_spec(&self) -> Result<VideoSpec, SceneBuildError> {
        use crate::generator::{build_camera, build_recorder, build_scene};

        let encoding = self.coding.encoding();
        let rotation = self.track_matrix.rotation();

        // The catalogue crosses every container with every rotation, but only
        // ISO can record one; the AVI entries keep the axis in their name alone.
        let container = match self.container {
            EntryContainer::Iso { capture } => Container::Iso { rotation, capture },
            EntryContainer::Avi => Container::Avi,
        };

        let name = JsonEntryName::new(self.manifest_name().as_str())
            .map_err(|error| SceneBuildError::EntryName(Box::new(error)))?;

        let format = Format::new(encoding, container, self.defect.clone());

        let spec = match self.scene {
            EntryScene::Flat(flat) => {
                let scene = build_scene(self, flat)?;
                let camera = build_camera(self)?;
                VideoSpec::new(name, scene, camera, format)
            }
            EntryScene::Deep(DeepAxis::Parallax) => {
                let shot = cover_depth::parallax(&build_recorder(self)?)?;
                VideoSpec::deep(name, shot, format)
            }
            EntryScene::Deep(DeepAxis::Roll) => {
                let shot = cover_depth::roll(&build_recorder(self)?)?;
                VideoSpec::deep(name, shot, format)
            }
            EntryScene::Deep(DeepAxis::DiscBokeh) => {
                let shot = cover_depth::disc_bokeh(&build_recorder(self)?)?;
                VideoSpec::deep(name, shot, format)
            }
            EntryScene::Deep(DeepAxis::HexBokeh) => {
                let shot = cover_depth::hex_bokeh(&build_recorder(self)?)?;
                VideoSpec::deep(name, shot, format)
            }
        };
        spec.map_err(|error| SceneBuildError::VideoSpec(Box::new(error)))
    }
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

    let mut result = covering_array()?;

    // Add singleton entry encoding frame numbers
    result.push(Entry {
        dimensions: make_dimensions(RawSize {
            width: NonZeroU16::new(256),
            height: NonZeroU16::new(256),
        })?,
        frame_count: FrameCountValue::new(100)?,
        frame_rate: FrameRateValue::Thirty,
        scene: EntryScene::Flat(FlatScene::new(
            BackgroundAxis::Solid,
            ShapeAxis::Disc,
            MotionAxis::Fixed,
        )),
        coding: CodingAxis::Raw,
        container: EntryContainer::Iso {
            capture: CaptureRate::PlaybackOnly,
        },
        track_matrix: TrackMatrixAxis::Identity,
        scale: ScaleAxis::Undeclared,
        recording: RecordingAxis::Ideal,
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
        scene: EntryScene::Flat(FlatScene::new(
            BackgroundAxis::Solid,
            ShapeAxis::Disc,
            MotionAxis::Fixed,
        )),
        coding: CodingAxis::MotionJpeg,
        container: EntryContainer::Iso {
            capture: CaptureRate::PlaybackOnly,
        },
        track_matrix: TrackMatrixAxis::Identity,
        scale: ScaleAxis::Undeclared,
        recording: RecordingAxis::Ideal,
        defect: Some(Defect::NoContainerSize),
    });

    result.extend(capture_singletons()?);
    result.extend(recording_singletons()?);
    result.extend(depth_singletons()?);

    Ok(result)
}

/// The entries of the pairwise covering array over the declared axes, in the order the greedy
/// construction picks them.
///
/// # Errors
///
/// Returns `CatalogError::UnsatisfiableObligation` if a required pair of values cannot be
/// combined in any entry, and every error the axis definitions and entry selection return.
fn covering_array() -> Result<Vec<Entry>, CatalogError> {
    let axes = cover_axes::define_axes()?;

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
        scene: EntryScene::Flat(FlatScene::new(
            BackgroundAxis::Solid,
            ShapeAxis::Disc,
            MotionAxis::Fixed,
        )),
        coding: CodingAxis::Raw,
        container: EntryContainer::Iso {
            capture: CaptureRate::Distinct(distinct),
        },
        track_matrix: TrackMatrixAxis::Identity,
        scale: ScaleAxis::Undeclared,
        recording: RecordingAxis::Ideal,
        defect: None,
    })
}

/// The ten ISO singletons that record through a sensor or film medium: each non-ideal preset in
/// [`RECORDED_PRESETS`], once with raw pixels and once with motion-JPEG.
///
/// The covering array keeps every entry ideal, so these carry the recording alone. Each preset
/// is a noise or grain property a reader meets in practice: a sensor's shot and read noise at
/// two sensitivities, a sensor's fixed pattern, and film grain fine and coarse. A raw entry
/// shows the noise exactly as the medium wrote it. A motion-JPEG entry shows how lossy coding
/// treats that noise.
///
/// # Errors
///
/// Returns every error [`recording_singleton`] returns.
fn recording_singletons() -> Result<Vec<Entry>, CatalogError> {
    let mut entries = Vec::new();
    for recording in RECORDED_PRESETS {
        for coding in [CodingAxis::Raw, CodingAxis::MotionJpeg] {
            entries.push(recording_singleton(recording, coding)?);
        }
    }
    Ok(entries)
}

/// A 64 by 48 ten-frame ISO singleton with a gradient background, a disc moving in a fixed
/// position, and the given recording and coding.
///
/// The gradient varies the tone across the frame, so the noise of the recording varies with
/// the tone it sits on.
///
/// # Errors
///
/// Returns `CatalogError::ZeroDimension` if the dimensions cannot be made, and
/// `CatalogError::ZeroFrameCount` if the frame count is zero.
fn recording_singleton(
    recording: RecordingAxis,
    coding: CodingAxis,
) -> Result<Entry, CatalogError> {
    use cover_axes::{make_dimensions, RawSize};

    Ok(Entry {
        dimensions: make_dimensions(RawSize {
            width: NonZeroU16::new(64),
            height: NonZeroU16::new(48),
        })?,
        frame_count: FrameCountValue::new(10)?,
        frame_rate: FrameRateValue::Thirty,
        scene: EntryScene::Flat(FlatScene::new(
            BackgroundAxis::Gradient,
            ShapeAxis::Disc,
            MotionAxis::Fixed,
        )),
        coding,
        container: EntryContainer::Iso {
            capture: CaptureRate::PlaybackOnly,
        },
        track_matrix: TrackMatrixAxis::Identity,
        scale: ScaleAxis::Undeclared,
        recording,
        defect: None,
    })
}

/// The four deep shots, one per depth effect, after the recording singletons. The covering array
/// does not hold them, so each is built here rather than generated.
///
/// # Errors
///
/// Returns every error [`depth_singleton`] returns.
fn depth_singletons() -> Result<Vec<Entry>, CatalogError> {
    // Parallax: layers at different depths cross the frame at different speeds as the camera
    // moves. Roll: a camera turning about its optical axis rotates layers at their own depths.
    // Disc bokeh: out-of-focus highlights blur to the outline of a round iris, and the highlight
    // at the focus distance stays a point. Hexagonal bokeh: the same highlights blur to the
    // outline of a six-bladed iris.
    [
        DeepAxis::Parallax,
        DeepAxis::Roll,
        DeepAxis::DiscBokeh,
        DeepAxis::HexBokeh,
    ]
    .into_iter()
    .map(depth_singleton)
    .collect()
}

/// A 32 by 24 eight-frame ISO singleton that is a deep shot of the given depth effect.
///
/// The shot's stage and rig come from `cover_depth`, which takes this entry's recorder. The
/// entry holds no background, shape or motion, since a deep shot has none of them.
///
/// # Errors
///
/// Returns `CatalogError::ZeroDimension` or `CatalogError::ZeroFrameCount` if the entry's
/// dimensions or frame count cannot be made.
fn depth_singleton(deep: DeepAxis) -> Result<Entry, CatalogError> {
    use cover_axes::{make_dimensions, RawSize};

    Ok(Entry {
        dimensions: make_dimensions(RawSize {
            width: NonZeroU16::new(32),
            height: NonZeroU16::new(24),
        })?,
        frame_count: FrameCountValue::new(8)?,
        frame_rate: FrameRateValue::Thirty,
        scene: EntryScene::Deep(deep),
        coding: CodingAxis::Raw,
        container: EntryContainer::Iso {
            capture: CaptureRate::PlaybackOnly,
        },
        track_matrix: TrackMatrixAxis::Identity,
        scale: ScaleAxis::Undeclared,
        recording: RecordingAxis::Ideal,
        defect: None,
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
        scene: EntryScene::Flat(FlatScene::new(
            axes.backgrounds
                .get(sel.background.0)
                .copied()
                .ok_or(CatalogError::AxisIndexOutOfRange)?,
            axes.shapes
                .get(sel.shape.0)
                .copied()
                .ok_or(CatalogError::AxisIndexOutOfRange)?,
            axes.motions
                .get(sel.motion.0)
                .copied()
                .ok_or(CatalogError::AxisIndexOutOfRange)?,
        )),
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
        recording: RecordingAxis::Ideal,
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
        scene: EntryScene::Flat(FlatScene::new(
            BackgroundAxis::Solid,
            ShapeAxis::Disc,
            MotionAxis::Fixed,
        )),
        coding: CodingAxis::Raw,
        container: EntryContainer::Iso {
            capture: CaptureRate::PlaybackOnly,
        },
        track_matrix: TrackMatrixAxis::Identity,
        scale: ScaleAxis::Undeclared,
        recording: RecordingAxis::Ideal,
        defect: None,
    })
}

#[cfg(test)]
impl Entry {
    /// Replaces every axis value of this entry, including its defect, and makes the entry a flat
    /// scene with the given background, shape and motion. The container axis goes through
    /// [`Entry::set_container_axis`], so an existing ISO capture rate is kept (through `with_axis`).
    pub(crate) fn set_axes(&mut self, axes: AxisValues) {
        self.frame_rate = axes.frame_rate;
        self.scene = EntryScene::Flat(FlatScene::new(axes.background, axes.shape, axes.motion));
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

    /// Replaces the recording medium of this entry.
    pub(crate) fn set_recording(&mut self, recording: RecordingAxis) {
        self.recording = recording;
    }
}
