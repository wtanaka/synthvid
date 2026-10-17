//! Axis definitions and helpers for catalog generation.

use super::cover_greedy::AxisSelection;
use super::cover_values::Compatibility;
use crate::cover::CatalogError;
use core::num::{NonZeroI64, NonZeroU16, NonZeroU32, NonZeroU64};
use synthvid_encode::defects::{
    Axis, BoxAddress, BoxPath, BoxStep, BoxTag, ContainerKind, DeclaredFrameCount, Defect,
};
use synthvid_encode::ContainerFit;
use synthvid_scene::{Dimensions, FrameCount, FrameCountError, FrameIndex, Height, Width};
use synthvid_scene::{FrameRateError, Overflow};

/// The declared defect slots (index 0 is always `None`, meaning "no defect").
///
/// Held on the heap: seventeen slots of a defect each are too large to build
/// on the stack.
pub(super) type DefectSlots = Vec<Option<Defect>>;

/// Frame count axis.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct FrameCountValue(FrameCount);

impl FrameCountValue {
    /// Creates a frame count.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::ZeroFrameCount`] if `count` is zero.
    pub const fn new(count: u32) -> Result<Self, CatalogError> {
        match FrameCount::new(count) {
            Ok(frames) => Ok(Self(frames)),
            Err(FrameCountError::Zero) => Err(CatalogError::ZeroFrameCount),
        }
    }

    /// Gets the frame count value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0 .0.get()
    }

    /// Gets the frame count as the scene's non-zero count.
    #[must_use]
    pub const fn frame_count(self) -> FrameCount {
        self.0
    }
}

/// Frame rate axis.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum FrameRateValue {
    /// 30 frames per second.
    Thirty,
    /// 60 frames per second.
    Sixty,
    /// 120 frames per second.
    OneTwenty,
    /// 240 frames per second.
    TwoForty,
    /// 30000/1001 frames per second (NTSC).
    Ntsc,
    /// An absurdly high frame rate for defect testing.
    Absurd,
}

/// Background axis.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum BackgroundAxis {
    /// Solid color.
    Solid,
    /// Checkerboard pattern.
    Checker,
    /// Linear gradient.
    Gradient,
    /// Random blobs.
    Blobs,
    /// Grid pattern.
    Grid,
}

/// Shape axis.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ShapeAxis {
    /// Disc.
    Disc,
    /// Rectangle.
    Rect,
    /// Polygon.
    Polygon,
    /// Cross.
    Cross,
}

/// Motion axis: different kinds of motion with reasonable parameters.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum MotionAxis {
    /// Fixed position.
    Fixed,
    /// Linear motion.
    Linear,
    /// Ballistic motion.
    Ballistic,
    /// Circular motion.
    Circular,
    /// Oscillating motion.
    Oscillating,
    /// Random walk.
    Walk,
}

/// Coding axis: how pixels are encoded.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum CodingAxis {
    /// Raw pixel data.
    Raw,
    /// Motion JPEG.
    MotionJpeg,
}

/// Container axis: file format.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ContainerAxis {
    /// ISO base media (MP4-like).
    Iso,
    /// AVI container.
    Avi,
}

/// Track display matrix axis: display transformation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum TrackMatrixAxis {
    /// Identity (no rotation).
    Identity,
    /// 90-degree rotation.
    Rotate90,
    /// 180-degree rotation.
    Rotate180,
    /// 270-degree rotation.
    Rotate270,
}

/// Scale axis: whether the scene has a physical scale.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ScaleAxis {
    /// Scene has a physical scale.
    Declared,
    /// Scene has no physical scale.
    Undeclared,
}

/// Holds all axis definitions for the catalog.
pub(super) struct AxisArrays {
    /// Dimension values.
    pub dimensions: [Dimensions; 5],
    /// Frame count values.
    pub frame_counts: [FrameCountValue; 5],
    /// Frame rate values.
    pub frame_rates: [FrameRateValue; 6],
    /// Background values.
    pub backgrounds: [BackgroundAxis; 5],
    /// Shape values.
    pub shapes: [ShapeAxis; 4],
    /// Motion values.
    pub motions: [MotionAxis; 6],
    /// Coding values.
    pub codings: [CodingAxis; 2],
    /// Container values.
    pub containers: [ContainerAxis; 2],
    /// Track matrix values.
    pub track_matrices: [TrackMatrixAxis; 4],
    /// Scale values.
    pub scales: [ScaleAxis; 2],
    /// Defect values.
    pub defects: DefectSlots,
}

/// Raw width/height pair before validation. A `None` side is a zero dimension.
#[derive(Copy, Clone)]
pub(super) struct RawSize {
    /// Width value before validation.
    pub(super) width: Option<NonZeroU16>,
    /// Height value before validation.
    pub(super) height: Option<NonZeroU16>,
}

/// Creates a Dimensions value from width and height u16 values.
/// Returns an error if either dimension is zero.
pub(super) fn make_dimensions(size: RawSize) -> Result<Dimensions, CatalogError> {
    let width = size.width.ok_or(CatalogError::ZeroDimension)?;
    let height = size.height.ok_or(CatalogError::ZeroDimension)?;
    Ok(Dimensions::new(
        Width::from_nonzero(width),
        Height::from_nonzero(height),
    ))
}

/// The container format a defect can only be applied to, or `None` when it
/// applies to either.
///
/// An address is written for one format, and the defects that name boxes by
/// path are written for ISO, so such a defect cannot meet the other format.
#[must_use]
pub(super) const fn defect_container_kind(defect: &Defect) -> Option<ContainerKind> {
    match defect {
        Defect::TruncateBefore(address) => Some(address.kind()),
        Defect::TruncateWithin {
            address,
            offset: _offset,
        } => Some(address.kind()),
        Defect::ZeroLengthPayload(_)
        | Defect::OversizedBoxLength(_)
        | Defect::UndersizedBoxLength(_)
        | Defect::OffsetPastEnd(_)
        | Defect::DuplicateBox(_)
        | Defect::ReorderedBoxes(_, _)
        | Defect::NoContainerSize
        | Defect::NonMonotonicTimestamps => Some(ContainerKind::Iso),
        Defect::UnknownBox {
            after: _after,
            tag: _tag,
        } => None,
        Defect::ZeroFrameRate | Defect::ZeroDimension(_) | Defect::AbsurdFrameRate(_) => None,
        Defect::DeclaredCountMismatch {
            declared: _declared,
        } => None,
    }
}

/// The container format a container axis value produces.
pub(super) const fn container_kind(container: ContainerAxis) -> ContainerKind {
    match container {
        ContainerAxis::Iso => ContainerKind::Iso,
        ContainerAxis::Avi => ContainerKind::Avi,
    }
}

/// Returns whether the container and the defect chosen in `selection` (indices
/// into `axes.containers` / `axes.defects`) can be combined in one entry.
/// Out-of-range indices are treated as compatible (nothing to rule out).
pub(super) fn container_defect_compatible(
    axes: &AxisArrays,
    selection: &AxisSelection,
) -> Compatibility {
    let Some(container) = axes.containers.get(selection.container.0) else {
        return Compatibility::Compatible;
    };
    match axes.defects.get(selection.defect.0) {
        Some(Some(defect)) => {
            if defect_container_kind(defect).is_none_or(|kind| kind == container_kind(*container)) {
                Compatibility::Compatible
            } else {
                Compatibility::Incompatible
            }
        }
        _ => Compatibility::Compatible,
    }
}

/// Returns whether the dimensions/frame-count/coding/container combination in
/// `selection` fits within its container's capacity. Raw (uncompressed)
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
    selection: &AxisSelection,
) -> Compatibility {
    let is_classic_container = axes.containers.get(selection.container.0).is_some();
    let is_raw = axes.codings.get(selection.coding.0) == Some(&CodingAxis::Raw);
    if !is_classic_container || !is_raw {
        return Compatibility::Compatible;
    }
    let Some(dims) = axes.dimensions.get(selection.dimensions.0) else {
        return Compatibility::Compatible;
    };
    let Some(frame_count) = axes.frame_counts.get(selection.frame_count.0) else {
        return Compatibility::Compatible;
    };
    match synthvid_encode::raw_frames_fit_classic_container(*dims, frame_count.frame_count()) {
        ContainerFit::Fits => Compatibility::Compatible,
        ContainerFit::TooLarge => Compatibility::Incompatible,
    }
}

/// The most pixels, summed over all frames, one catalogue entry may hold.
///
/// Pairwise coverage asks for every size beside every frame count, which
/// alone renders a 1920 by 1080 video of 20000 frames: over a hundred
/// gigabytes raw. Nothing about a size or a frame count needs the other to be
/// large, so pairs above this are left out. The cap keeps the largest raw
/// entry near 30 MB.
const MAX_PIXEL_FRAMES: u64 = 10_000_000;

/// Returns whether the dimensions and frame count chosen in `selection`
/// (indices into `axes.dimensions` / `axes.frame_counts`) stay within
/// [`MAX_PIXEL_FRAMES`]. Out-of-range indices are treated as compatible
/// (nothing to rule out).
pub(super) fn volume_compatible(axes: &AxisArrays, selection: &AxisSelection) -> Compatibility {
    let Some(dims) = axes.dimensions.get(selection.dimensions.0) else {
        return Compatibility::Compatible;
    };
    let Some(frame_count) = axes.frame_counts.get(selection.frame_count.0) else {
        return Compatibility::Compatible;
    };
    // 16-bit width times 16-bit height times a 32-bit count is below 2^64, so
    // the product cannot overflow a u64.
    let total: u64 = [
        NonZeroU64::from(dims.width.get()).get(),
        NonZeroU64::from(dims.height.get()).get(),
        u64::from(frame_count.get()),
    ]
    .into_iter()
    .product();
    if total <= MAX_PIXEL_FRAMES {
        Compatibility::Compatible
    } else {
        Compatibility::Incompatible
    }
}

/// Build an `AbsurdFrameRate` defect, propagating any errors.
pub(super) fn build_absurd_frame_rate_defect() -> Result<Defect, CatalogError> {
    // Create a frame rate of 1_000_001 / 1
    let nz_one = NonZeroI64::new(1).ok_or(CatalogError::InvalidFrameRate)?;

    let ratio = synthvid_scene::Ratio::new(synthvid_scene::Numerator(1_000_001), nz_one)
        .map_err(|Overflow| CatalogError::InvalidFrameRate)?;

    let frame_rate = synthvid_scene::FrameRate::new(ratio)
        .map_err(|FrameRateError::NonPositive| CatalogError::InvalidFrameRate)?;

    Ok(Defect::AbsurdFrameRate(frame_rate))
}

/// How far into a box's own bytes a `TruncateWithin` cuts.
///
/// Every catalogue frame is at least 16 by 16 pixels, so the boxes cut are
/// longer than this.
fn truncation_offset() -> Result<NonZeroU32, CatalogError> {
    NonZeroU32::new(100).ok_or(CatalogError::ZeroTruncationOffset)
}

/// The address of an ISO box: a top-level box and the boxes below it, naming
/// the first sibling carrying each tag.
fn iso_address(top: BoxTag, below: &[BoxTag]) -> BoxAddress {
    descend(
        BoxAddress::top(ContainerKind::Iso, BoxStep::first(top)),
        below,
    )
}

/// The address of an AVI chunk below the `RIFF` root.
fn avi_address(below: &[BoxTag]) -> BoxAddress {
    descend(
        BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF"))),
        below,
    )
}

/// The address of the first frame in an AVI file, whichever codec wrote it.
fn avi_first_frame_address() -> BoxAddress {
    avi_address(&[BoxTag::new(*b"movi")]).child(BoxStep::frame(FrameIndex::new(0)))
}

/// Extends an address by one step per tag, each naming the first sibling.
fn descend(address: BoxAddress, below: &[BoxTag]) -> BoxAddress {
    below
        .iter()
        .fold(address, |so_far, tag| so_far.child(BoxStep::first(*tag)))
}

/// All possible defect instances for the catalog.
pub(super) fn all_defects() -> Result<DefectSlots, CatalogError> {
    Ok(vec![
        None,
        Some(Defect::TruncateBefore(iso_address(
            BoxTag::new(*b"moov"),
            &[],
        ))),
        Some(Defect::TruncateWithin {
            address: iso_address(BoxTag::new(*b"mdat"), &[])
                .child(BoxStep::frame(FrameIndex::new(0))),
            offset: truncation_offset()?,
        }),
        Some(Defect::TruncateBefore(avi_address(&[BoxTag::new(
            *b"movi",
        )]))),
        Some(Defect::TruncateWithin {
            address: avi_first_frame_address(),
            offset: truncation_offset()?,
        }),
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
            [*b"moov", *b"trak", *b"mdia", *b"minf", *b"stbl", *b"stco"]
                .into_iter()
                .fold(BoxPath::root(), |path, tag| path.child(BoxTag::new(tag))),
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
        Some(Defect::DeclaredCountMismatch {
            declared: DeclaredFrameCount(42),
        }),
        Some(Defect::ReorderedBoxes(
            BoxPath::root().child(BoxTag::new(*b"moov")),
            BoxPath::root().child(BoxTag::new(*b"mdat")),
        )),
    ])
}
