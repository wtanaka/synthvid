use crate::manifest::{Manifest, ManifestAffine, ManifestFrame, ManifestHeader};
use crate::names::BackgroundName;
use crate::testutil::Frac;
use crate::writer::JsonEntryName;
use synthvid_scene::{
    Affine, CaptureCarrier, Dimensions, FrameRate, FrameRates, Height, RatePair, Width,
};
use synthvid_testkit::{ensure, Fail, OrFail as _};

/// A serialised manifest, as the writer emits it.
struct ManifestText(String);

fn identity_camera() -> Result<ManifestAffine, Fail> {
    Ok(ManifestAffine::from_affine(&Affine {
        xx: Frac(1, 1).ratio()?,
        xy: Frac(0, 1).ratio()?,
        tx: Frac(0, 1).ratio()?,
        yx: Frac(0, 1).ratio()?,
        yy: Frac(1, 1).ratio()?,
        ty: Frac(0, 1).ratio()?,
    }))
}

/// Serializes a one-frame manifest with an empty scene around the given header.
fn one_frame_json(header: ManifestHeader) -> Result<ManifestText, Fail> {
    let frame = ManifestFrame::new(&identity_camera()?, Vec::new());
    let manifest = Manifest::new(header, Vec::new(), vec![frame]).or_fail()?;
    Ok(ManifestText(
        manifest.to_canonical_json().as_str().to_owned(),
    ))
}

fn ntsc_header(extra: Option<FrameRate>) -> Result<ManifestHeader, Fail> {
    let playback = FrameRate::new(Frac(30000, 1001).ratio()?).or_fail()?;
    let frame_rates = match extra {
        Some(capture) => {
            FrameRates::distinct_in(RatePair { playback, capture }, CaptureCarrier::ProjectBox)
                .or_fail()?
        }
        None => FrameRates::playback_only(playback),
    };
    Ok(ManifestHeader::new(
        JsonEntryName::new("capture-keys").or_fail()?,
        Dimensions::new(Width::new(16).or_fail()?, Height::new(16).or_fail()?),
        frame_rates,
        BackgroundName::Solid,
        None,
    ))
}

fn check_capture_rate_key_order() -> Result<(), Fail> {
    let capture = FrameRate::new(Frac(24, 1).ratio()?).or_fail()?;
    let text = one_frame_json(ntsc_header(Some(capture))?)?.0;
    ensure!(
        text.contains(r#""frame_rate":{"den":1001,"num":30000}"#),
        "playback rate must be emitted as frame_rate: {text}"
    );
    ensure!(
        text.contains(r#""capture_rate":{"den":1,"num":24}"#),
        "capture rate must be emitted as capture_rate: {text}"
    );
    // Top-level keys sort by UTF-8 bytes: background, capture_rate, dimensions,
    // frame_count, frame_rate.
    let mut last = 0;
    for key in [
        "\"background\"",
        "\"capture_rate\"",
        "\"dimensions\"",
        "\"frame_count\"",
        "\"frame_rate\"",
    ] {
        let at = text.find(key).or_fail()?;
        ensure!(at > last, "{key} must sort after the key before it: {text}");
        last = at;
    }
    Ok(())
}

#[test]
fn capture_rate_key_order() {
    assert_eq!(check_capture_rate_key_order(), Ok(()));
}

fn check_playback_only_has_no_capture_key() -> Result<(), Fail> {
    let text = one_frame_json(ntsc_header(None)?)?.0;
    ensure!(
        !text.contains("capture_rate"),
        "a playback-only manifest must not carry capture_rate: {text}"
    );
    ensure!(
        text.contains(r#""frame_rate":{"den":1001,"num":30000}"#),
        "playback rate must still be emitted: {text}"
    );
    Ok(())
}

#[test]
fn playback_only_has_no_capture_key() {
    assert_eq!(check_playback_only_has_no_capture_key(), Ok(()));
}

/// The NTSC header, with a distinct capture rate of 24/1 carried in `carrier`.
fn ntsc_header_in(carrier: CaptureCarrier) -> Result<ManifestHeader, Fail> {
    let playback = FrameRate::new(Frac(30000, 1001).ratio()?).or_fail()?;
    let capture = FrameRate::new(Frac(24, 1).ratio()?).or_fail()?;
    let frame_rates = FrameRates::distinct_in(RatePair { playback, capture }, carrier).or_fail()?;
    Ok(ManifestHeader::new(
        JsonEntryName::new("capture-keys").or_fail()?,
        Dimensions::new(Width::new(16).or_fail()?, Height::new(16).or_fail()?),
        frame_rates,
        BackgroundName::Solid,
        None,
    ))
}

fn check_project_box_capture_is_carried_as_cfps() -> Result<(), Fail> {
    let text = one_frame_json(ntsc_header_in(CaptureCarrier::ProjectBox)?)?.0;
    ensure!(
        text.contains(r#""capture_carrier":"cfps""#),
        "a project-box capture must name cfps as its carrier: {text}"
    );
    Ok(())
}

#[test]
fn project_box_capture_is_carried_as_cfps() {
    assert_eq!(check_project_box_capture_is_carried_as_cfps(), Ok(()));
}

fn check_quicktime_capture_is_carried_as_meta() -> Result<(), Fail> {
    let text = one_frame_json(ntsc_header_in(CaptureCarrier::QuickTimeMeta)?)?.0;
    ensure!(
        text.contains(r#""capture_carrier":"meta""#),
        "a QuickTime capture must name meta as its carrier: {text}"
    );
    Ok(())
}

#[test]
fn quicktime_capture_is_carried_as_meta() {
    assert_eq!(check_quicktime_capture_is_carried_as_meta(), Ok(()));
}

fn check_capture_carrier_key_order() -> Result<(), Fail> {
    let text = one_frame_json(ntsc_header_in(CaptureCarrier::ProjectBox)?)?.0;
    // Keys sort by UTF-8 bytes, so "capture_carrier" precedes "capture_rate": the
    // carrier is emitted beside the rate, not after it.
    let background = text.find("\"background\"").or_fail()?;
    let carrier = text.find("\"capture_carrier\"").or_fail()?;
    let rate = text.find("\"capture_rate\"").or_fail()?;
    let dimensions = text.find("\"dimensions\"").or_fail()?;
    ensure!(
        background < carrier && carrier < rate && rate < dimensions,
        "capture_carrier must sort between background and capture_rate: {text}"
    );
    Ok(())
}

#[test]
fn capture_carrier_key_order() {
    assert_eq!(check_capture_carrier_key_order(), Ok(()));
}

fn check_playback_only_has_no_carrier_key() -> Result<(), Fail> {
    let text = one_frame_json(ntsc_header(None)?)?.0;
    ensure!(
        !text.contains("capture_carrier"),
        "a playback-only manifest must not carry capture_carrier: {text}"
    );
    Ok(())
}

#[test]
fn playback_only_has_no_carrier_key() {
    assert_eq!(check_playback_only_has_no_carrier_key(), Ok(()));
}
