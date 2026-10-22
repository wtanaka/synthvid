//! Catalogue entries that tests build from the covering array, and the axis overrides they apply.

use super::*;

/// The axis values a test overrides on an entry, leaving its size and frame count.
pub(crate) struct AxisValues {
    /// Frame rate.
    pub(crate) frame_rate: FrameRateValue,
    /// Background type.
    pub(crate) background: BackgroundAxis,
    /// Shape type.
    pub(crate) shape: ShapeAxis,
    /// Motion type.
    pub(crate) motion: MotionAxis,
    /// Pixel encoding.
    pub(crate) coding: CodingAxis,
    /// Container format.
    pub(crate) container: ContainerAxis,
    /// Display matrix transformation.
    pub(crate) track_matrix: TrackMatrixAxis,
    /// Physical scale declaration.
    pub(crate) scale: ScaleAxis,
    /// Optional defect.
    pub(crate) defect: Option<Defect>,
}

/// The picture size and frame count a test-built entry starts from.
#[derive(Clone, Copy, Debug)]
pub(crate) enum EntrySize {
    /// 16 by 16 pixels, one frame.
    Tiny1,
    /// 16 by 16 pixels, ten frames.
    Tiny10,
    /// 320 by 240 pixels, ten frames.
    Qvga10,
}

/// Builds a catalogue-shaped entry for a test: the dimensions and frame count
/// of the first catalogue entry of the given `size`, then `edit`.
///
/// The covering array chooses which combinations the catalogue holds, so a
/// test that needs one particular combination builds it rather than hoping
/// the catalogue still contains it.
pub(crate) fn entry_like(
    size: EntrySize,
    edit: impl FnOnce(&mut Entry),
) -> Result<Entry, synthvid_testkit::Fail> {
    use synthvid_testkit::OrFail as _;
    let prefix = match size {
        EntrySize::Tiny1 => "0016x0016-1-",
        EntrySize::Tiny10 => "0016x0016-10-",
        EntrySize::Qvga10 => "0320x0240-10-",
    };
    let mut entry = generate_catalogue()
        .or_fail()?
        .into_iter()
        .find(|entry| entry.manifest_name().as_str().starts_with(prefix))
        .or_fail()?;
    edit(&mut entry);
    Ok(entry)
}

/// The AVI entry the truncation digest is pinned on: motion-JPEG frames in
/// an AVI cut just before the `movi` list.
pub(crate) fn avi_truncated_entry() -> Result<Entry, synthvid_testkit::Fail> {
    use synthvid_encode::defects::{BoxAddress, BoxStep, BoxTag, ContainerKind};
    entry_like(EntrySize::Tiny10, |entry| {
        entry.frame_rate = FrameRateValue::Sixty;
        entry.background = BackgroundAxis::Checker;
        entry.shape = ShapeAxis::Rect;
        entry.motion = MotionAxis::Linear;
        entry.coding = CodingAxis::MotionJpeg;
        entry.set_container_axis(ContainerAxis::Avi);
        entry.track_matrix = TrackMatrixAxis::Rotate90;
        entry.scale = ScaleAxis::Undeclared;
        entry.defect = Some(Defect::TruncateBefore(
            BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")))
                .child(BoxStep::first(BoxTag::new(*b"movi"))),
        ));
    })
}
