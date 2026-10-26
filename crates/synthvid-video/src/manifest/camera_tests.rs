use core::num::NonZeroU16;

use crate::json_names::JsonEntryName;
use crate::manifest::{Manifest, ManifestAffine, ManifestCamera, ManifestFrame, ManifestHeader};
use crate::medium::{
    ColumnSigma, FilmGrain, GrainBlur, GrainDensity, GrainRadius, GrainSeed, GrainSpread, Medium,
    PatternSeed, PrnuSpread, ReadSigma, SensorNoise, SensorSeeds, ShotGain, TemporalSeed, Q8_8,
};
use crate::names::BackgroundName;
use crate::optics::{Optics, VignetteStrength};
use crate::testutil::Frac;
use core::fmt;
use synthvid_scene::{Affine, Dimensions, FrameRate, FrameRates, Height, Seed, Width};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// The key an object sits under in the header.
#[derive(Copy, Clone)]
struct MemberKey(pub &'static str);

/// An object's canonical JSON text.
#[derive(Copy, Clone)]
struct MemberObject(pub &'static str);

/// The serialized text of a manifest.
#[derive(Debug, Eq, PartialEq)]
struct ManifestText(pub String);

impl fmt::Display for ManifestText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A key of the header and the object written under it, as the manifest must spell them.
#[derive(Copy, Clone)]
struct Member {
    /// The key the object sits under.
    key: MemberKey,
    /// The object's canonical JSON text.
    object: MemberObject,
}

/// Serializes a one-frame manifest with an empty scene, recording on `medium` through `optics`.
fn manifest_text(medium: Medium, optics: Optics) -> Result<ManifestText, Fail> {
    let camera = ManifestCamera::new(
        Dimensions::new(Width::new(16).or_fail()?, Height::new(16).or_fail()?),
        medium,
        optics,
    );
    let header = ManifestHeader::new(
        JsonEntryName::new("camera-keys").or_fail()?,
        &camera,
        FrameRates::playback_only(FrameRate::new(Frac(30, 1).ratio()?).or_fail()?),
        BackgroundName::Solid,
        None,
    );
    let view = ManifestAffine::from_affine(&Affine {
        xx: Frac(1, 1).ratio()?,
        xy: Frac(0, 1).ratio()?,
        tx: Frac(0, 1).ratio()?,
        yx: Frac(0, 1).ratio()?,
        yy: Frac(1, 1).ratio()?,
        ty: Frac(0, 1).ratio()?,
    });
    let frame = ManifestFrame::new(&view, Vec::new());
    let manifest = Manifest::new(header, Vec::new(), vec![frame]).or_fail()?;
    Ok(ManifestText(
        manifest.to_canonical_json().as_str().to_owned(),
    ))
}

/// Sensor noise with awkward raw terms and a temporal seed beyond the range of a 53-bit float.
fn sensor() -> Result<Medium, Fail> {
    let seeds = SensorSeeds::new(
        TemporalSeed::new(Seed::new(18_446_744_073_709_551_557)),
        PatternSeed::new(Seed::new(7)),
    );
    Ok(Medium::Sensor(
        SensorNoise::new(
            ShotGain(Q8_8(77)),
            ReadSigma(Q8_8(1237)),
            PrnuSpread(655),
            ColumnSigma(Q8_8(384)),
            seeds,
        )
        .or_fail()?,
    ))
}

/// Film grain with a radius of 3/700 of the frame height.
fn film() -> Result<Medium, Fail> {
    let radius = GrainRadius::new(Frac(3, 700).ratio()?).or_fail()?;
    Ok(Medium::Film(FilmGrain::new(
        radius,
        GrainSpread(Q8_8(77)),
        GrainDensity::new(NonZeroU16::new(300).or_fail()?).or_fail()?,
        GrainBlur(Q8_8(128)),
        GrainSeed::new(Seed::new(99)),
    )))
}

/// A flat camera's vignette with a raw strength of 128, which is one half of full strength.
fn vignette() -> Result<Optics, Fail> {
    let strength = VignetteStrength::new(NonZeroU16::new(128).or_fail()?).or_fail()?;
    Ok(Optics::Vignette(strength))
}

/// Fails unless the manifest text carries the member's object under its key.
fn expect_member(text: &ManifestText, member: Member) -> Result<(), Fail> {
    let MemberKey(key) = member.key;
    let MemberObject(object) = member.object;
    let ManifestText(body) = text;
    let needle = format!(r#""{key}":{object}"#);
    ensure!(
        body.contains(needle.as_str()),
        "manifest must carry {needle}: {body}"
    );
    Ok(())
}

fn check_sensor_medium_is_written_in_full() -> Result<(), Fail> {
    let text = manifest_text(sensor()?, Optics::Off)?;
    expect_member(
        &text,
        Member {
            key: MemberKey("medium"),
            object: MemberObject(
                r#"{"column":384,"kind":"sensor","pattern_seed":"7","prnu":655,"read":1237,"shot":77,"temporal_seed":"18446744073709551557"}"#,
            ),
        },
    )?;
    ensure!(
        !text.0.contains("\"optics\""),
        "an Off optics must add no key: {text}"
    );
    Ok(())
}

#[test]
fn sensor_medium_is_written_in_full() {
    assert_eq!(check_sensor_medium_is_written_in_full(), Ok(()));
}

fn check_film_medium_is_written_in_full() -> Result<(), Fail> {
    let text = manifest_text(film()?, Optics::Off)?;
    expect_member(
        &text,
        Member {
            key: MemberKey("medium"),
            object: MemberObject(
                r#"{"blur":128,"density":300,"kind":"film","radius":{"den":700,"num":3},"seed":"99","spread":77}"#,
            ),
        },
    )?;
    ensure!(
        !text.0.contains("\"optics\""),
        "an Off optics must add no key: {text}"
    );
    Ok(())
}

#[test]
fn film_medium_is_written_in_full() {
    assert_eq!(check_film_medium_is_written_in_full(), Ok(()));
}

fn check_vignette_optics_is_written_in_full() -> Result<(), Fail> {
    let text = manifest_text(Medium::Off, vignette()?)?;
    expect_member(
        &text,
        Member {
            key: MemberKey("optics"),
            object: MemberObject(r#"{"kind":"vignette","strength":128}"#),
        },
    )?;
    ensure!(
        !text.0.contains("\"medium\""),
        "an Off medium must add no key: {text}"
    );
    Ok(())
}

#[test]
fn vignette_optics_is_written_in_full() {
    assert_eq!(check_vignette_optics_is_written_in_full(), Ok(()));
}

fn check_both_parts_are_written_together() -> Result<(), Fail> {
    let text = manifest_text(film()?, vignette()?)?;
    expect_member(
        &text,
        Member {
            key: MemberKey("medium"),
            object: MemberObject(
                r#"{"blur":128,"density":300,"kind":"film","radius":{"den":700,"num":3},"seed":"99","spread":77}"#,
            ),
        },
    )?;
    expect_member(
        &text,
        Member {
            key: MemberKey("optics"),
            object: MemberObject(r#"{"kind":"vignette","strength":128}"#),
        },
    )?;
    Ok(())
}

#[test]
fn both_parts_are_written_together() {
    assert_eq!(check_both_parts_are_written_together(), Ok(()));
}

fn check_off_camera_has_neither_key() -> Result<(), Fail> {
    let text = manifest_text(Medium::Off, Optics::Off)?;
    ensure_eq!(
        text,
        ManifestText(String::from(
            r#"{"background":"solid","dimensions":{"height":16,"width":16},"frame_count":1,"frame_rate":{"den":1,"num":30},"frames":[{"camera":{"a":{"den":1,"num":1},"b":{"den":1,"num":0},"c":{"den":1,"num":0},"d":{"den":1,"num":1},"tx":{"den":1,"num":0},"ty":{"den":1,"num":0}},"index":0,"objects":[]}],"name":"camera-keys","objects":[],"schema_version":1}"#
        ),),
        "an Off medium and an Off optics must serialize exactly as a camera without them"
    );
    Ok(())
}

#[test]
fn off_camera_has_neither_key() {
    assert_eq!(check_off_camera_has_neither_key(), Ok(()));
}

fn check_camera_manifest_is_deterministic() -> Result<(), Fail> {
    let first = manifest_text(film()?, vignette()?)?;
    let second = manifest_text(film()?, vignette()?)?;
    ensure_eq!(
        first,
        second,
        "identical cameras must produce identical manifests"
    );
    Ok(())
}

#[test]
fn camera_manifest_is_deterministic() {
    assert_eq!(check_camera_manifest_is_deterministic(), Ok(()));
}
