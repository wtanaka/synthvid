use crate::defects::{BoxPath, BoxTag};
use crate::iso::{encode_iso, IsoCodec, IsoError, TrackMatrix};
use crate::iso_helpers::TrackParams;
use crate::media::{ContainerFile, EncodedFrame};
use core::num::{NonZeroI64, NonZeroU32};
use synthvid_scene::{
    CaptureCarrier, FrameRate, FrameRates, Height, Numerator, RatePair, Ratio, Width,
};
use synthvid_testkit::iso::{
    child_path, payload, sibling_boxes, tags_in, u32_at, BigEndianWord, BoxPath as WalkPath,
    BoxTag as WalkTag, BoxView, ByteOffset, IsoBytes,
};
use synthvid_testkit::{ensure, ensure_eq, refusal, Fail, OrFail as _};

/// A box path from tags, for the defects that name one.
fn box_path(tags: &[WalkTag]) -> BoxPath {
    BoxPath::from_tags(tags.iter().map(|tag| BoxTag::new(tag.0)).collect())
}

/// The walker's view of a whole container file.
fn iso(file: &ContainerFile) -> IsoBytes<'_> {
    IsoBytes(file.as_bytes())
}

/// The box at `path` in `file`, header included.
fn at<'file>(file: &'file ContainerFile, path: &[WalkTag]) -> Result<BoxView<'file>, Fail> {
    child_path(iso(file), WalkPath(path))
}

/// Seven 5x3 RGB frames, 45 bytes each, encoded at the given frame rates.
fn encode_seven_frames(rates: FrameRates) -> Result<ContainerFile, Fail> {
    let frames: Vec<EncodedFrame> = (0..7_u8).map(|fill| EncodedFrame(vec![fill; 45])).collect();
    let track_params = TrackParams {
        width: Width::new(5).or_fail()?,
        height: Height::new(3).or_fail()?,
        codec: IsoCodec::UncompressedRgb,
        track_matrix: TrackMatrix::identity(),
    };
    encode_iso(rates, &frames, track_params).or_fail()
}

/// The error encoding seven frames at the given rates returns.
fn encode_seven_frames_error(rates: FrameRates) -> Result<IsoError, Fail> {
    let frames: Vec<EncodedFrame> = (0..7_u8).map(|fill| EncodedFrame(vec![fill; 45])).collect();
    let track_params = TrackParams {
        width: Width::new(5).or_fail()?,
        height: Height::new(3).or_fail()?,
        codec: IsoCodec::UncompressedRgb,
        track_matrix: TrackMatrix::identity(),
    };
    refusal(encode_iso(rates, &frames, track_params))
}

/// The number of frames in a fraction, before reduction.
#[derive(Clone, Copy)]
struct FractionNumerator(u32);

/// The number of seconds in a fraction, before reduction.
#[derive(Clone, Copy)]
struct FractionDenominator(u32);

/// A fraction written as its numerator over its denominator, before reduction.
#[derive(Clone, Copy)]
struct Fraction {
    numer: FractionNumerator,
    denom: FractionDenominator,
}

/// A frame rate from a fraction whose denominator is non-zero.
fn frame_rate(fraction: Fraction) -> Result<FrameRate, Fail> {
    FrameRate::from_fraction(
        fraction.numer.0,
        NonZeroU32::new(fraction.denom.0).or_fail()?,
    )
    .or_fail()
}

/// Playback 30000/1001 alone, for comparison with the distinct-capture file.
fn playback_30000_1001() -> Result<FrameRate, Fail> {
    frame_rate(Fraction {
        numer: FractionNumerator(30000),
        denom: FractionDenominator(1001),
    })
}

/// Playback 30000/1001 with a distinct capture rate of 24/1.
fn distinct_30000_1001_over_24() -> Result<FrameRates, Fail> {
    let capture = FrameRate::from_fps(24).or_fail()?;
    FrameRates::distinct_in(
        RatePair {
            playback: playback_30000_1001()?,
            capture,
        },
        CaptureCarrier::ProjectBox,
    )
    .or_fail()
}

/// The 12-byte payload of the `cfps` box, the only box a capture rate writes.
struct CfpsPayload([u8; 12]);

/// The payload of the `cfps` box in `file`.
fn cfps_payload(file: &ContainerFile) -> Result<CfpsPayload, Fail> {
    let boxed = at(
        file,
        &[WalkTag(*b"moov"), WalkTag(*b"udta"), WalkTag(*b"cfps")],
    )?;
    let payload: [u8; 12] = payload(boxed)?.0.try_into().or_fail()?;
    Ok(CfpsPayload(payload))
}

/// The `cfps` payload written for a distinct capture rate over a playback rate.
fn cfps_payload_for(rates: RatePair) -> Result<CfpsPayload, Fail> {
    let frame_rates = FrameRates::distinct_in(rates, CaptureCarrier::ProjectBox).or_fail()?;
    cfps_payload(&encode_seven_frames(frame_rates)?)
}

// Hand-computed for seven frames, playback 30000/1001, capture 24/1:
//   movie timescale (mvhd)           = playback numerator            = 30000
//   movie duration (mvhd, tkhd)      = 7 frames * playback denom 1001 = 7007
//   media timescale (mdhd)           = playback numerator            = 30000
//   media duration (mdhd)            = 7 frames * playback denom 1001 = 7007
//   stts sample_count / sample_delta = 7 / playback denom 1001        = 7 / 1001
//   cfps numerator / denominator     = capture 24 / 1                 = 24 / 1
#[test]
fn distinct_rates_write_hand_computed_container_timing() {
    assert_eq!(
        check_distinct_rates_write_hand_computed_container_timing(),
        Ok(())
    );
}

fn check_distinct_rates_write_hand_computed_container_timing() -> Result<(), Fail> {
    let bytes = encode_seven_frames(distinct_30000_1001_over_24()?)?;

    // Offsets count from the start of each box, header included. mvhd:
    // size, type, version/flags, creation and modification times (16), then
    // the timescale (20) and the duration (24).
    let movie_header = at(&bytes, &[WalkTag(*b"moov"), WalkTag(*b"mvhd")])?;
    ensure_eq!(
        u32_at(movie_header.0, ByteOffset(20))?,
        BigEndianWord(30000)
    );
    ensure_eq!(u32_at(movie_header.0, ByteOffset(24))?, BigEndianWord(7007));
    // tkhd: the same 16, then track ID and a reserved word (24) before the
    // duration (28).
    let tkhd = at(
        &bytes,
        &[WalkTag(*b"moov"), WalkTag(*b"trak"), WalkTag(*b"tkhd")],
    )?;
    ensure_eq!(u32_at(tkhd.0, ByteOffset(28))?, BigEndianWord(7007));
    // mdhd: timescale (20) and duration (24), both at the playback rate.
    let media_header = at(
        &bytes,
        &[
            WalkTag(*b"moov"),
            WalkTag(*b"trak"),
            WalkTag(*b"mdia"),
            WalkTag(*b"mdhd"),
        ],
    )?;
    ensure_eq!(
        u32_at(media_header.0, ByteOffset(20))?,
        BigEndianWord(30000)
    );
    ensure_eq!(u32_at(media_header.0, ByteOffset(24))?, BigEndianWord(7007));
    // stts: entry count (12), sample count (16), sample delta (20).
    let stts = at(
        &bytes,
        &[
            WalkTag(*b"moov"),
            WalkTag(*b"trak"),
            WalkTag(*b"mdia"),
            WalkTag(*b"minf"),
            WalkTag(*b"stbl"),
            WalkTag(*b"stts"),
        ],
    )?;
    ensure_eq!(u32_at(stts.0, ByteOffset(12))?, BigEndianWord(1));
    ensure_eq!(u32_at(stts.0, ByteOffset(16))?, BigEndianWord(7));
    ensure_eq!(u32_at(stts.0, ByteOffset(20))?, BigEndianWord(1001));
    Ok(())
}

#[test]
fn distinct_capture_writes_no_edit_list() {
    assert_eq!(check_distinct_capture_writes_no_edit_list(), Ok(()));
}

fn check_distinct_capture_writes_no_edit_list() -> Result<(), Fail> {
    let bytes = encode_seven_frames(distinct_30000_1001_over_24()?)?;
    let moov = payload(at(&bytes, &[WalkTag(*b"moov")])?)?;
    let trak = payload(at(&bytes, &[WalkTag(*b"moov"), WalkTag(*b"trak")])?)?;
    ensure!(
        !tags_in(moov)?.contains(&WalkTag(*b"edts")),
        "no edts among the children of moov"
    );
    ensure!(
        !tags_in(trak)?.contains(&WalkTag(*b"edts")),
        "no edts among the children of trak"
    );
    Ok(())
}

#[test]
fn container_timing_matches_playback_only_file() {
    assert_eq!(check_container_timing_matches_playback_only_file(), Ok(()));
}

fn check_container_timing_matches_playback_only_file() -> Result<(), Fail> {
    let distinct = encode_seven_frames(distinct_30000_1001_over_24()?)?;
    let playback = encode_seven_frames(FrameRates::playback_only(playback_30000_1001()?))?;
    let paths: [&[WalkTag]; 4] = [
        &[WalkTag(*b"moov"), WalkTag(*b"mvhd")],
        &[WalkTag(*b"moov"), WalkTag(*b"trak"), WalkTag(*b"tkhd")],
        &[
            WalkTag(*b"moov"),
            WalkTag(*b"trak"),
            WalkTag(*b"mdia"),
            WalkTag(*b"mdhd"),
        ],
        &[
            WalkTag(*b"moov"),
            WalkTag(*b"trak"),
            WalkTag(*b"mdia"),
            WalkTag(*b"minf"),
            WalkTag(*b"stbl"),
            WalkTag(*b"stts"),
        ],
    ];
    for path in paths {
        ensure_eq!(at(&distinct, path)?, at(&playback, path)?);
    }
    Ok(())
}

#[test]
fn capture_rate_is_the_last_child_of_moov() {
    assert_eq!(check_capture_rate_is_the_last_child_of_moov(), Ok(()));
}

fn check_capture_rate_is_the_last_child_of_moov() -> Result<(), Fail> {
    let bytes = encode_seven_frames(distinct_30000_1001_over_24()?)?;
    let moov_children = sibling_boxes(payload(at(&bytes, &[WalkTag(*b"moov")])?)?)?;
    let last = *moov_children.last().or_fail()?;
    ensure_eq!(
        last.tag,
        WalkTag(*b"udta"),
        "udta is the last child of moov"
    );

    let udta_tags = tags_in(payload(last.boxed)?)?;
    ensure_eq!(udta_tags, vec![WalkTag(*b"cfps")], "udta holds only cfps");

    // cfps: 8-byte header (size 20), version 0, flags 0, numerator 24, denominator 1.
    let cfps = at(
        &bytes,
        &[WalkTag(*b"moov"), WalkTag(*b"udta"), WalkTag(*b"cfps")],
    )?;
    ensure_eq!(cfps.0 .0.len(), 20);
    let expected: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0x18, 0, 0, 0, 1];
    ensure_eq!(cfps_payload(&bytes)?.0, expected);
    Ok(())
}

#[test]
fn cfps_writes_the_reduced_capture_rate_over_a_fraction_playback() {
    assert_eq!(
        check_cfps_writes_the_reduced_capture_rate_over_a_fraction_playback(),
        Ok(())
    );
}

fn check_cfps_writes_the_reduced_capture_rate_over_a_fraction_playback() -> Result<(), Fail> {
    // 24000 = 0x5DC0 (93 * 256 + 192); 1001 = 0x03E9 (3 * 256 + 233).
    let expected: [u8; 12] = [0, 0, 0, 0, 0, 0, 0x5D, 0xC0, 0, 0, 0x03, 0xE9];
    ensure_eq!(
        cfps_payload_for(RatePair {
            playback: playback_30000_1001()?,
            capture: frame_rate(Fraction {
                numer: FractionNumerator(24000),
                denom: FractionDenominator(1001),
            })?,
        })?
        .0,
        expected
    );
    Ok(())
}

#[test]
fn cfps_writes_an_integer_capture_rate_over_an_integer_playback() {
    assert_eq!(
        check_cfps_writes_an_integer_capture_rate_over_an_integer_playback(),
        Ok(())
    );
}

fn check_cfps_writes_an_integer_capture_rate_over_an_integer_playback() -> Result<(), Fail> {
    // 240 = 0xF0; denominator 1.
    let expected: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0xF0, 0, 0, 0, 1];
    ensure_eq!(
        cfps_payload_for(RatePair {
            playback: frame_rate(Fraction {
                numer: FractionNumerator(30),
                denom: FractionDenominator(1),
            })?,
            capture: frame_rate(Fraction {
                numer: FractionNumerator(240),
                denom: FractionDenominator(1),
            })?,
        })?
        .0,
        expected
    );
    Ok(())
}

#[test]
fn cfps_writes_a_capture_rate_slower_than_playback() {
    assert_eq!(
        check_cfps_writes_a_capture_rate_slower_than_playback(),
        Ok(())
    );
}

fn check_cfps_writes_a_capture_rate_slower_than_playback() -> Result<(), Fail> {
    // 12 = 0x0C; denominator 1.
    let expected: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0x0C, 0, 0, 0, 1];
    ensure_eq!(
        cfps_payload_for(RatePair {
            playback: frame_rate(Fraction {
                numer: FractionNumerator(30),
                denom: FractionDenominator(1),
            })?,
            capture: frame_rate(Fraction {
                numer: FractionNumerator(12),
                denom: FractionDenominator(1),
            })?,
        })?
        .0,
        expected
    );
    Ok(())
}

#[test]
fn cfps_writes_an_unreduced_capture_rate_reduced() {
    assert_eq!(
        check_cfps_writes_an_unreduced_capture_rate_reduced(),
        Ok(())
    );
}

fn check_cfps_writes_an_unreduced_capture_rate_reduced() -> Result<(), Fail> {
    // 48000/2002 is 24000/1001 once reduced by two.
    let expected: [u8; 12] = [0, 0, 0, 0, 0, 0, 0x5D, 0xC0, 0, 0, 0x03, 0xE9];
    ensure_eq!(
        cfps_payload_for(RatePair {
            playback: playback_30000_1001()?,
            capture: frame_rate(Fraction {
                numer: FractionNumerator(48000),
                denom: FractionDenominator(2002),
            })?,
        })?
        .0,
        expected
    );
    Ok(())
}

#[test]
fn capture_numerator_above_u32_is_refused() {
    assert_eq!(check_capture_numerator_above_u32_is_refused(), Ok(()));
}

fn check_capture_numerator_above_u32_is_refused() -> Result<(), Fail> {
    // 5_000_000_000 is above u32::MAX (4_294_967_295).
    let capture = FrameRate::new(
        Ratio::new(Numerator(5_000_000_000), NonZeroI64::new(1).or_fail()?).or_fail()?,
    )
    .or_fail()?;
    let rates = FrameRates::distinct_in(
        RatePair {
            playback: playback_30000_1001()?,
            capture,
        },
        CaptureCarrier::ProjectBox,
    )
    .or_fail()?;
    ensure_eq!(
        encode_seven_frames_error(rates)?,
        IsoError::CaptureRateTooLarge
    );
    Ok(())
}

#[test]
fn capture_denominator_above_u32_is_refused() {
    assert_eq!(check_capture_denominator_above_u32_is_refused(), Ok(()));
}

fn check_capture_denominator_above_u32_is_refused() -> Result<(), Fail> {
    // 5_000_000_000 is above u32::MAX (4_294_967_295).
    let capture = FrameRate::new(
        Ratio::new(Numerator(1), NonZeroI64::new(5_000_000_000).or_fail()?).or_fail()?,
    )
    .or_fail()?;
    let rates = FrameRates::distinct_in(
        RatePair {
            playback: playback_30000_1001()?,
            capture,
        },
        CaptureCarrier::ProjectBox,
    )
    .or_fail()?;
    ensure_eq!(
        encode_seven_frames_error(rates)?,
        IsoError::CaptureRateTooLarge
    );
    Ok(())
}

#[test]
fn playback_only_has_no_udta_or_edts() {
    assert_eq!(check_playback_only_has_no_udta_or_edts(), Ok(()));
}

fn check_playback_only_has_no_udta_or_edts() -> Result<(), Fail> {
    let bytes = encode_seven_frames(FrameRates::playback_only(playback_30000_1001()?))?;
    let moov = payload(at(&bytes, &[WalkTag(*b"moov")])?)?;
    let trak = payload(at(&bytes, &[WalkTag(*b"moov"), WalkTag(*b"trak")])?)?;
    ensure!(
        !tags_in(moov)?.contains(&WalkTag(*b"udta")),
        "no udta among the children of moov"
    );
    ensure!(
        !tags_in(trak)?.contains(&WalkTag(*b"edts")),
        "no edts among the children of trak"
    );
    let mdhd = at(
        &bytes,
        &[
            WalkTag(*b"moov"),
            WalkTag(*b"trak"),
            WalkTag(*b"mdia"),
            WalkTag(*b"mdhd"),
        ],
    )?;
    ensure_eq!(u32_at(mdhd.0, ByteOffset(20))?, BigEndianWord(30000));
    Ok(())
}

#[test]
fn same_input_encodes_byte_identically_twice() {
    assert_eq!(check_same_input_encodes_byte_identically_twice(), Ok(()));
}

fn check_same_input_encodes_byte_identically_twice() -> Result<(), Fail> {
    let first = encode_seven_frames(distinct_30000_1001_over_24()?)?;
    let second = encode_seven_frames(distinct_30000_1001_over_24()?)?;
    ensure_eq!(first, second);
    Ok(())
}

// The ftyp box is 20 bytes, so the moov box begins at byte 20 and its first
// child, mvhd, at byte 28 (20 + the 8-byte moov header).
const MOOV_START: usize = 20;
const MVHD_START: usize = 28;

#[test]
fn stco_first_entry_points_at_the_first_frame_with_a_capture_rate() {
    assert_eq!(
        check_stco_first_entry_points_at_the_first_frame_with_a_capture_rate(),
        Ok(())
    );
}

fn check_stco_first_entry_points_at_the_first_frame_with_a_capture_rate() -> Result<(), Fail> {
    let bytes = encode_seven_frames(distinct_30000_1001_over_24()?)?;

    // Top level is ftyp (20 bytes), moov, then mdat.
    ensure_eq!(
        tags_in(iso(&bytes))?,
        vec![WalkTag(*b"ftyp"), WalkTag(*b"moov"), WalkTag(*b"mdat")]
    );

    // mdat starts where moov ends; its 8-byte header precedes the first frame.
    let moov_len = at(&bytes, &[WalkTag(*b"moov")])?.0 .0.len();
    let first_frame = MOOV_START
        .checked_add(moov_len)
        .and_then(|end| end.checked_add(8))
        .or_fail()?;
    let second_frame = first_frame.checked_add(45).or_fail()?;
    let after_second = first_frame.checked_add(90).or_fail()?;

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

    // The bytes there are the first frame (fill 0), then the second (fill 1).
    let frame0 = bytes.as_bytes().get(first_frame..second_frame).or_fail()?;
    let frame1 = bytes.as_bytes().get(second_frame..after_second).or_fail()?;
    ensure_eq!(frame0, vec![0_u8; 45].as_slice());
    ensure_eq!(frame1, vec![1_u8; 45].as_slice());
    Ok(())
}

mod defects;

mod quicktime;
