//! Tests for the truncation defects.

use super::tests::{applied, make_test_frame, Media};
#[cfg(test)]
use super::*;
use crate::{encode_avi, encode_iso, AviCodec, EncodedFrame, IsoCodec};
use synthvid_scene::{FrameIndex, FrameRate, Height, Width};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// The length of a file a truncation produced, in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CutLength(usize);

/// Lengths of truncated files, in ascending order.
#[derive(Debug, PartialEq, Eq)]
struct CutLengths(Vec<CutLength>);

/// Creates an ISO file whose two frames of odd length give the layout
/// something awkward: same-tag siblings and an unaligned payload.
fn make_iso_two_frames() -> Result<Media, Fail> {
    let width = Width::new(1).or_fail()?;
    let height = Height::new(1).or_fail()?;
    let frame = EncodedFrame::raw(&make_test_frame()?);
    encode_iso(
        crate::FrameRates {
            playback: FrameRate::from_fps(24).or_fail()?,
            capture: None,
        },
        &[frame.clone(), frame],
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

/// Creates an AVI file with two 3-byte frames: two same-tag `00db` chunks,
/// each followed by a pad byte.
fn make_avi_two_frames() -> Result<Media, Fail> {
    let width = Width::new(1).or_fail()?;
    let height = Height::new(1).or_fail()?;
    let frame = EncodedFrame::raw(&make_test_frame()?);
    encode_avi(
        FrameRate::from_fps(24).or_fail()?,
        &[frame.clone(), frame],
        width,
        height,
        AviCodec::UncompressedRgb,
    )
    .map(Media::of)
    .or_fail()
}

/// The length of every file a truncation can produce, spelling each box's
/// start with `TruncateBefore` and every position inside it with
/// `TruncateWithin`.
fn every_cut_length(input: &Media) -> Result<CutLengths, Fail> {
    let layout = defects_layout::Layout::of(ContainerBytes::new(&input.0)).or_fail()?;
    let mut lengths = Vec::new();
    for region in layout.regions() {
        let address = region.address.clone();
        let (start, end) = layout.span(&address).or_fail()?;
        let before = applied(&Defect::TruncateBefore(address.clone()), input)?;
        lengths.push(CutLength(before.0.len()));
        for byte_count in 1..end.get().checked_sub(start.get()).or_fail()? {
            let offset = u32::try_from(byte_count).or_fail()?;
            let within = applied(
                &Defect::TruncateWithin {
                    address: address.clone(),
                    offset: NonZeroU32::new(offset).or_fail()?,
                },
                input,
            )?;
            lengths.push(CutLength(within.0.len()));
        }
    }
    lengths.sort_unstable();
    Ok(CutLengths(lengths))
}

/// The length of every proper prefix of `input`, shortest first.
fn every_prefix_length(input: &Media) -> CutLengths {
    CutLengths((0..input.0.len()).map(CutLength).collect())
}

#[test]
fn truncations_partition_iso() {
    assert_eq!(check_truncations_partition_iso(), Ok(()));
}

fn check_truncations_partition_iso() -> Result<(), Fail> {
    let input = make_iso_two_frames()?;
    // Every cut position has exactly one spelling.
    ensure_eq!(every_cut_length(&input)?, every_prefix_length(&input));
    Ok(())
}

#[test]
fn truncations_partition_avi() {
    assert_eq!(check_truncations_partition_avi(), Ok(()));
}

fn check_truncations_partition_avi() -> Result<(), Fail> {
    let input = make_avi_two_frames()?;
    ensure_eq!(every_cut_length(&input)?, every_prefix_length(&input));
    Ok(())
}

#[test]
fn truncations_partition_avi_with_every_padding_amount() {
    assert_eq!(
        check_truncations_partition_avi_with_every_padding_amount(),
        Ok(())
    );
}

fn check_truncations_partition_avi_with_every_padding_amount() -> Result<(), Fail> {
    let width = Width::new(1).or_fail()?;
    let height = Height::new(1).or_fail()?;
    // Chunk lengths of 1, 2 and 5 bytes take 1, 0 and 1 pad bytes, where
    // padding to a 4-byte boundary would add 3, 2 and 3. Only a walker that
    // pads as RIFF does stays aligned.
    let input = Media::of(
        encode_avi(
            FrameRate::from_fps(24).or_fail()?,
            &[
                EncodedFrame(vec![1]),
                EncodedFrame(vec![1, 2]),
                EncodedFrame(vec![1, 2, 3, 4, 5]),
            ],
            width,
            height,
            AviCodec::MotionJpeg,
        )
        .or_fail()?,
    );
    ensure_eq!(every_cut_length(&input)?, every_prefix_length(&input));
    Ok(())
}

#[test]
fn truncations_keep_a_prefix() {
    assert_eq!(check_truncations_keep_a_prefix(), Ok(()));
}

fn check_truncations_keep_a_prefix() -> Result<(), Fail> {
    let input = make_avi_two_frames()?;
    let riff = BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")));
    let cut = applied(
        &Defect::TruncateWithin {
            address: riff,
            offset: NonZeroU32::new(5).or_fail()?,
        },
        &input,
    )?;
    ensure_eq!(Some(cut.0.as_slice()), input.0.get(..5));
    Ok(())
}

#[test]
fn truncate_before_first_box_is_empty() {
    assert_eq!(check_truncate_before_first_box_is_empty(), Ok(()));
}

fn check_truncate_before_first_box_is_empty() -> Result<(), Fail> {
    let input = make_iso_two_frames()?;
    let ftyp = BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"ftyp")));
    let cut = applied(&Defect::TruncateBefore(ftyp), &input)?;
    ensure!(cut.0.is_empty());
    Ok(())
}

#[test]
fn truncate_before_names_the_second_of_two_siblings() {
    assert_eq!(
        check_truncate_before_names_the_second_of_two_siblings(),
        Ok(())
    );
}

fn check_truncate_before_names_the_second_of_two_siblings() -> Result<(), Fail> {
    let input = make_avi_two_frames()?;
    let first = applied(
        &Defect::TruncateBefore(movi_frame(FrameIndex::new(0))),
        &input,
    )?;
    let second = applied(
        &Defect::TruncateBefore(movi_frame(FrameIndex::new(1))),
        &input,
    )?;
    // The first frame is an 8-byte header, 3 bytes of pixels and a pad byte.
    ensure_eq!(second.0.len().checked_sub(first.0.len()).or_fail()?, 12);
    Ok(())
}

/// Creates an MJPEG AVI whose first frame is 9 bytes (1 mod 4) and is
/// followed by a second, 13-byte frame.
fn make_avi_odd_mjpeg() -> Result<Media, Fail> {
    let width = Width::new(16).or_fail()?;
    let height = Height::new(16).or_fail()?;
    encode_avi(
        FrameRate::from_fps(24).or_fail()?,
        &[EncodedFrame(vec![0xA5; 9]), EncodedFrame(vec![0x5A; 13])],
        width,
        height,
        AviCodec::MotionJpeg,
    )
    .map(Media::of)
    .or_fail()
}

#[test]
fn truncate_before_second_frame_after_odd_mjpeg_frame() {
    assert_eq!(
        check_truncate_before_second_frame_after_odd_mjpeg_frame(),
        Ok(())
    );
}

fn check_truncate_before_second_frame_after_odd_mjpeg_frame() -> Result<(), Fail> {
    let input = make_avi_odd_mjpeg()?;
    let first = applied(
        &Defect::TruncateBefore(movi_frame(FrameIndex::new(0))),
        &input,
    )?;
    let second = applied(
        &Defect::TruncateBefore(movi_frame(FrameIndex::new(1))),
        &input,
    )?;
    // An 8-byte header, 9 bytes of JPEG and exactly one pad byte.
    ensure_eq!(second.0.len().checked_sub(first.0.len()).or_fail()?, 18);
    // A frame's region starts at its data, so its chunk header ends there.
    let header = second.0.len().checked_sub(8).or_fail()?;
    let size_field = second.0.len().checked_sub(4).or_fail()?;
    ensure_eq!(input.0.get(header..size_field), Some(b"00dc".as_slice()));
    ensure_eq!(every_cut_length(&input)?, every_prefix_length(&input));
    Ok(())
}

/// The address of frame `index` under `movi`.
fn movi_frame(index: FrameIndex) -> BoxAddress {
    BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")))
        .child(BoxStep::first(BoxTag::new(*b"movi")))
        .child(BoxStep::frame(index))
}

/// The address of frame `index` under `mdat`.
fn mdat_frame(index: FrameIndex) -> BoxAddress {
    BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"mdat")))
        .child(BoxStep::frame(index))
}

#[test]
fn a_frame_address_holds_for_every_codec() {
    assert_eq!(check_a_frame_address_holds_for_every_codec(), Ok(()));
}

fn check_a_frame_address_holds_for_every_codec() -> Result<(), Fail> {
    for input in [make_avi_two_frames()?, make_avi_odd_mjpeg()?] {
        let layout = defects_layout::Layout::of(ContainerBytes::new(&input.0)).or_fail()?;
        ensure!(layout.span(&movi_frame(FrameIndex::new(0))).is_ok());
        ensure!(layout.span(&movi_frame(FrameIndex::new(1))).is_ok());
        ensure_eq!(
            layout.span(&movi_frame(FrameIndex::new(2))),
            Err(DefectError::AddressNotFound)
        );
    }
    Ok(())
}

#[test]
fn an_offset_into_a_frame_counts_the_same_bytes_in_both_formats() {
    assert_eq!(
        check_an_offset_into_a_frame_counts_the_same_bytes_in_both_formats(),
        Ok(())
    );
}

fn check_an_offset_into_a_frame_counts_the_same_bytes_in_both_formats() -> Result<(), Fail> {
    for (input, frame) in [
        (make_iso_two_frames()?, mdat_frame(FrameIndex::new(0))),
        (make_avi_two_frames()?, movi_frame(FrameIndex::new(0))),
    ] {
        let before = applied(&Defect::TruncateBefore(frame.clone()), &input)?;
        let within = applied(
            &Defect::TruncateWithin {
                address: frame,
                offset: NonZeroU32::new(2).or_fail()?,
            },
            &input,
        )?;
        ensure_eq!(within.0.len().checked_sub(before.0.len()).or_fail()?, 2);
        ensure_eq!(input.0.get(..within.0.len()), Some(within.0.as_slice()));
    }
    Ok(())
}

#[test]
fn a_frame_has_no_tagged_address() {
    assert_eq!(check_a_frame_has_no_tagged_address(), Ok(()));
}

fn check_a_frame_has_no_tagged_address() -> Result<(), Fail> {
    let input = make_avi_two_frames()?;
    let layout = defects_layout::Layout::of(ContainerBytes::new(&input.0)).or_fail()?;
    let tagged = BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")))
        .child(BoxStep::first(BoxTag::new(*b"movi")))
        .child(BoxStep::first(BoxTag::new(*b"00db")));
    ensure_eq!(layout.span(&tagged), Err(DefectError::AddressNotFound));
    Ok(())
}

#[test]
fn iso_layout_reaches_sample_table_boxes() {
    assert_eq!(check_iso_layout_reaches_sample_table_boxes(), Ok(()));
}

fn check_iso_layout_reaches_sample_table_boxes() -> Result<(), Fail> {
    let input = make_iso_two_frames()?;
    let layout = defects_layout::Layout::of(ContainerBytes::new(&input.0)).or_fail()?;
    let stco = BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"moov")))
        .child(BoxStep::first(BoxTag::new(*b"trak")))
        .child(BoxStep::first(BoxTag::new(*b"mdia")))
        .child(BoxStep::first(BoxTag::new(*b"minf")))
        .child(BoxStep::first(BoxTag::new(*b"stbl")))
        .child(BoxStep::first(BoxTag::new(*b"stco")));
    ensure!(layout.span(&stco).is_ok());
    Ok(())
}

#[test]
fn iso_lookup_does_not_descend_into_a_leaf_box() {
    assert_eq!(check_iso_lookup_does_not_descend_into_a_leaf_box(), Ok(()));
}

fn check_iso_lookup_does_not_descend_into_a_leaf_box() -> Result<(), Fail> {
    let input = make_iso_two_frames()?;
    let path = BoxPath::root()
        .child(BoxTag::new(*b"mdat"))
        .child(BoxTag::new(*b"stco"));
    ensure_eq!(
        defects_box::find_iso_box(ContainerBytes::new(&input.0), &path).map(|_| ()),
        Err(DefectError::BoxNotFound(path))
    );
    Ok(())
}

#[test]
fn find_avi_box_finds_a_frame_shorter_than_a_chunk_header() {
    assert_eq!(
        check_find_avi_box_finds_a_frame_shorter_than_a_chunk_header(),
        Ok(())
    );
}

fn check_find_avi_box_finds_a_frame_shorter_than_a_chunk_header() -> Result<(), Fail> {
    let input = make_avi_two_frames()?;
    let path = BoxPath::root()
        .child(BoxTag::new(*b"movi"))
        .child(BoxTag::new(*b"00db"));
    let frame = defects_box::find_avi_box(ContainerBytes::new(&input.0), &path).or_fail()?;
    ensure_eq!(
        frame
            .payload_end
            .get()
            .checked_sub(frame.payload_start.get())
            .or_fail()?,
        3
    );
    Ok(())
}

#[test]
fn iso_frames_are_addressed_by_number_below_mdat() {
    assert_eq!(
        check_iso_frames_are_addressed_by_number_below_mdat(),
        Ok(())
    );
}

fn check_iso_frames_are_addressed_by_number_below_mdat() -> Result<(), Fail> {
    let input = make_iso_two_frames()?;
    let layout = defects_layout::Layout::of(ContainerBytes::new(&input.0)).or_fail()?;
    let (first_start, first_end) = layout.span(&mdat_frame(FrameIndex::new(0))).or_fail()?;
    let (second_start, second_end) = layout.span(&mdat_frame(FrameIndex::new(1))).or_fail()?;
    ensure_eq!(first_end, second_start);
    ensure_eq!(second_end.get(), input.0.len());
    ensure_eq!(
        first_end.get().checked_sub(first_start.get()).or_fail()?,
        second_end.get().checked_sub(second_start.get()).or_fail()?
    );
    ensure_eq!(
        layout.span(&mdat_frame(FrameIndex::new(2))),
        Err(DefectError::AddressNotFound)
    );
    Ok(())
}

#[test]
fn find_avi_box_walks_past_odd_mjpeg_frame() {
    assert_eq!(check_find_avi_box_walks_past_odd_mjpeg_frame(), Ok(()));
}

fn check_find_avi_box_walks_past_odd_mjpeg_frame() -> Result<(), Fail> {
    let input = make_avi_odd_mjpeg()?;
    let movi = BoxPath::root().child(BoxTag::new(*b"movi"));
    let first = defects_box::find_avi_box(
        ContainerBytes::new(&input.0),
        &movi.clone().child(BoxTag::new(*b"00dc")),
    )
    .or_fail()?;
    ensure_eq!(
        first
            .payload_end
            .get()
            .checked_sub(first.payload_start.get())
            .or_fail()?,
        9
    );
    // Looking for a tag that is absent steps over both frames, then idx1;
    // a walker that misplaced the second frame would read garbage instead.
    let absent = movi.child(BoxTag::new(*b"JUNK"));
    ensure_eq!(
        defects_box::find_avi_box(ContainerBytes::new(&input.0), &absent).map(|_| ()),
        Err(DefectError::BoxNotFound(absent))
    );
    Ok(())
}

#[test]
fn truncate_within_past_owned_bytes_errors() {
    assert_eq!(check_truncate_within_past_owned_bytes_errors(), Ok(()));
}

fn check_truncate_within_past_owned_bytes_errors() -> Result<(), Fail> {
    let input = make_iso_two_frames()?;
    let ftyp = BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"ftyp")));
    let layout = defects_layout::Layout::of(ContainerBytes::new(&input.0)).or_fail()?;
    let (start, end) = layout.span(&ftyp).or_fail()?;
    let owned = u32::try_from(end.get().checked_sub(start.get()).or_fail()?).or_fail()?;
    // An offset at the next box's start is TruncateBefore, not this.
    ensure_eq!(
        apply(
            &Defect::TruncateWithin {
                address: ftyp,
                offset: NonZeroU32::new(owned).or_fail()?,
            },
            &input.file(),
        ),
        Err(DefectError::OffsetOutOfRange)
    );
    Ok(())
}

#[test]
fn truncate_with_the_other_formats_address_errors() {
    assert_eq!(
        check_truncate_with_the_other_formats_address_errors(),
        Ok(())
    );
}

fn check_truncate_with_the_other_formats_address_errors() -> Result<(), Fail> {
    let input = make_iso_two_frames()?;
    let riff = BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")));
    ensure_eq!(
        apply(&Defect::TruncateBefore(riff), &input.file()),
        Err(DefectError::IncompatibleFileType)
    );
    Ok(())
}

#[test]
fn truncate_missing_box_errors() {
    assert_eq!(check_truncate_missing_box_errors(), Ok(()));
}

fn check_truncate_missing_box_errors() -> Result<(), Fail> {
    let input = make_iso_two_frames()?;
    let absent = BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"zzzz")));
    ensure_eq!(
        apply(&Defect::TruncateBefore(absent), &input.file()),
        Err(DefectError::AddressNotFound)
    );
    Ok(())
}

#[test]
fn truncate_non_container_errors() {
    let ftyp = BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"ftyp")));
    assert_eq!(
        apply(
            &Defect::TruncateBefore(ftyp),
            &ContainerFile(vec![0_u8; 100])
        ),
        Err(DefectError::IncompatibleFileType)
    );
}
