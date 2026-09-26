//! Axis definitions and helpers for catalog generation.

use crate::cover::CatalogError;
use synthvid_encode::defects::{Axis, BoxPath, BoxTag, ByteOffset, Defect};
use synthvid_scene::{Dimensions, Height, Width};

/// The 15 declared defect slots (index 0 is always `None`, meaning "no defect").
pub(super) type DefectSlots = [Option<Defect>; 15];

/// Frame count axis.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct FrameCountValue(u32);

impl FrameCountValue {
    /// Creates a frame count.
    #[must_use]
    pub const fn new(count: u32) -> Self {
        Self(count)
    }

    /// Gets the frame count value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Frame rate axis.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum FrameRateValue {
    /// 30 frames per second
    Thirty,
    /// 60 frames per second
    Sixty,
    /// 120 frames per second
    OneTwenty,
    /// 240 frames per second
    TwoForty,
    /// 30000/1001 frames per second (NTSC)
    Ntsc,
    /// An absurdly high frame rate for defect testing
    Absurd,
}

/// Background axis.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum BackgroundAxis {
    /// Solid color
    Solid,
    /// Checkerboard pattern
    Checker,
    /// Linear gradient
    Gradient,
    /// Random blobs
    Blobs,
    /// Grid pattern
    Grid,
}

/// Shape axis.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ShapeAxis {
    /// Disc
    Disc,
    /// Rectangle
    Rect,
    /// Polygon
    Polygon,
    /// Cross
    Cross,
}

/// Motion axis: different kinds of motion with reasonable parameters.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum MotionAxis {
    /// Fixed position
    Fixed,
    /// Linear motion
    Linear,
    /// Ballistic motion
    Ballistic,
    /// Circular motion
    Circular,
    /// Oscillating motion
    Oscillating,
    /// Random walk
    Walk,
}

/// Coding axis: how pixels are encoded.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum CodingAxis {
    /// Raw pixel data
    Raw,
    /// Motion JPEG
    MotionJpeg,
}

/// Container axis: file format.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ContainerAxis {
    /// ISO base media (MP4-like)
    Iso,
    /// AVI container
    Avi,
}

/// Track display matrix axis: display transformation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum TrackMatrixAxis {
    /// Identity (no rotation)
    Identity,
    /// 90-degree rotation
    Rotate90,
    /// 180-degree rotation
    Rotate180,
    /// 270-degree rotation
    Rotate270,
}

/// Scale axis: whether the scene has a physical scale.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ScaleAxis {
    /// Scene has a physical scale
    WithScale,
    /// Scene has no physical scale
    NoScale,
}

/// Holds all axis definitions for the catalog.
pub(super) struct AxisArrays {
    /// Dimension values
    pub dimensions: [Dimensions; 5],
    /// Frame count values
    pub frame_counts: [FrameCountValue; 5],
    /// Frame rate values
    pub frame_rates: [FrameRateValue; 6],
    /// Background values
    pub backgrounds: [BackgroundAxis; 5],
    /// Shape values
    pub shapes: [ShapeAxis; 4],
    /// Motion values
    pub motions: [MotionAxis; 6],
    /// Coding values
    pub codings: [CodingAxis; 2],
    /// Container values
    pub containers: [ContainerAxis; 2],
    /// Track matrix values
    pub track_matrices: [TrackMatrixAxis; 4],
    /// Scale values
    pub scales: [ScaleAxis; 2],
    /// Defect values
    pub defects: DefectSlots,
}

/// Raw width/height pair before validation.
#[derive(Copy, Clone)]
pub(super) struct RawSize {
    /// Width value before validation
    pub(super) width: u16,
    /// Height value before validation
    pub(super) height: u16,
}

/// Creates a Dimensions value from width and height u16 values.
/// Returns an error if either dimension is zero.
pub(super) fn make_dimensions(size: RawSize) -> Result<Dimensions, CatalogError> {
    let width = Width::new(size.width).map_err(|_| CatalogError::ZeroDimension)?;
    let height = Height::new(size.height).map_err(|_| CatalogError::ZeroDimension)?;
    Ok(Dimensions::new(width, height))
}

/// Maps a Defect variant to its stable name fragment.
/// Matches on the enum variant only, ignoring payload differences.
pub(super) const fn defect_variant_str(d: &Defect) -> &'static str {
    match d {
        Defect::TruncateAt(_) => "trunc-at",
        Defect::TruncateBox(_) => "trunc-box",
        Defect::ZeroLengthPayload(_) => "zero-pay",
        Defect::OversizedBoxLength(_) => "over-len",
        Defect::UndersizedBoxLength(_) => "under-len",
        Defect::OffsetPastEnd(_) => "off-end",
        Defect::DuplicateBox(_) => "dup-box",
        Defect::UnknownBox { .. } => "unk-box",
        Defect::ZeroFrameRate => "zero-fps",
        Defect::ZeroDimension(_) => "zero-dim",
        Defect::AbsurdFrameRate(_) => "absurd-fps",
        Defect::NonMonotonicTimestamps => "non-mono",
        Defect::DeclaredCountMismatch { .. } => "count-mis",
        Defect::ReorderedBoxes(_, _) => "reorder",
    }
}

/// Returns true if a defect targets an ISO base-media box path and therefore
/// cannot be applied to a non-ISO (e.g. AVI/RIFF) container.
pub(super) const fn defect_requires_iso_boxes(d: &Defect) -> bool {
    matches!(
        d,
        Defect::TruncateBox(_)
            | Defect::ZeroLengthPayload(_)
            | Defect::OversizedBoxLength(_)
            | Defect::UndersizedBoxLength(_)
            | Defect::OffsetPastEnd(_)
            | Defect::DuplicateBox(_)
            | Defect::ReorderedBoxes(_, _)
            | Defect::NonMonotonicTimestamps
    )
}

/// Returns true if the container at `container_index` and the defect at
/// `defect_index` (indices into `axes.containers` / `axes.defects`) can be
/// combined in one entry. Out-of-range indices are treated as compatible
/// (nothing to rule out).
pub(super) fn container_defect_compatible(
    axes: &AxisArrays,
    container_index: usize,
    defect_index: usize,
) -> bool {
    let is_avi = axes.containers.get(container_index) == Some(&ContainerAxis::Avi);
    if !is_avi {
        return true;
    }
    match axes.defects.get(defect_index) {
        Some(Some(d)) => !defect_requires_iso_boxes(d),
        _ => true,
    }
}

/// Returns true if the given dimensions/frame-count/coding/container
/// combination fits within its container's capacity. Raw (uncompressed)
/// frame data grows linearly with pixel count and frame count and can
/// exceed a classic container's 32-bit size limit; motion-JPEG frames are
/// compressed and stay well under it. Both the ISO `mdat` box and the AVI
/// `movi`/`idx1` structures share this same 32-bit limit, so both
/// containers are checked identically via
/// [`synthvid_encode::raw_frames_fit_classic_container`], the single place
/// that limit is defined. Out-of-range indices are treated as compatible
/// (nothing to rule out).
pub(super) fn raw_avi_size_compatible(
    axes: &AxisArrays,
    dimensions_index: usize,
    frame_count_index: usize,
    coding_index: usize,
    container_index: usize,
) -> bool {
    let is_classic_container = axes.containers.get(container_index).is_some();
    let is_raw = axes.codings.get(coding_index) == Some(&CodingAxis::Raw);
    if !is_classic_container || !is_raw {
        return true;
    }
    let Some(dims) = axes.dimensions.get(dimensions_index) else {
        return true;
    };
    let Some(frame_count) = axes.frame_counts.get(frame_count_index) else {
        return true;
    };
    synthvid_encode::raw_frames_fit_classic_container(dims.width, dims.height, frame_count.get())
}

/// Build an `AbsurdFrameRate` defect, propagating any errors.
pub(super) fn build_absurd_frame_rate_defect() -> Result<Defect, CatalogError> {
    // Create a frame rate of 1_000_001 / 1
    let nz_one = core::num::NonZeroI64::new(1).ok_or(CatalogError::InvalidFrameRate)?;

    let ratio = synthvid_scene::Ratio::new(1_000_001, nz_one)
        .map_err(|_| CatalogError::InvalidFrameRate)?;

    let frame_rate =
        synthvid_scene::FrameRate::new(ratio).map_err(|_| CatalogError::InvalidFrameRate)?;

    Ok(Defect::AbsurdFrameRate(frame_rate))
}

/// All possible defect instances for the catalog.
pub(super) fn all_defects() -> Result<DefectSlots, CatalogError> {
    Ok([
        None,
        Some(Defect::TruncateAt(ByteOffset::new(1000))),
        Some(Defect::TruncateBox(
            BoxPath::root().child(BoxTag::new(*b"moov")),
        )),
        Some(Defect::ZeroLengthPayload(
            BoxPath::root().child(BoxTag::new(*b"mdat")),
        )),
        Some(Defect::OversizedBoxLength(
            BoxPath::root().child(BoxTag::new(*b"ftyp")),
        )),
        Some(Defect::UndersizedBoxLength(
            BoxPath::root().child(BoxTag::new(*b"moov")),
        )),
        Some(Defect::OffsetPastEnd(
            BoxPath::root().child(BoxTag::new(*b"mdat")),
        )),
        Some(Defect::DuplicateBox(
            BoxPath::root()
                .child(BoxTag::new(*b"moov"))
                .child(BoxTag::new(*b"trak")),
        )),
        Some(Defect::UnknownBox {
            after: BoxPath::root(),
            tag: BoxTag::new(*b"UNKN"),
        }),
        Some(Defect::ZeroFrameRate),
        Some(Defect::ZeroDimension(Axis::Width)),
        Some(build_absurd_frame_rate_defect()?),
        Some(Defect::NonMonotonicTimestamps),
        Some(Defect::DeclaredCountMismatch { declared: 42 }),
        Some(Defect::ReorderedBoxes(
            BoxPath::root().child(BoxTag::new(*b"moov")),
            BoxPath::root().child(BoxTag::new(*b"mdat")),
        )),
    ])
}

/// Gets the index for a frame rate value.
pub(super) const fn get_frame_rate_index(fr: FrameRateValue) -> usize {
    match fr {
        FrameRateValue::Thirty => 0,
        FrameRateValue::Sixty => 1,
        FrameRateValue::OneTwenty => 2,
        FrameRateValue::TwoForty => 3,
        FrameRateValue::Ntsc => 4,
        FrameRateValue::Absurd => 5,
    }
}

/// Gets the index for a background value.
pub(super) const fn get_background_index(bg: BackgroundAxis) -> usize {
    match bg {
        BackgroundAxis::Solid => 0,
        BackgroundAxis::Checker => 1,
        BackgroundAxis::Gradient => 2,
        BackgroundAxis::Blobs => 3,
        BackgroundAxis::Grid => 4,
    }
}

/// Gets the index for a shape value.
pub(super) const fn get_shape_index(sh: ShapeAxis) -> usize {
    match sh {
        ShapeAxis::Disc => 0,
        ShapeAxis::Rect => 1,
        ShapeAxis::Polygon => 2,
        ShapeAxis::Cross => 3,
    }
}

/// Gets the index for a motion value.
pub(super) const fn get_motion_index(mot: MotionAxis) -> usize {
    match mot {
        MotionAxis::Fixed => 0,
        MotionAxis::Linear => 1,
        MotionAxis::Ballistic => 2,
        MotionAxis::Circular => 3,
        MotionAxis::Oscillating => 4,
        MotionAxis::Walk => 5,
    }
}

/// Gets the index for a coding value.
pub(super) const fn get_coding_index(cod: CodingAxis) -> usize {
    match cod {
        CodingAxis::Raw => 0,
        CodingAxis::MotionJpeg => 1,
    }
}

/// Gets the index for a container value.
pub(super) const fn get_container_index(cont: ContainerAxis) -> usize {
    match cont {
        ContainerAxis::Iso => 0,
        ContainerAxis::Avi => 1,
    }
}

/// Gets the index for a track matrix value.
pub(super) const fn get_track_matrix_index(tm: TrackMatrixAxis) -> usize {
    match tm {
        TrackMatrixAxis::Identity => 0,
        TrackMatrixAxis::Rotate90 => 1,
        TrackMatrixAxis::Rotate180 => 2,
        TrackMatrixAxis::Rotate270 => 3,
    }
}

/// Gets the index for a scale value.
pub(super) const fn get_scale_index(sc: ScaleAxis) -> usize {
    match sc {
        ScaleAxis::WithScale => 0,
        ScaleAxis::NoScale => 1,
    }
}

/// Gets the index for a defect value.
#[must_use]
pub(super) const fn get_defect_index(def: Option<&Defect>) -> usize {
    match def {
        None => 0,
        Some(Defect::TruncateAt(_)) => 1,
        Some(Defect::TruncateBox(_)) => 2,
        Some(Defect::ZeroLengthPayload(_)) => 3,
        Some(Defect::OversizedBoxLength(_)) => 4,
        Some(Defect::UndersizedBoxLength(_)) => 5,
        Some(Defect::OffsetPastEnd(_)) => 6,
        Some(Defect::DuplicateBox(_)) => 7,
        Some(Defect::UnknownBox { .. }) => 8,
        Some(Defect::ZeroFrameRate) => 9,
        Some(Defect::ZeroDimension(_)) => 10,
        Some(Defect::AbsurdFrameRate(_)) => 11,
        Some(Defect::NonMonotonicTimestamps) => 12,
        Some(Defect::DeclaredCountMismatch { .. }) => 13,
        Some(Defect::ReorderedBoxes(_, _)) => 14,
    }
}
