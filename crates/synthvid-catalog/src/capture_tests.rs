use crate::cover::generate_catalogue;
use synthvid_testkit::iso::{
    child_path, payload, tags_in, u32_at, BigEndianWord, BoxPath, BoxTag, ByteOffset, IsoBytes,
};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};
use synthvid_video::render;

/// The catalogue entry a check renders, by its manifest name.
struct EntryName(&'static str);

/// A rate as the manifest spells it, for the check to look for verbatim.
struct RateStatement(&'static str);

/// The two rate statements a manifest must contain.
struct RateStatements {
    playback: RateStatement,
    capture: RateStatement,
}

/// The mdhd timescale a rendered entry must carry.
struct Timescale(u32);

/// The big-endian version and flags, capture numerator and capture denominator
/// that the `cfps` payload must hold.
struct CfpsPayload([u8; 12]);

/// The facts a rendered catalogue entry is checked against.
struct Expected {
    name: EntryName,
    rates: RateStatements,
    timescale: Timescale,
    cfps: CfpsPayload,
}

/// Renders the catalogue entry named by `expected` and checks the manifest's two
/// rates, the media's timescale, and the `cfps` payload that carries the capture
/// rate.
///
/// The timescale is the playback numerator, because the track is timed at the
/// playback rate. The `cfps` payload is four bytes of version and flags, then the
/// capture numerator and denominator as big-endian `u32`s.
fn check_capture_entry_states_both_rates(expected: &Expected) -> Result<(), Fail> {
    let entry = generate_catalogue()
        .or_fail()?
        .into_iter()
        .find(|entry| entry.manifest_name().as_str() == expected.name.0)
        .or_fail()?;
    let generated = render(&entry.to_video_spec().or_fail()?).or_fail()?;

    let manifest = generated.manifest_json().as_str();
    ensure!(
        manifest.contains(expected.rates.playback.0),
        "manifest must state the playback rate {}",
        expected.rates.playback.0
    );
    ensure!(
        manifest.contains(expected.rates.capture.0),
        "manifest must state the capture rate {}",
        expected.rates.capture.0
    );

    let media = IsoBytes(generated.media_bytes().as_bytes());
    // The timescale is 20 bytes into the mdhd box: 8 header, 4 version and flags,
    // 4 creation time, 4 modification time.
    let mdhd = child_path(
        media,
        BoxPath(&[
            BoxTag(*b"moov"),
            BoxTag(*b"trak"),
            BoxTag(*b"mdia"),
            BoxTag(*b"mdhd"),
        ]),
    )?;
    ensure_eq!(
        u32_at(mdhd.0, ByteOffset(20))?,
        BigEndianWord(expected.timescale.0)
    );
    let trak = payload(child_path(
        media,
        BoxPath(&[BoxTag(*b"moov"), BoxTag(*b"trak")]),
    )?)?;
    ensure!(
        !tags_in(trak)?.contains(&BoxTag(*b"edts")),
        "a distinct capture rate must not write an edts box"
    );

    let cfps = child_path(
        media,
        BoxPath(&[BoxTag(*b"moov"), BoxTag(*b"udta"), BoxTag(*b"cfps")]),
    )?;
    ensure_eq!(payload(cfps)?, IsoBytes(&expected.cfps.0));
    Ok(())
}

// Hand-computed for the NTSC entry captured at 24000/1001: the playback rate is
// 30000/1001 (num 30000, den 1001) and the capture rate is 24000/1001 (num 24000,
// den 1001), so the manifest states both. The mdhd timescale is 30000. In the cfps
// payload, 24000 is 0x5DC0 and 1001 is 0x03E9.
fn check_ntsc_entry_captured_at_24000x1001_states_both_rates() -> Result<(), Fail> {
    check_capture_entry_states_both_rates(&Expected {
        name: EntryName(
            "0016x0016-10-ntsc-solid-disc-fixed-raw-iso-identity-unscaled-cap24000x1001-nodef",
        ),
        rates: RateStatements {
            playback: RateStatement(r#""frame_rate":{"den":1001,"num":30000}"#),
            capture: RateStatement(r#""capture_rate":{"den":1001,"num":24000}"#),
        },
        timescale: Timescale(30000),
        cfps: CfpsPayload([0, 0, 0, 0, 0, 0, 0x5D, 0xC0, 0, 0, 0x03, 0xE9]),
    })
}

#[test]
fn ntsc_entry_captured_at_24000x1001_states_both_rates() {
    assert_eq!(
        check_ntsc_entry_captured_at_24000x1001_states_both_rates(),
        Ok(())
    );
}

// Hand-computed for the 30 fps entry captured at 240/1: the playback rate is 30/1
// and the capture rate is 240/1, so the manifest states both. The mdhd timescale is
// 30. In the cfps payload, 240 is 0xF0 and the denominator is 1.
fn check_thirty_fps_entry_captured_at_240_states_both_rates() -> Result<(), Fail> {
    check_capture_entry_states_both_rates(&Expected {
        name: EntryName(
            "0016x0016-10-30fps-solid-disc-fixed-raw-iso-identity-unscaled-cap240-nodef",
        ),
        rates: RateStatements {
            playback: RateStatement(r#""frame_rate":{"den":1,"num":30}"#),
            capture: RateStatement(r#""capture_rate":{"den":1,"num":240}"#),
        },
        timescale: Timescale(30),
        cfps: CfpsPayload([0, 0, 0, 0, 0, 0, 0, 0xF0, 0, 0, 0, 1]),
    })
}

#[test]
fn thirty_fps_entry_captured_at_240_states_both_rates() {
    assert_eq!(
        check_thirty_fps_entry_captured_at_240_states_both_rates(),
        Ok(())
    );
}

/// The float32 payload of a `QuickTime` `data` box: type indicator 23, locale 0, then the
/// big-endian float32 bits of the capture rate.
struct DataPayload([u8; 12]);

/// The facts a `QuickTime`-carried catalogue entry is checked against.
struct QuickTimeExpected {
    name: EntryName,
    rates: RateStatements,
    timescale: Timescale,
    data: DataPayload,
}

/// Renders the `QuickTime`-carried entry named by `expected` and checks that its manifest
/// states both rates, that its timing is at the playback rate, that `moov` ends with `meta`
/// and holds no `udta`, and that the `meta` item carries the capture rate as a float32.
fn check_quicktime_entry(expected: &QuickTimeExpected) -> Result<(), Fail> {
    let entry = generate_catalogue()
        .or_fail()?
        .into_iter()
        .find(|entry| entry.manifest_name().as_str() == expected.name.0)
        .or_fail()?;
    let generated = render(&entry.to_video_spec().or_fail()?).or_fail()?;

    let manifest = generated.manifest_json().as_str();
    ensure!(
        manifest.contains(expected.rates.playback.0),
        "manifest must state the playback rate {}",
        expected.rates.playback.0
    );
    ensure!(
        manifest.contains(expected.rates.capture.0),
        "manifest must state the capture rate {}",
        expected.rates.capture.0
    );

    let media = IsoBytes(generated.media_bytes().as_bytes());
    let mdhd = child_path(
        media,
        BoxPath(&[
            BoxTag(*b"moov"),
            BoxTag(*b"trak"),
            BoxTag(*b"mdia"),
            BoxTag(*b"mdhd"),
        ]),
    )?;
    ensure_eq!(
        u32_at(mdhd.0, ByteOffset(20))?,
        BigEndianWord(expected.timescale.0)
    );

    let moov = payload(child_path(media, BoxPath(&[BoxTag(*b"moov")]))?)?;
    ensure_eq!(
        tags_in(moov)?,
        vec![BoxTag(*b"mvhd"), BoxTag(*b"trak"), BoxTag(*b"meta")],
        "moov ends with meta and holds no udta"
    );

    // meta is a full box, so its version and flags come before its children, which the
    // walker cannot tile from the start of the payload. The children begin 4 bytes in.
    let meta = payload(child_path(
        media,
        BoxPath(&[BoxTag(*b"moov"), BoxTag(*b"meta")]),
    )?)?;
    let meta_children = IsoBytes(meta.0.get(4..).or_fail()?);
    let data = child_path(
        meta_children,
        BoxPath(&[BoxTag(*b"ilst"), BoxTag([0, 0, 0, 1]), BoxTag(*b"data")]),
    )?;
    ensure_eq!(payload(data)?, IsoBytes(&expected.data.0));
    Ok(())
}

// Hand-computed for the 30 fps entry captured at 240/1 in the QuickTime box: 240 is
// 1.875 * 2^7, so the float32 is sign 0, biased exponent 134 (0x86) and fraction 0.875
// (0x700000), which is 0x43700000. The indicator 23 is 0x17, the locale is 0.
fn check_thirty_fps_entry_captured_at_240_in_quicktime_meta() -> Result<(), Fail> {
    check_quicktime_entry(&QuickTimeExpected {
        name: EntryName(
            "0016x0016-10-30fps-solid-disc-fixed-raw-iso-identity-unscaled-qtcap240-nodef",
        ),
        rates: RateStatements {
            playback: RateStatement(r#""frame_rate":{"den":1,"num":30}"#),
            capture: RateStatement(r#""capture_rate":{"den":1,"num":240}"#),
        },
        timescale: Timescale(30),
        data: DataPayload([0, 0, 0, 23, 0, 0, 0, 0, 0x43, 0x70, 0x00, 0x00]),
    })
}

#[test]
fn thirty_fps_entry_captured_at_240_in_quicktime_meta() {
    assert_eq!(
        check_thirty_fps_entry_captured_at_240_in_quicktime_meta(),
        Ok(())
    );
}

// Hand-computed for the NTSC entry captured at 24000/1001 in the QuickTime box: the
// value is 23.976023976..., which lies in [16, 32), so the biased exponent is 131 (0x83).
// Its significand 1.4985014985... rounds to 0x3FCEE6 in 23 bits, giving 0x41BFCEE6.
// float32 cannot hold 24000/1001 exactly, so this payload is the nearest float only.
fn check_ntsc_entry_captured_at_24000x1001_in_quicktime_meta() -> Result<(), Fail> {
    check_quicktime_entry(&QuickTimeExpected {
        name: EntryName(
            "0016x0016-10-ntsc-solid-disc-fixed-raw-iso-identity-unscaled-qtcap24000x1001-nodef",
        ),
        rates: RateStatements {
            playback: RateStatement(r#""frame_rate":{"den":1001,"num":30000}"#),
            capture: RateStatement(r#""capture_rate":{"den":1001,"num":24000}"#),
        },
        timescale: Timescale(30000),
        data: DataPayload([0, 0, 0, 23, 0, 0, 0, 0, 0x41, 0xBF, 0xCE, 0xE6]),
    })
}

#[test]
fn ntsc_entry_captured_at_24000x1001_in_quicktime_meta() {
    assert_eq!(
        check_ntsc_entry_captured_at_24000x1001_in_quicktime_meta(),
        Ok(())
    );
}

// Hand-computed for the 30 fps entry captured at 12/1: the playback rate is 30/1 and the
// capture rate is 12/1 (already reduced), so the manifest states both. The mdhd timescale
// is 30. In the cfps payload, 12 is 0x0C and the denominator is 1.
fn check_thirty_fps_entry_captured_at_12_states_both_rates() -> Result<(), Fail> {
    check_capture_entry_states_both_rates(&Expected {
        name: EntryName(
            "0016x0016-10-30fps-solid-disc-fixed-raw-iso-identity-unscaled-cap12-nodef",
        ),
        rates: RateStatements {
            playback: RateStatement(r#""frame_rate":{"den":1,"num":30}"#),
            capture: RateStatement(r#""capture_rate":{"den":1,"num":12}"#),
        },
        timescale: Timescale(30),
        cfps: CfpsPayload([0, 0, 0, 0, 0, 0, 0, 0x0C, 0, 0, 0, 1]),
    })
}

#[test]
fn thirty_fps_entry_captured_at_12_states_both_rates() {
    assert_eq!(
        check_thirty_fps_entry_captured_at_12_states_both_rates(),
        Ok(())
    );
}

// Hand-computed for the NTSC entry captured at 25/1: the playback rate is 30000/1001 and
// the capture rate is 25/1, so the manifest states both. The mdhd timescale is 30000.
// In the cfps payload, 25 is 0x19 and the denominator is 1.
fn check_ntsc_entry_captured_at_25_states_both_rates() -> Result<(), Fail> {
    check_capture_entry_states_both_rates(&Expected {
        name: EntryName("0016x0016-10-ntsc-solid-disc-fixed-raw-iso-identity-unscaled-cap25-nodef"),
        rates: RateStatements {
            playback: RateStatement(r#""frame_rate":{"den":1001,"num":30000}"#),
            capture: RateStatement(r#""capture_rate":{"den":1,"num":25}"#),
        },
        timescale: Timescale(30000),
        cfps: CfpsPayload([0, 0, 0, 0, 0, 0, 0, 0x19, 0, 0, 0, 1]),
    })
}

#[test]
fn ntsc_entry_captured_at_25_states_both_rates() {
    assert_eq!(check_ntsc_entry_captured_at_25_states_both_rates(), Ok(()));
}

// Hand-computed for the NTSC entry captured at 1000/1: the playback rate is 30000/1001
// and the capture rate is 1000/1, so the manifest states both. The mdhd timescale is
// 30000. In the cfps payload, 1000 is 0x03E8 (3 * 256 + 232) and the denominator is 1.
fn check_ntsc_entry_captured_at_1000_states_both_rates() -> Result<(), Fail> {
    check_capture_entry_states_both_rates(&Expected {
        name: EntryName(
            "0016x0016-10-ntsc-solid-disc-fixed-raw-iso-identity-unscaled-cap1000-nodef",
        ),
        rates: RateStatements {
            playback: RateStatement(r#""frame_rate":{"den":1001,"num":30000}"#),
            capture: RateStatement(r#""capture_rate":{"den":1,"num":1000}"#),
        },
        timescale: Timescale(30000),
        cfps: CfpsPayload([0, 0, 0, 0, 0, 0, 0x03, 0xE8, 0, 0, 0, 1]),
    })
}

#[test]
fn ntsc_entry_captured_at_1000_states_both_rates() {
    assert_eq!(
        check_ntsc_entry_captured_at_1000_states_both_rates(),
        Ok(())
    );
}

// Hand-computed for the 30 fps entry captured at 120000/1001: 120000 and 1001 share no
// common factor (1001 = 7 * 11 * 13, 120000 = 2^6 * 3 * 5^4), so the fraction is already
// reduced. The mdhd timescale is 30. In the cfps payload, 120000 is 0x0001D4C0
// (1 * 65536 + 54464) and 1001 is 0x000003E9 (3 * 256 + 233).
fn check_thirty_fps_entry_captured_at_120000x1001_states_both_rates() -> Result<(), Fail> {
    check_capture_entry_states_both_rates(&Expected {
        name: EntryName(
            "0016x0016-10-30fps-solid-disc-fixed-raw-iso-identity-unscaled-cap120000x1001-nodef",
        ),
        rates: RateStatements {
            playback: RateStatement(r#""frame_rate":{"den":1,"num":30}"#),
            capture: RateStatement(r#""capture_rate":{"den":1001,"num":120000}"#),
        },
        timescale: Timescale(30),
        cfps: CfpsPayload([0, 0, 0, 0, 0x00, 0x01, 0xD4, 0xC0, 0, 0, 0x03, 0xE9]),
    })
}

#[test]
fn thirty_fps_entry_captured_at_120000x1001_states_both_rates() {
    assert_eq!(
        check_thirty_fps_entry_captured_at_120000x1001_states_both_rates(),
        Ok(())
    );
}

// Hand-computed for the 30 fps entry captured at 12/1 in the QuickTime box: 12 is 1.5 * 2^3,
// so the float32 is sign 0, biased exponent 127 + 3 = 130 (0x82) and fraction 0.5 (0x400000).
// The word is 0x41400000. The indicator 23 is 0x17, the locale is 0.
fn check_thirty_fps_entry_captured_at_12_in_quicktime_meta() -> Result<(), Fail> {
    check_quicktime_entry(&QuickTimeExpected {
        name: EntryName(
            "0016x0016-10-30fps-solid-disc-fixed-raw-iso-identity-unscaled-qtcap12-nodef",
        ),
        rates: RateStatements {
            playback: RateStatement(r#""frame_rate":{"den":1,"num":30}"#),
            capture: RateStatement(r#""capture_rate":{"den":1,"num":12}"#),
        },
        timescale: Timescale(30),
        data: DataPayload([0, 0, 0, 23, 0, 0, 0, 0, 0x41, 0x40, 0x00, 0x00]),
    })
}

#[test]
fn thirty_fps_entry_captured_at_12_in_quicktime_meta() {
    assert_eq!(
        check_thirty_fps_entry_captured_at_12_in_quicktime_meta(),
        Ok(())
    );
}
