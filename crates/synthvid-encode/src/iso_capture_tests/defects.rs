//! Tests of the container defects, applied to a file that carries a capture rate.

use super::{
    at, box_path, cfps_payload, distinct_30000_1001_over_24, encode_seven_frames, iso, MOOV_START,
    MVHD_START,
};
use crate::defects::{apply, BoxAddress, BoxStep, BoxTag, ContainerKind, Defect};
use crate::media::ContainerFile;
use core::num::NonZeroU32;
use synthvid_testkit::iso::{
    payload, tags_in, u32_at, BigEndianWord, BoxTag as WalkTag, ByteOffset,
};
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

/// The file that results from applying `defect` to `file`.
fn applied(defect: &Defect, file: &ContainerFile) -> Result<ContainerFile, Fail> {
    apply(defect, file).or_fail()
}

/// The `moov` address of a top-level box, for a truncation.
fn top_level(tag: BoxTag) -> BoxAddress {
    BoxAddress::top(ContainerKind::Iso, BoxStep::first(tag))
}

#[test]
fn truncation_inside_moov_cuts_at_the_offset() {
    assert_eq!(check_truncation_inside_moov_cuts_at_the_offset(), Ok(()));
}

fn check_truncation_inside_moov_cuts_at_the_offset() -> Result<(), Fail> {
    let input = encode_seven_frames(distinct_30000_1001_over_24()?)?;
    let moov_len = at(&input, &[WalkTag(*b"moov")])?.0 .0.len();

    // A container owns only its own header, so a cut inside moov is a cut
    // inside its 8-byte header: 4 bytes in is 20 + 4 = 24.
    let inside_header = Defect::TruncateWithin {
        address: top_level(BoxTag::new(*b"moov")),
        offset: NonZeroU32::new(4).or_fail()?,
    };
    ensure_eq!(
        applied(&inside_header, &input)?.as_bytes(),
        input.as_bytes().get(..24).or_fail()?
    );

    // Inside mvhd, a leaf that owns its whole body: 40 bytes into mvhd, which
    // starts at 28, is 68.
    let inside_mvhd = Defect::TruncateWithin {
        address: BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"moov")))
            .child(BoxStep::first(BoxTag::new(*b"mvhd"))),
        offset: NonZeroU32::new(40).or_fail()?,
    };
    ensure_eq!(
        applied(&inside_mvhd, &input)?.as_bytes(),
        input.as_bytes().get(..68).or_fail()?
    );

    // Inside udta, the last child of moov: udta is 28 bytes (an 8-byte header
    // and the 20-byte cfps), so it starts 28 bytes before moov ends. The
    // defect addresses udta as a leaf, which owns all of its bytes, so 10
    // bytes in is udta_start + 10. cfps is not addressable on its own.
    ensure_eq!(
        at(&input, &[WalkTag(*b"moov"), WalkTag(*b"udta")])?
            .0
             .0
            .len(),
        28
    );
    let udta_start = MOOV_START
        .checked_add(moov_len)
        .and_then(|end| end.checked_sub(28))
        .or_fail()?;
    let cut = udta_start.checked_add(10).or_fail()?;
    let inside_udta = Defect::TruncateWithin {
        address: BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"moov")))
            .child(BoxStep::first(BoxTag::new(*b"udta"))),
        offset: NonZeroU32::new(10).or_fail()?,
    };
    ensure_eq!(
        applied(&inside_udta, &input)?.as_bytes(),
        input.as_bytes().get(..cut).or_fail()?
    );
    Ok(())
}

#[test]
fn duplicated_mvhd_adds_one_copy_inside_moov() {
    assert_eq!(check_duplicated_mvhd_adds_one_copy_inside_moov(), Ok(()));
}

fn check_duplicated_mvhd_adds_one_copy_inside_moov() -> Result<(), Fail> {
    let input = encode_seven_frames(distinct_30000_1001_over_24()?)?;
    let mvhd_len = at(&input, &[WalkTag(*b"moov"), WalkTag(*b"mvhd")])?
        .0
         .0
        .len();
    // mvhd is 108 bytes: an 8-byte header and a 100-byte body.
    ensure_eq!(mvhd_len, 108);

    let defect = Defect::DuplicateBox(box_path(&[WalkTag(*b"moov"), WalkTag(*b"mvhd")]));
    let result = applied(&defect, &input)?;
    ensure_eq!(
        result.as_bytes().len(),
        input.as_bytes().len().checked_add(mvhd_len).or_fail()?
    );
    ensure_eq!(
        tags_in(payload(at(&result, &[WalkTag(*b"moov")])?)?)?,
        vec![
            WalkTag(*b"mvhd"),
            WalkTag(*b"mvhd"),
            WalkTag(*b"trak"),
            WalkTag(*b"udta"),
        ]
    );
    ensure_eq!(
        at(&result, &[WalkTag(*b"moov"), WalkTag(*b"mvhd")])?,
        at(&input, &[WalkTag(*b"moov"), WalkTag(*b"mvhd")])?
    );
    // The capture rate is still the last child, and still 24/1.
    ensure_eq!(
        tags_in(payload(at(
            &result,
            &[WalkTag(*b"moov"), WalkTag(*b"udta")]
        )?)?)?,
        vec![WalkTag(*b"cfps")]
    );
    let expected: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0x18, 0, 0, 0, 1];
    ensure_eq!(cfps_payload(&result)?.0, expected);
    Ok(())
}

#[test]
fn oversized_mvhd_length_is_written_over_the_size_word_only() {
    assert_eq!(
        check_oversized_mvhd_length_is_written_over_the_size_word_only(),
        Ok(())
    );
}

fn check_oversized_mvhd_length_is_written_over_the_size_word_only() -> Result<(), Fail> {
    let input = encode_seven_frames(distinct_30000_1001_over_24()?)?;
    ensure_eq!(
        u32_at(iso(&input), ByteOffset(MVHD_START))?,
        BigEndianWord(108)
    );

    let defect = Defect::OversizedBoxLength(box_path(&[WalkTag(*b"moov"), WalkTag(*b"mvhd")]));
    let result = applied(&defect, &input)?;

    // 108 + 256 = 364 = 0x0000_016C, and no other byte changes.
    let mut expected = input.as_bytes().to_vec();
    let end = MVHD_START.checked_add(4).or_fail()?;
    expected
        .get_mut(MVHD_START..end)
        .or_fail()?
        .copy_from_slice(&[0, 0, 0x01, 0x6C]);
    ensure_eq!(result.as_bytes(), expected.as_slice());
    Ok(())
}

#[test]
fn undersized_mvhd_length_is_written_over_the_size_word_only() {
    assert_eq!(
        check_undersized_mvhd_length_is_written_over_the_size_word_only(),
        Ok(())
    );
}

fn check_undersized_mvhd_length_is_written_over_the_size_word_only() -> Result<(), Fail> {
    let input = encode_seven_frames(distinct_30000_1001_over_24()?)?;
    ensure_eq!(
        u32_at(iso(&input), ByteOffset(MVHD_START))?,
        BigEndianWord(108)
    );

    let defect = Defect::UndersizedBoxLength(box_path(&[WalkTag(*b"moov"), WalkTag(*b"mvhd")]));
    let result = applied(&defect, &input)?;

    // 108 - 1 = 107 = 0x0000_006B, and no other byte changes.
    let mut expected = input.as_bytes().to_vec();
    let end = MVHD_START.checked_add(4).or_fail()?;
    expected
        .get_mut(MVHD_START..end)
        .or_fail()?
        .copy_from_slice(&[0, 0, 0, 0x6B]);
    ensure_eq!(result.as_bytes(), expected.as_slice());
    Ok(())
}

#[test]
fn unknown_box_after_trak_lands_before_udta() {
    assert_eq!(check_unknown_box_after_trak_lands_before_udta(), Ok(()));
}

fn check_unknown_box_after_trak_lands_before_udta() -> Result<(), Fail> {
    let input = encode_seven_frames(distinct_30000_1001_over_24()?)?;
    let defect = Defect::UnknownBox {
        after: box_path(&[WalkTag(*b"moov"), WalkTag(*b"trak")]),
        tag: BoxTag::new(*b"zzzz"),
    };
    let result = applied(&defect, &input)?;

    // An empty box is its 8-byte header: size 8, then the tag. The defect is
    // named for its parent, so the box becomes the last child of trak, and
    // trak and moov grow by its length.
    ensure_eq!(
        result.as_bytes().len(),
        input.as_bytes().len().checked_add(8).or_fail()?
    );
    ensure_eq!(
        tags_in(payload(at(&result, &[WalkTag(*b"moov")])?)?)?,
        vec![WalkTag(*b"mvhd"), WalkTag(*b"trak"), WalkTag(*b"udta")]
    );
    ensure_eq!(
        tags_in(payload(at(
            &result,
            &[WalkTag(*b"moov"), WalkTag(*b"trak")]
        )?)?)?,
        vec![WalkTag(*b"tkhd"), WalkTag(*b"mdia"), WalkTag(*b"zzzz")]
    );
    ensure_eq!(
        at(
            &result,
            &[WalkTag(*b"moov"), WalkTag(*b"trak"), WalkTag(*b"zzzz")]
        )?
        .0
         .0,
        [0, 0, 0, 8, b'z', b'z', b'z', b'z'].as_slice()
    );
    // The udta that follows trak is untouched and still holds the capture rate.
    ensure_eq!(
        tags_in(payload(at(
            &result,
            &[WalkTag(*b"moov"), WalkTag(*b"udta")]
        )?)?)?,
        vec![WalkTag(*b"cfps")]
    );
    let expected: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0x18, 0, 0, 0, 1];
    ensure_eq!(cfps_payload(&result)?.0, expected);
    Ok(())
}
