//! Tests for the truncation defects.

#[cfg(test)]
use super::*;
use crate::{encode_avi, encode_iso, AviCodec, IsoCodec};
use defects_box::ContainerBytes;
use synthvid_scene::{Dimensions, Frame, FrameIndex, Height, Width};

/// Creates a tiny 1×1 frame for testing.
fn make_test_frame() -> Frame {
    let width = Width::new(1).expect("valid width");
    let height = Height::new(1).expect("valid height");
    let dims = Dimensions::new(width, height);
    Frame::new(dims, vec![0_u8; 3]).expect("valid frame")
}

/// Creates an ISO file whose two frames of odd length give the layout
/// something awkward: same-tag siblings and an unaligned payload.
fn make_iso_two_frames() -> Vec<u8> {
    let width = Width::new(1).expect("valid width");
    let height = Height::new(1).expect("valid height");
    let frame = make_test_frame().data().to_vec();
    encode_iso(
        synthvid_scene::FrameRate::from_fps(24).expect("valid frame rate"),
        None,
        &[frame.clone(), frame],
        width,
        height,
        IsoCodec::UncompressedRgb,
        crate::TrackMatrix::identity(),
    )
    .expect("valid ISO encoding")
}

/// Creates an AVI file with two 3-byte frames: two same-tag `00db` chunks,
/// each followed by a pad byte.
fn make_avi_two_frames() -> Vec<u8> {
    let width = Width::new(1).expect("valid width");
    let height = Height::new(1).expect("valid height");
    let frame = make_test_frame().data().to_vec();
    encode_avi(
        synthvid_scene::FrameRate::from_fps(24).expect("valid frame rate"),
        &[frame.clone(), frame],
        width,
        height,
        AviCodec::UncompressedRgb,
    )
    .expect("valid AVI encoding")
}

/// The length of every file a truncation can produce, spelling each box's
/// start with `TruncateBefore` and every position inside it with
/// `TruncateWithin`.
fn every_cut_length(input: &[u8]) -> Vec<usize> {
    let layout = defects_layout::Layout::of(ContainerBytes::new(input)).expect("layout");
    let mut lengths = Vec::new();
    for region in layout.regions() {
        let address = region.address.clone();
        let (start, end) = layout.span(&address).expect("span");
        let before = apply(&Defect::TruncateBefore(address.clone()), input).expect("before");
        lengths.push(before.len());
        for offset in 1..end.saturating_sub(start) {
            let offset = u32::try_from(offset).expect("small offset");
            let within = apply(
                &Defect::TruncateWithin {
                    address: address.clone(),
                    offset: core::num::NonZeroU32::new(offset).expect("non-zero"),
                },
                input,
            )
            .expect("within");
            lengths.push(within.len());
        }
    }
    lengths.sort_unstable();
    lengths
}

#[test]
fn test_truncations_partition_iso() {
    let input = make_iso_two_frames();
    // Every cut position has exactly one spelling.
    assert_eq!(
        every_cut_length(&input),
        (0..input.len()).collect::<Vec<_>>()
    );
}

#[test]
fn test_truncations_partition_avi() {
    let input = make_avi_two_frames();
    assert_eq!(
        every_cut_length(&input),
        (0..input.len()).collect::<Vec<_>>()
    );
}

#[test]
fn test_truncations_partition_avi_with_every_padding_amount() {
    let width = Width::new(1).expect("valid width");
    let height = Height::new(1).expect("valid height");
    // Chunk lengths of 1, 2 and 5 bytes take 1, 0 and 1 pad bytes, where
    // padding to a 4-byte boundary would add 3, 2 and 3. Only a walker that
    // pads as RIFF does stays aligned.
    let input = encode_avi(
        synthvid_scene::FrameRate::from_fps(24).expect("valid frame rate"),
        &[vec![1], vec![1, 2], vec![1, 2, 3, 4, 5]],
        width,
        height,
        AviCodec::MotionJpeg,
    )
    .expect("valid AVI encoding");
    assert_eq!(
        every_cut_length(&input),
        (0..input.len()).collect::<Vec<_>>()
    );
}

#[test]
fn test_truncations_keep_a_prefix() {
    let input = make_avi_two_frames();
    let riff = BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")));
    let cut = apply(
        &Defect::TruncateWithin {
            address: riff,
            offset: core::num::NonZeroU32::new(5).expect("non-zero"),
        },
        &input,
    )
    .expect("within");
    assert_eq!(cut, input[..5]);
}

#[test]
fn test_truncate_before_first_box_is_empty() {
    let input = make_iso_two_frames();
    let ftyp = BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"ftyp")));
    let cut = apply(&Defect::TruncateBefore(ftyp), &input).expect("before");
    assert!(cut.is_empty());
}

#[test]
fn test_truncate_before_names_the_second_of_two_siblings() {
    let input = make_avi_two_frames();
    let frame = |index| {
        BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")))
            .child(BoxStep::first(BoxTag::new(*b"movi")))
            .child(BoxStep::frame(FrameIndex::new(index)))
    };
    let first = apply(&Defect::TruncateBefore(frame(0)), &input).expect("first");
    let second = apply(&Defect::TruncateBefore(frame(1)), &input).expect("second");
    // The first frame is an 8-byte header, 3 bytes of pixels and a pad byte.
    assert_eq!(second.len() - first.len(), 12);
}

/// Creates an MJPEG AVI whose first frame is 9 bytes (1 mod 4) and is
/// followed by a second, 13-byte frame.
fn make_avi_odd_mjpeg() -> Vec<u8> {
    let width = Width::new(16).expect("valid width");
    let height = Height::new(16).expect("valid height");
    encode_avi(
        synthvid_scene::FrameRate::from_fps(24).expect("valid frame rate"),
        &[vec![0xA5; 9], vec![0x5A; 13]],
        width,
        height,
        AviCodec::MotionJpeg,
    )
    .expect("valid AVI encoding")
}

#[test]
fn test_truncate_before_second_frame_after_odd_mjpeg_frame() {
    let input = make_avi_odd_mjpeg();
    let frame = |index| {
        BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")))
            .child(BoxStep::first(BoxTag::new(*b"movi")))
            .child(BoxStep::frame(FrameIndex::new(index)))
    };
    let first = apply(&Defect::TruncateBefore(frame(0)), &input).expect("first");
    let second = apply(&Defect::TruncateBefore(frame(1)), &input).expect("second");
    // An 8-byte header, 9 bytes of JPEG and exactly one pad byte.
    assert_eq!(second.len() - first.len(), 18);
    // A frame's region starts at its data, so its chunk header ends there.
    assert_eq!(&input[second.len() - 8..second.len() - 4], b"00dc");
    assert_eq!(
        every_cut_length(&input),
        (0..input.len()).collect::<Vec<_>>()
    );
}

/// The address of frame `index` under `movi`.
fn movi_frame(index: u32) -> BoxAddress {
    BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")))
        .child(BoxStep::first(BoxTag::new(*b"movi")))
        .child(BoxStep::frame(FrameIndex::new(index)))
}

#[test]
fn test_a_frame_address_holds_for_every_codec() {
    for input in [make_avi_two_frames(), make_avi_odd_mjpeg()] {
        let layout = defects_layout::Layout::of(ContainerBytes::new(&input)).expect("layout");
        assert!(layout.span(&movi_frame(0)).is_ok());
        assert!(layout.span(&movi_frame(1)).is_ok());
        assert_eq!(
            layout.span(&movi_frame(2)),
            Err(DefectError::AddressNotFound)
        );
    }
}

#[test]
fn test_an_offset_into_a_frame_counts_the_same_bytes_in_both_formats() {
    let iso_frame = BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"mdat")))
        .child(BoxStep::frame(FrameIndex::new(0)));
    for (input, frame) in [
        (make_iso_two_frames(), iso_frame),
        (make_avi_two_frames(), movi_frame(0)),
    ] {
        let before = apply(&Defect::TruncateBefore(frame.clone()), &input).expect("before");
        let within = apply(
            &Defect::TruncateWithin {
                address: frame,
                offset: core::num::NonZeroU32::new(2).expect("nonzero"),
            },
            &input,
        )
        .expect("within");
        assert_eq!(within.len() - before.len(), 2);
        assert_eq!(&input[..within.len()], &within[..]);
    }
}

#[test]
fn test_a_frame_has_no_tagged_address() {
    let input = make_avi_two_frames();
    let layout = defects_layout::Layout::of(ContainerBytes::new(&input)).expect("layout");
    let tagged = BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")))
        .child(BoxStep::first(BoxTag::new(*b"movi")))
        .child(BoxStep::first(BoxTag::new(*b"00db")));
    assert_eq!(layout.span(&tagged), Err(DefectError::AddressNotFound));
}

#[test]
fn test_iso_layout_reaches_sample_table_boxes() {
    let input = make_iso_two_frames();
    let layout = defects_layout::Layout::of(ContainerBytes::new(&input)).expect("layout");
    let stco = ["moov", "trak", "mdia", "minf", "stbl", "stco"]
        .into_iter()
        .map(|tag| BoxStep::first(BoxTag::new(tag.as_bytes().try_into().expect("four bytes"))))
        .fold(None, |address: Option<BoxAddress>, step| {
            Some(address.map_or_else(
                || BoxAddress::top(ContainerKind::Iso, step),
                |address| address.child(step),
            ))
        })
        .expect("a non-empty path");
    assert!(layout.span(&stco).is_ok());
}

#[test]
fn test_iso_lookup_does_not_descend_into_a_leaf_box() {
    let input = make_iso_two_frames();
    let path = BoxPath::root()
        .child(BoxTag::new(*b"mdat"))
        .child(BoxTag::new(*b"stco"));
    assert_eq!(
        defects_box::find_iso_box(ContainerBytes::new(&input), &path).map(|_| ()),
        Err(DefectError::BoxNotFound(path))
    );
}

#[test]
fn test_find_avi_box_finds_a_frame_shorter_than_a_chunk_header() {
    let input = make_avi_two_frames();
    let path = BoxPath::root()
        .child(BoxTag::new(*b"movi"))
        .child(BoxTag::new(*b"00db"));
    let frame = defects_box::find_avi_box(ContainerBytes::new(&input), &path).expect("frame");
    assert_eq!(frame.payload_end.get() - frame.payload_start.get(), 3);
}

#[test]
fn test_iso_frames_are_addressed_by_number_below_mdat() {
    let input = make_iso_two_frames();
    let layout = defects_layout::Layout::of(ContainerBytes::new(&input)).expect("layout");
    let frame = |index| {
        BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"mdat")))
            .child(BoxStep::frame(FrameIndex::new(index)))
    };
    let (first_start, first_end) = layout.span(&frame(0)).expect("frame 0");
    let (second_start, second_end) = layout.span(&frame(1)).expect("frame 1");
    assert_eq!(first_end, second_start);
    assert_eq!(second_end, input.len());
    assert_eq!(first_end - first_start, second_end - second_start);
    assert_eq!(layout.span(&frame(2)), Err(DefectError::AddressNotFound));
}

#[test]
fn test_find_avi_box_walks_past_odd_mjpeg_frame() {
    let input = make_avi_odd_mjpeg();
    let movi = BoxPath::root().child(BoxTag::new(*b"movi"));
    let first = defects_box::find_avi_box(
        ContainerBytes::new(&input),
        &movi.clone().child(BoxTag::new(*b"00dc")),
    )
    .expect("first frame");
    assert_eq!(first.payload_end.get() - first.payload_start.get(), 9);
    // Looking for a tag that is absent steps over both frames, then idx1;
    // a walker that misplaced the second frame would read garbage instead.
    let absent = movi.child(BoxTag::new(*b"JUNK"));
    assert_eq!(
        defects_box::find_avi_box(ContainerBytes::new(&input), &absent).map(|_| ()),
        Err(DefectError::BoxNotFound(absent))
    );
}

#[test]
fn test_truncate_within_past_owned_bytes_errors() {
    let input = make_iso_two_frames();
    let ftyp = BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"ftyp")));
    let layout = defects_layout::Layout::of(ContainerBytes::new(&input)).expect("layout");
    let (start, end) = layout.span(&ftyp).expect("span");
    let owned = u32::try_from(end.saturating_sub(start)).expect("small");
    let err = apply(
        &Defect::TruncateWithin {
            address: ftyp,
            offset: core::num::NonZeroU32::new(owned).expect("non-zero"),
        },
        &input,
    )
    .expect_err("offset at the next box's start is TruncateBefore, not this");
    assert_eq!(err, DefectError::OffsetOutOfRange);
}

#[test]
fn test_truncate_with_the_other_formats_address_errors() {
    let input = make_iso_two_frames();
    let riff = BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")));
    let err = apply(&Defect::TruncateBefore(riff), &input).expect_err("wrong format");
    assert_eq!(err, DefectError::IncompatibleFileType);
}

#[test]
fn test_truncate_missing_box_errors() {
    let input = make_iso_two_frames();
    let absent = BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"zzzz")));
    let err = apply(&Defect::TruncateBefore(absent), &input).expect_err("absent");
    assert_eq!(err, DefectError::AddressNotFound);
}

#[test]
fn test_truncate_non_container_errors() {
    let ftyp = BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"ftyp")));
    let err = apply(&Defect::TruncateBefore(ftyp), &[0_u8; 100]).expect_err("not a container");
    assert_eq!(err, DefectError::IncompatibleFileType);
}
