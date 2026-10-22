//! Tests of the `QuickTime`-style `meta` carrier of a distinct capture rate.
//!
//! Every byte is read through the testkit walker, never searched for, and every
//! expected layout is written out by hand from the box sizes.

use super::{
    at, distinct_30000_1001_over_24, encode_seven_frames, frame_rate, playback_30000_1001,
    Fraction, FractionDenominator, FractionNumerator, MOOV_START,
};
use crate::defects::{apply, BoxAddress, BoxStep, BoxTag, ContainerKind, Defect};
use crate::media::ContainerFile;
use core::num::NonZeroU32;
use synthvid_scene::{CaptureCarrier, FrameRate, FrameRates, RatePair};
use synthvid_testkit::iso::{
    payload, tags_in, u32_at, BigEndianWord, BoxTag as WalkTag, ByteOffset, IsoBytes,
};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// The tags of the children of `moov`, in file order.
fn tags_in_moov(file: &ContainerFile) -> Result<Vec<WalkTag>, Fail> {
    tags_in(payload(at(file, &[WalkTag(*b"moov")])?)?)
}

/// Playback 30000/1001 with a capture rate of `capture`, carried in the `QuickTime` `meta` box.
fn quicktime_over_30000_1001(capture: FrameRate) -> Result<FrameRates, Fail> {
    FrameRates::distinct_in(
        RatePair {
            playback: playback_30000_1001()?,
            capture,
        },
        CaptureCarrier::QuickTimeMeta,
    )
    .or_fail()
}

/// Playback 30000/1001 with a capture rate of 24000/1001, carried in the `meta` box.
fn quicktime_24000_1001() -> Result<FrameRates, Fail> {
    quicktime_over_30000_1001(frame_rate(Fraction {
        numer: FractionNumerator(24000),
        denom: FractionDenominator(1001),
    })?)
}

/// The four big-endian bytes of a float32.
#[derive(Clone, Copy)]
struct FloatBytes([u8; 4]);

/// The bytes of one complete meta box.
struct MetaBytes(Vec<u8>);

/// The meta box that carries `float` as its capture rate, laid out by hand: 136 bytes.
///
/// - `meta`: size 136, type, then version 0 and zero flags (12 bytes).
/// - `hdlr`: size 33; version and flags; pre-defined 0; handler `mdta`; 12 reserved
///   zeros; a name of one null byte (8 + 4 + 4 + 4 + 12 + 1 = 33).
/// - `keys`: size 55; version and flags; entry count 1; then one entry of size 39
///   (4 + 4 + 31), namespace `mdta`, and the 31-byte key. 8 + 4 + 4 + 39 = 55.
/// - `ilst`: size 36; one item of size 28 whose type is the big-endian index 1; inside
///   it a `data` box of size 20 holding indicator 23, locale 0 and the float32.
///   8 + 28 = 36, and 8 + 4 + 4 + 4 = 20 for the data box.
///
/// 12 + 33 + 55 + 36 = 136.
fn expected_meta(float: FloatBytes) -> Result<MetaBytes, Fail> {
    let key: [u8; 31] = *b"com.apple.quicktime.capture.fps";
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[0, 0, 0, 136]);
    bytes.extend_from_slice(b"meta");
    bytes.extend_from_slice(&[0, 0, 0, 0]);

    bytes.extend_from_slice(&[0, 0, 0, 33]);
    bytes.extend_from_slice(b"hdlr");
    bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(b"mdta");
    bytes.extend_from_slice(&[0; 12]);
    bytes.push(0);

    bytes.extend_from_slice(&[0, 0, 0, 55]);
    bytes.extend_from_slice(b"keys");
    bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    bytes.extend_from_slice(&[0, 0, 0, 39]);
    bytes.extend_from_slice(b"mdta");
    bytes.extend_from_slice(&key);

    bytes.extend_from_slice(&[0, 0, 0, 36]);
    bytes.extend_from_slice(b"ilst");
    bytes.extend_from_slice(&[0, 0, 0, 28, 0, 0, 0, 1]);
    bytes.extend_from_slice(&[0, 0, 0, 20]);
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&[0, 0, 0, 23, 0, 0, 0, 0]);
    bytes.extend_from_slice(&float.0);

    ensure_eq!(bytes.len(), 136);
    Ok(MetaBytes(bytes))
}

#[test]
fn meta_box_for_24000_1001_has_the_hand_laid_out_bytes() {
    assert_eq!(
        check_meta_box_for_24000_1001_has_the_hand_laid_out_bytes(),
        Ok(())
    );
}

// 24000/1001 is 23.976023976..., in [16, 32): biased exponent 131 (0x83), and the
// significand 1.4985014985... is 0x3FCEE6 in 23 bits after rounding, so the word is
// 0x41BFCEE6.
fn check_meta_box_for_24000_1001_has_the_hand_laid_out_bytes() -> Result<(), Fail> {
    let bytes = encode_seven_frames(quicktime_24000_1001()?)?;
    ensure_eq!(
        at(&bytes, &[WalkTag(*b"moov"), WalkTag(*b"meta")])?.0 .0,
        expected_meta(FloatBytes([0x41, 0xBF, 0xCE, 0xE6]))?
            .0
            .as_slice()
    );
    Ok(())
}

#[test]
fn meta_box_for_240_1_has_the_hand_laid_out_bytes() {
    assert_eq!(
        check_meta_box_for_240_1_has_the_hand_laid_out_bytes(),
        Ok(())
    );
}

// 240 is 1.875 * 2^7: biased exponent 134 (0x86), fraction 0.875, which is 0x700000 in
// 23 bits. The word is 0x43700000, exact in float32.
fn check_meta_box_for_240_1_has_the_hand_laid_out_bytes() -> Result<(), Fail> {
    let capture = frame_rate(Fraction {
        numer: FractionNumerator(240),
        denom: FractionDenominator(1),
    })?;
    let bytes = encode_seven_frames(quicktime_over_30000_1001(capture)?)?;
    ensure_eq!(
        at(&bytes, &[WalkTag(*b"moov"), WalkTag(*b"meta")])?.0 .0,
        expected_meta(FloatBytes([0x43, 0x70, 0x00, 0x00]))?
            .0
            .as_slice()
    );
    Ok(())
}

#[test]
fn meta_is_the_last_child_of_moov_after_trak() {
    assert_eq!(check_meta_is_the_last_child_of_moov_after_trak(), Ok(()));
}

fn check_meta_is_the_last_child_of_moov_after_trak() -> Result<(), Fail> {
    let bytes = encode_seven_frames(quicktime_24000_1001()?)?;
    ensure_eq!(
        tags_in_moov(&bytes)?,
        vec![WalkTag(*b"mvhd"), WalkTag(*b"trak"), WalkTag(*b"meta")]
    );
    Ok(())
}

#[test]
fn quicktime_form_has_no_cfps_and_project_form_has_no_meta() {
    assert_eq!(
        check_quicktime_form_has_no_cfps_and_project_form_has_no_meta(),
        Ok(())
    );
}

fn check_quicktime_form_has_no_cfps_and_project_form_has_no_meta() -> Result<(), Fail> {
    let quicktime = encode_seven_frames(quicktime_24000_1001()?)?;
    let project = encode_seven_frames(distinct_30000_1001_over_24()?)?;
    let quicktime_moov = tags_in_moov(&quicktime)?;
    let project_moov = tags_in_moov(&project)?;
    ensure!(
        !quicktime_moov.contains(&WalkTag(*b"udta")),
        "the QuickTime form writes no udta, so no cfps"
    );
    ensure!(
        !project_moov.contains(&WalkTag(*b"meta")),
        "the project form writes no meta"
    );
    ensure_eq!(
        project_moov,
        vec![WalkTag(*b"mvhd"), WalkTag(*b"trak"), WalkTag(*b"udta")]
    );
    Ok(())
}

#[test]
fn playback_only_writes_no_meta() {
    assert_eq!(check_playback_only_writes_no_meta(), Ok(()));
}

fn check_playback_only_writes_no_meta() -> Result<(), Fail> {
    let bytes = encode_seven_frames(FrameRates::playback_only(playback_30000_1001()?))?;
    ensure_eq!(
        tags_in_moov(&bytes)?,
        vec![WalkTag(*b"mvhd"), WalkTag(*b"trak")]
    );
    Ok(())
}

/// The file that results from applying `defect` to `file`.
fn applied(defect: &Defect, file: &ContainerFile) -> Result<ContainerFile, Fail> {
    apply(defect, file).or_fail()
}

#[test]
fn truncation_inside_meta_cuts_at_the_offset() {
    assert_eq!(check_truncation_inside_meta_cuts_at_the_offset(), Ok(()));
}

fn check_truncation_inside_meta_cuts_at_the_offset() -> Result<(), Fail> {
    let input = encode_seven_frames(quicktime_24000_1001()?)?;
    let moov_len = at(&input, &[WalkTag(*b"moov")])?.0 .0.len();
    let meta_len = at(&input, &[WalkTag(*b"moov"), WalkTag(*b"meta")])?
        .0
         .0
        .len();
    // meta is the last child of moov, so it ends where moov does.
    let meta_start = MOOV_START
        .checked_add(moov_len)
        .and_then(|end| end.checked_sub(meta_len))
        .or_fail()?;
    // 10 bytes into meta is past its 8-byte header, inside its version and flags.
    let cut = meta_start.checked_add(10).or_fail()?;
    let defect = Defect::TruncateWithin {
        address: BoxAddress::top(ContainerKind::Iso, BoxStep::first(BoxTag::new(*b"moov")))
            .child(BoxStep::first(BoxTag::new(*b"meta"))),
        offset: NonZeroU32::new(10).or_fail()?,
    };
    ensure_eq!(
        applied(&defect, &input)?.as_bytes(),
        input.as_bytes().get(..cut).or_fail()?
    );
    Ok(())
}

#[test]
fn size_omission_applies_to_a_quicktime_file_and_leaves_meta_alone() {
    assert_eq!(
        check_size_omission_applies_to_a_quicktime_file_and_leaves_meta_alone(),
        Ok(())
    );
}

fn check_size_omission_applies_to_a_quicktime_file_and_leaves_meta_alone() -> Result<(), Fail> {
    let input = encode_seven_frames(quicktime_24000_1001()?)?;
    let result = applied(&Defect::NoContainerSize, &input)?;

    // The size is zeroed in tkhd (width and height at payload 76..84) and nowhere else.
    let tkhd = payload(at(
        &result,
        &[WalkTag(*b"moov"), WalkTag(*b"trak"), WalkTag(*b"tkhd")],
    )?)?;
    ensure_eq!(tkhd.0.get(76..84).or_fail()?, [0_u8; 8].as_slice());
    ensure_eq!(result.as_bytes().len(), input.as_bytes().len());
    ensure_eq!(
        at(&result, &[WalkTag(*b"moov"), WalkTag(*b"meta")])?.0 .0,
        at(&input, &[WalkTag(*b"moov"), WalkTag(*b"meta")])?.0 .0
    );
    ensure_eq!(
        tags_in_moov(&result)?,
        vec![WalkTag(*b"mvhd"), WalkTag(*b"trak"), WalkTag(*b"meta")]
    );
    Ok(())
}

#[test]
fn stco_first_entry_points_at_the_first_frame_with_a_quicktime_meta() {
    assert_eq!(
        check_stco_first_entry_points_at_the_first_frame_with_a_quicktime_meta(),
        Ok(())
    );
}

fn check_stco_first_entry_points_at_the_first_frame_with_a_quicktime_meta() -> Result<(), Fail> {
    let bytes = encode_seven_frames(quicktime_24000_1001()?)?;

    // Top level is ftyp, moov, then mdat, as in the project-box form.
    ensure_eq!(
        tags_in(IsoBytes(bytes.as_bytes()))?,
        vec![WalkTag(*b"ftyp"), WalkTag(*b"moov"), WalkTag(*b"mdat")]
    );

    // The moov here carries meta in place of udta, so its length is taken from the file
    // rather than assumed. mdat starts where moov ends; its 8-byte header precedes the
    // first frame.
    let moov_len = at(&bytes, &[WalkTag(*b"moov")])?.0 .0.len();
    let first_frame = MOOV_START
        .checked_add(moov_len)
        .and_then(|end| end.checked_add(8))
        .or_fail()?;
    let second_frame = first_frame.checked_add(45).or_fail()?;

    // stco: 8-byte header, version and flags, entry count, then entries.
    let stco = at(
        &bytes,
        &[
            WalkTag(*b"moov"),
            WalkTag(*b"trak"),
            WalkTag(*b"mdia"),
            WalkTag(*b"minf"),
            WalkTag(*b"stbl"),
            WalkTag(*b"stco"),
        ],
    )?;
    ensure_eq!(
        u32_at(stco.0, ByteOffset(16))?,
        BigEndianWord(u32::try_from(first_frame).or_fail()?)
    );

    // The bytes there are the first frame (fill 0).
    let frame0 = bytes.as_bytes().get(first_frame..second_frame).or_fail()?;
    ensure_eq!(frame0, vec![0_u8; 45].as_slice());
    Ok(())
}
