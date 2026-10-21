#[cfg(test)]
use super::*;
use crate::{encode_avi, encode_iso, AviCodec, EncodedFrame, IsoCodec};
use synthvid_scene::{Dimensions, Frame, FrameRate, Height, PixelBuffer, Width};
use synthvid_testkit::{ensure, ensure_eq, ensure_ne, Fail, OrFail as _};

/// An encoded container file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Media(pub(super) Vec<u8>);

impl Media {
    /// These bytes as a container file, for [`apply`].
    pub(super) fn file(&self) -> ContainerFile {
        ContainerFile(self.0.clone())
    }

    /// The bytes of `file`.
    pub(super) fn of(file: ContainerFile) -> Self {
        Self(file.0)
    }
}

/// The container a test file is written in.
#[derive(Clone, Copy, Debug)]
pub(super) enum Base {
    /// A one-frame ISO file.
    Iso,
    /// A one-frame AVI file.
    Avi,
}

impl Base {
    /// Encodes the one-frame file of this kind.
    pub(super) fn media(self) -> Result<Media, Fail> {
        match self {
            Self::Iso => make_iso_base(),
            Self::Avi => make_avi_base(),
        }
    }
}

/// Creates a tiny 1×1 frame for testing.
pub(super) fn make_test_frame() -> Result<Frame, Fail> {
    let width = Width::new(1).or_fail()?;
    let height = Height::new(1).or_fail()?;
    let dims = Dimensions::new(width, height);
    let data = vec![0_u8; 3];
    Frame::new(dims, PixelBuffer(data)).or_fail()
}

/// Creates a base ISO file for testing.
fn make_iso_base() -> Result<Media, Fail> {
    let width = Width::new(1).or_fail()?;
    let height = Height::new(1).or_fail()?;
    let frame = make_test_frame()?;
    encode_iso(
        crate::FrameRates {
            playback: FrameRate::from_fps(24).or_fail()?,
            capture: None,
        },
        &[EncodedFrame::raw(&frame)],
        crate::TrackParams {
            width,
            height,
            codec: IsoCodec::UncompressedRgb,
            track_matrix: crate::TrackMatrix::identity(),
        },
    )
    .map(Media::of)
    .or_fail()
}

/// Creates a base AVI file for testing.
fn make_avi_base() -> Result<Media, Fail> {
    let width = Width::new(1).or_fail()?;
    let height = Height::new(1).or_fail()?;
    let frame = make_test_frame()?;
    encode_avi(
        FrameRate::from_fps(24).or_fail()?,
        &[EncodedFrame::raw(&frame)],
        width,
        height,
        AviCodec::UncompressedRgb,
    )
    .map(Media::of)
    .or_fail()
}

/// Applies `defect` to `input`.
pub(super) fn applied(defect: &Defect, input: &Media) -> Result<Media, Fail> {
    apply(defect, &input.file()).map(Media::of).or_fail()
}

/// The path of a box directly below the root.
fn top(tag: BoxTag) -> BoxPath {
    BoxPath::root().child(tag)
}

/// Fails unless `defect` changes the bytes of the `base` file.
fn changes_bytes(base: Base, defect: &Defect) -> Result<(), Fail> {
    let input = base.media()?;
    let result = applied(defect, &input)?;
    ensure_ne!(result, input, "defect should change bytes");
    Ok(())
}

/// Fails unless `defect` changes the bytes of the `base` file but not its length.
fn changes_bytes_in_place(base: Base, defect: &Defect) -> Result<(), Fail> {
    let input = base.media()?;
    let result = applied(defect, &input)?;
    ensure_ne!(result, input, "defect should change bytes");
    ensure_eq!(
        result.0.len(),
        input.0.len(),
        "defect should not change file size"
    );
    Ok(())
}

/// Fails unless `defect` changes the bytes of the `base` file and lengthens it.
fn grows(base: Base, defect: &Defect) -> Result<(), Fail> {
    let input = base.media()?;
    let result = applied(defect, &input)?;
    ensure_ne!(result, input, "defect should change bytes");
    ensure!(result.0.len() > input.0.len(), "file should grow");
    Ok(())
}

/// Fails unless applying `defect` twice to the `base` file gives the same bytes.
fn reproducible(base: Base, defect: &Defect) -> Result<(), Fail> {
    let input = base.media()?;
    let result1 = applied(defect, &input)?;
    let result2 = applied(defect, &input)?;
    ensure_eq!(result1, result2, "defect must be reproducible");
    Ok(())
}

/// Applies `defect` to 100 zero bytes, which are no container at all.
fn on_garbage(defect: &Defect) -> Result<Media, DefectError> {
    apply(defect, &ContainerFile(vec![0_u8; 100])).map(Media::of)
}

#[test]
fn boxpath_builder() {
    let path = BoxPath::root()
        .child(BoxTag::new(*b"moov"))
        .child(BoxTag::new(*b"trak"))
        .child(BoxTag::new(*b"tkhd"));
    assert_eq!(path.tags().len(), 3);
    assert_eq!(path.tags().first(), Some(&BoxTag::new(*b"moov")));
}

#[test]
fn axis_enum() {
    let width = Axis::Width;
    let height = Axis::Height;
    assert_ne!(width, height);
}

#[test]
fn zero_payload_iso_changes_bytes() {
    let defect = Defect::ZeroLengthPayload(top(BoxTag::new(*b"ftyp")));
    assert_eq!(changes_bytes_in_place(Base::Iso, &defect), Ok(()));
}

#[test]
fn zero_payload_iso_reproducible() {
    let defect = Defect::ZeroLengthPayload(top(BoxTag::new(*b"ftyp")));
    assert_eq!(reproducible(Base::Iso, &defect), Ok(()));
}

#[test]
fn zero_payload_iso_missing_box_errors() {
    let defect = Defect::ZeroLengthPayload(top(BoxTag::new(*b"ftyp")));
    assert_eq!(on_garbage(&defect), Err(DefectError::IncompatibleFileType));
}

#[test]
fn oversized_iso_changes_bytes() {
    let defect = Defect::OversizedBoxLength(top(BoxTag::new(*b"ftyp")));
    assert_eq!(changes_bytes(Base::Iso, &defect), Ok(()));
}

#[test]
fn oversized_iso_reproducible() {
    let defect = Defect::OversizedBoxLength(top(BoxTag::new(*b"ftyp")));
    assert_eq!(reproducible(Base::Iso, &defect), Ok(()));
}

#[test]
fn oversized_iso_missing_box_errors() {
    let defect = Defect::OversizedBoxLength(top(BoxTag::new(*b"ftyp")));
    assert_eq!(on_garbage(&defect), Err(DefectError::IncompatibleFileType));
}

#[test]
fn undersized_iso_changes_bytes() {
    let defect = Defect::UndersizedBoxLength(top(BoxTag::new(*b"ftyp")));
    assert_eq!(changes_bytes(Base::Iso, &defect), Ok(()));
}

#[test]
fn undersized_iso_reproducible() {
    let defect = Defect::UndersizedBoxLength(top(BoxTag::new(*b"ftyp")));
    assert_eq!(reproducible(Base::Iso, &defect), Ok(()));
}

#[test]
fn undersized_iso_missing_box_errors() {
    let defect = Defect::UndersizedBoxLength(top(BoxTag::new(*b"ftyp")));
    assert_eq!(on_garbage(&defect), Err(DefectError::IncompatibleFileType));
}

#[test]
fn offset_past_end_iso_changes_bytes() {
    let defect = Defect::OffsetPastEnd(top(BoxTag::new(*b"moov")));
    assert_eq!(changes_bytes(Base::Iso, &defect), Ok(()));
}

#[test]
fn offset_past_end_iso_reproducible() {
    let defect = Defect::OffsetPastEnd(top(BoxTag::new(*b"moov")));
    assert_eq!(reproducible(Base::Iso, &defect), Ok(()));
}

#[test]
fn offset_past_end_iso_missing_box_errors() {
    let defect = Defect::OffsetPastEnd(top(BoxTag::new(*b"moov")));
    assert_eq!(on_garbage(&defect), Err(DefectError::IncompatibleFileType));
}

#[test]
fn duplicate_box_iso_changes_bytes() {
    let defect = Defect::DuplicateBox(top(BoxTag::new(*b"ftyp")));
    assert_eq!(grows(Base::Iso, &defect), Ok(()));
}

#[test]
fn duplicate_box_iso_reproducible() {
    let defect = Defect::DuplicateBox(top(BoxTag::new(*b"ftyp")));
    assert_eq!(reproducible(Base::Iso, &defect), Ok(()));
}

#[test]
fn duplicate_box_iso_missing_box_errors() {
    let defect = Defect::DuplicateBox(top(BoxTag::new(*b"ftyp")));
    assert_eq!(on_garbage(&defect), Err(DefectError::IncompatibleFileType));
}

#[test]
fn unknown_box_iso_changes_bytes() {
    let defect = Defect::UnknownBox {
        after: top(BoxTag::new(*b"ftyp")),
        tag: BoxTag::new(*b"xxxx"),
    };
    assert_eq!(grows(Base::Iso, &defect), Ok(()));
}

#[test]
fn unknown_box_iso_reproducible() {
    let defect = Defect::UnknownBox {
        after: top(BoxTag::new(*b"ftyp")),
        tag: BoxTag::new(*b"xxxx"),
    };
    assert_eq!(reproducible(Base::Iso, &defect), Ok(()));
}

#[test]
fn unknown_box_iso_missing_box_errors() {
    let defect = Defect::UnknownBox {
        after: top(BoxTag::new(*b"ftyp")),
        tag: BoxTag::new(*b"xxxx"),
    };
    assert_eq!(on_garbage(&defect), Err(DefectError::IncompatibleFileType));
}

#[test]
fn zero_frame_rate_iso_changes_bytes() {
    let defect = Defect::ZeroFrameRate;
    assert_eq!(changes_bytes(Base::Iso, &defect), Ok(()));
}

#[test]
fn zero_frame_rate_iso_reproducible() {
    let defect = Defect::ZeroFrameRate;
    assert_eq!(reproducible(Base::Iso, &defect), Ok(()));
}

#[test]
fn zero_frame_rate_iso_missing_box_errors() {
    let defect = Defect::ZeroFrameRate;
    assert_eq!(on_garbage(&defect), Err(DefectError::IncompatibleFileType));
}

#[test]
fn zero_dimension_width_iso_changes_bytes() {
    let defect = Defect::ZeroDimension(Axis::Width);
    assert_eq!(changes_bytes(Base::Iso, &defect), Ok(()));
}

#[test]
fn zero_dimension_height_iso_changes_bytes() {
    let defect = Defect::ZeroDimension(Axis::Height);
    assert_eq!(changes_bytes(Base::Iso, &defect), Ok(()));
}

#[test]
fn zero_dimension_iso_reproducible() {
    let defect = Defect::ZeroDimension(Axis::Width);
    assert_eq!(reproducible(Base::Iso, &defect), Ok(()));
}

#[test]
fn zero_dimension_iso_missing_box_errors() {
    let defect = Defect::ZeroDimension(Axis::Width);
    assert_eq!(on_garbage(&defect), Err(DefectError::IncompatibleFileType));
}

#[test]
fn absurd_frame_rate_iso_changes_bytes() {
    assert_eq!(check_absurd_frame_rate_iso_changes_bytes(), Ok(()));
}

fn check_absurd_frame_rate_iso_changes_bytes() -> Result<(), Fail> {
    let defect = Defect::AbsurdFrameRate(FrameRate::from_fps(1_000_000).or_fail()?);
    changes_bytes(Base::Iso, &defect)
}

#[test]
fn absurd_frame_rate_iso_reproducible() {
    assert_eq!(check_absurd_frame_rate_iso_reproducible(), Ok(()));
}

fn check_absurd_frame_rate_iso_reproducible() -> Result<(), Fail> {
    let defect = Defect::AbsurdFrameRate(FrameRate::from_fps(1_000_000).or_fail()?);
    reproducible(Base::Iso, &defect)
}

#[test]
fn absurd_frame_rate_iso_missing_box_errors() {
    assert_eq!(check_absurd_frame_rate_iso_missing_box_errors(), Ok(()));
}

fn check_absurd_frame_rate_iso_missing_box_errors() -> Result<(), Fail> {
    let defect = Defect::AbsurdFrameRate(FrameRate::from_fps(1_000_000).or_fail()?);
    ensure_eq!(on_garbage(&defect), Err(DefectError::IncompatibleFileType));
    Ok(())
}

#[test]
fn non_monotonic_timestamps_iso_changes_bytes() {
    let defect = Defect::NonMonotonicTimestamps;
    assert_eq!(changes_bytes(Base::Iso, &defect), Ok(()));
}

#[test]
fn non_monotonic_timestamps_iso_reproducible() {
    let defect = Defect::NonMonotonicTimestamps;
    assert_eq!(reproducible(Base::Iso, &defect), Ok(()));
}

#[test]
fn non_monotonic_timestamps_iso_missing_box_errors() {
    let defect = Defect::NonMonotonicTimestamps;
    assert_eq!(on_garbage(&defect), Err(DefectError::IncompatibleFileType));
}

#[test]
fn declared_count_mismatch_iso_changes_bytes() {
    let defect = Defect::DeclaredCountMismatch {
        declared: DeclaredFrameCount(999),
    };
    assert_eq!(changes_bytes(Base::Iso, &defect), Ok(()));
}

#[test]
fn declared_count_mismatch_iso_reproducible() {
    let defect = Defect::DeclaredCountMismatch {
        declared: DeclaredFrameCount(999),
    };
    assert_eq!(reproducible(Base::Iso, &defect), Ok(()));
}

#[test]
fn declared_count_mismatch_iso_missing_box_errors() {
    let defect = Defect::DeclaredCountMismatch {
        declared: DeclaredFrameCount(999),
    };
    assert_eq!(on_garbage(&defect), Err(DefectError::IncompatibleFileType));
}

#[test]
fn reordered_boxes_iso_changes_bytes() {
    let defect = Defect::ReorderedBoxes(top(BoxTag::new(*b"ftyp")), top(BoxTag::new(*b"mdat")));
    assert_eq!(changes_bytes(Base::Iso, &defect), Ok(()));
}

#[test]
fn reordered_boxes_iso_reproducible() {
    let defect = Defect::ReorderedBoxes(top(BoxTag::new(*b"ftyp")), top(BoxTag::new(*b"mdat")));
    assert_eq!(reproducible(Base::Iso, &defect), Ok(()));
}

#[test]
fn reordered_boxes_iso_missing_box_errors() {
    let defect = Defect::ReorderedBoxes(top(BoxTag::new(*b"ftyp")), top(BoxTag::new(*b"mdat")));
    assert_eq!(on_garbage(&defect), Err(DefectError::IncompatibleFileType));
}

#[test]
fn zero_dimension_width_iso_exact_value() {
    let defect = Defect::ZeroDimension(Axis::Width);
    assert_eq!(changes_bytes_in_place(Base::Iso, &defect), Ok(()));
}

#[test]
fn zero_dimension_height_iso_exact_value() {
    let defect = Defect::ZeroDimension(Axis::Height);
    assert_eq!(changes_bytes_in_place(Base::Iso, &defect), Ok(()));
}

#[test]
fn offset_past_end_iso_exact_value() {
    let defect = Defect::OffsetPastEnd(
        BoxPath::root()
            .child(BoxTag::new(*b"moov"))
            .child(BoxTag::new(*b"trak"))
            .child(BoxTag::new(*b"mdia"))
            .child(BoxTag::new(*b"minf"))
            .child(BoxTag::new(*b"stbl"))
            .child(BoxTag::new(*b"stco")),
    );
    assert_eq!(changes_bytes_in_place(Base::Iso, &defect), Ok(()));
}

#[test]
fn declared_count_mismatch_iso_stsz_field() {
    let defect = Defect::DeclaredCountMismatch {
        declared: DeclaredFrameCount(42),
    };
    assert_eq!(changes_bytes_in_place(Base::Iso, &defect), Ok(()));
}

#[test]
fn non_monotonic_timestamps_iso_sample_delta() {
    let defect = Defect::NonMonotonicTimestamps;
    assert_eq!(changes_bytes_in_place(Base::Iso, &defect), Ok(()));
}

#[test]
fn zero_frame_rate_avi_correct_offset() {
    let defect = Defect::ZeroFrameRate;
    assert_eq!(changes_bytes_in_place(Base::Avi, &defect), Ok(()));
}

#[test]
fn zero_frame_rate_avi_reproducible() {
    let defect = Defect::ZeroFrameRate;
    assert_eq!(reproducible(Base::Avi, &defect), Ok(()));
}

#[test]
fn absurd_frame_rate_avi_correct_offset() {
    assert_eq!(check_absurd_frame_rate_avi_correct_offset(), Ok(()));
}

fn check_absurd_frame_rate_avi_correct_offset() -> Result<(), Fail> {
    let defect = Defect::AbsurdFrameRate(FrameRate::from_fps(2000).or_fail()?);
    changes_bytes_in_place(Base::Avi, &defect)
}

#[test]
fn absurd_frame_rate_avi_reproducible() {
    assert_eq!(check_absurd_frame_rate_avi_reproducible(), Ok(()));
}

fn check_absurd_frame_rate_avi_reproducible() -> Result<(), Fail> {
    let defect = Defect::AbsurdFrameRate(FrameRate::from_fps(2000).or_fail()?);
    reproducible(Base::Avi, &defect)
}

#[test]
fn zero_dimension_width_avi_correct_offset() {
    let defect = Defect::ZeroDimension(Axis::Width);
    assert_eq!(changes_bytes_in_place(Base::Avi, &defect), Ok(()));
}

#[test]
fn zero_dimension_height_avi_correct_offset() {
    let defect = Defect::ZeroDimension(Axis::Height);
    assert_eq!(changes_bytes_in_place(Base::Avi, &defect), Ok(()));
}

#[test]
fn zero_dimension_avi_reproducible() {
    let defect = Defect::ZeroDimension(Axis::Width);
    assert_eq!(reproducible(Base::Avi, &defect), Ok(()));
}

#[test]
fn declared_count_mismatch_avi_correct_offset() {
    let defect = Defect::DeclaredCountMismatch {
        declared: DeclaredFrameCount(99),
    };
    assert_eq!(changes_bytes_in_place(Base::Avi, &defect), Ok(()));
}

#[test]
fn declared_count_mismatch_avi_reproducible() {
    let defect = Defect::DeclaredCountMismatch {
        declared: DeclaredFrameCount(99),
    };
    assert_eq!(reproducible(Base::Avi, &defect), Ok(()));
}
