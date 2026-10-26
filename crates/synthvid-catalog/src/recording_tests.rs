//! Tests for the recording singletons, which record through a sensor or film medium.

extern crate alloc;

use crate::cover::{generate_catalogue, Entry, RecordingAxis};
use alloc::collections::BTreeSet;
use core::num::{NonZeroI64, NonZeroU16};
use synthvid_scene::{Numerator, Ratio, Seed};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};
use synthvid_video::medium::{PatternSeed, SensorSeeds, TemporalSeed};
use synthvid_video::{
    render, shot_sigma, Amplitude, ColumnSigma, FilmGrain, GrainBlur, GrainDensity, GrainRadius,
    GrainSeed, GrainSpread, Light, Medium, PrnuSpread, ReadSigma, SensorNoise, ShotGain, Q8_8,
};

/// The number of entries before the recording singletons: the covering array and the capture
/// singletons, which come first in the catalog in that order.
const PREVIOUS_ENTRY_COUNT: usize = 138;

/// The manifest names of the ten recording singletons, in generation order: each non-ideal
/// preset with raw pixels, then with motion-JPEG.
const RECORDING_NAMES: [&str; 10] = [
    "0064x0048-10-30fps-gradient-disc-fixed-raw-iso-identity-unscaled-nodef-reclowiso",
    "0064x0048-10-30fps-gradient-disc-fixed-mjpeg-iso-identity-unscaled-nodef-reclowiso",
    "0064x0048-10-30fps-gradient-disc-fixed-raw-iso-identity-unscaled-nodef-rechighiso",
    "0064x0048-10-30fps-gradient-disc-fixed-mjpeg-iso-identity-unscaled-nodef-rechighiso",
    "0064x0048-10-30fps-gradient-disc-fixed-raw-iso-identity-unscaled-nodef-recbanded",
    "0064x0048-10-30fps-gradient-disc-fixed-mjpeg-iso-identity-unscaled-nodef-recbanded",
    "0064x0048-10-30fps-gradient-disc-fixed-raw-iso-identity-unscaled-nodef-recfinegrain",
    "0064x0048-10-30fps-gradient-disc-fixed-mjpeg-iso-identity-unscaled-nodef-recfinegrain",
    "0064x0048-10-30fps-gradient-disc-fixed-raw-iso-identity-unscaled-nodef-reccoarsegrain",
    "0064x0048-10-30fps-gradient-disc-fixed-mjpeg-iso-identity-unscaled-nodef-reccoarsegrain",
];

/// The recording segments an ideal entry's name must never contain.
const RECORDING_SEGMENTS: [&str; 5] = [
    "-reclowiso",
    "-rechighiso",
    "-recbanded",
    "-recfinegrain",
    "-reccoarsegrain",
];

/// The entries that record through a sensor or film medium.
fn recording_entries() -> Result<Vec<Entry>, Fail> {
    let catalogue = generate_catalogue().or_fail()?;
    Ok(catalogue
        .into_iter()
        .filter(|entry| entry.recording() != RecordingAxis::Ideal)
        .collect())
}

/// The sensor noise the low-sensitivity preset is specified to have, written out by hand.
fn low_iso_noise() -> Result<SensorNoise, Fail> {
    SensorNoise::new(
        ShotGain(Q8_8(18)),
        ReadSigma(Q8_8(128)),
        PrnuSpread(0),
        ColumnSigma(Q8_8(0)),
        SensorSeeds::new(
            TemporalSeed::new(Seed::new(1001)),
            PatternSeed::new(Seed::new(2001)),
        ),
    )
    .or_fail()
}

/// The sensor noise the high-sensitivity preset is specified to have, written out by hand.
fn high_iso_noise() -> Result<SensorNoise, Fail> {
    SensorNoise::new(
        ShotGain(Q8_8(128)),
        ReadSigma(Q8_8(640)),
        PrnuSpread(0),
        ColumnSigma(Q8_8(0)),
        SensorSeeds::new(
            TemporalSeed::new(Seed::new(1002)),
            PatternSeed::new(Seed::new(2002)),
        ),
    )
    .or_fail()
}

/// The sensor noise the banded preset is specified to have, written out by hand.
fn banded_noise() -> Result<SensorNoise, Fail> {
    SensorNoise::new(
        ShotGain(Q8_8(18)),
        ReadSigma(Q8_8(128)),
        PrnuSpread(655),
        ColumnSigma(Q8_8(384)),
        SensorSeeds::new(
            TemporalSeed::new(Seed::new(1003)),
            PatternSeed::new(Seed::new(2003)),
        ),
    )
    .or_fail()
}

/// The film grain the fine-grain preset is specified to have, written out by hand.
fn fine_grain_expected() -> Result<FilmGrain, Fail> {
    let radius = Ratio::new(Numerator(1), NonZeroI64::new(400).or_fail()?).or_fail()?;
    Ok(FilmGrain::new(
        GrainRadius::new(radius).or_fail()?,
        GrainSpread(Q8_8(0)),
        GrainDensity::new(NonZeroU16::new(256).or_fail()?).or_fail()?,
        GrainBlur(Q8_8(0)),
        GrainSeed::new(Seed::new(3001)),
    ))
}

/// The film grain the coarse-grain preset is specified to have, written out by hand.
fn coarse_grain_expected() -> Result<FilmGrain, Fail> {
    let radius = Ratio::new(Numerator(1), NonZeroI64::new(16).or_fail()?).or_fail()?;
    Ok(FilmGrain::new(
        GrainRadius::new(radius).or_fail()?,
        GrainSpread(Q8_8(77)),
        GrainDensity::new(NonZeroU16::new(256).or_fail()?).or_fail()?,
        GrainBlur(Q8_8(128)),
        GrainSeed::new(Seed::new(3002)),
    ))
}

fn check_previous_entries_come_first_and_stay_ideal() -> Result<(), Fail> {
    let catalogue = generate_catalogue().or_fail()?;
    let previous = catalogue.get(..PREVIOUS_ENTRY_COUNT).or_fail()?;
    ensure_eq!(previous.len(), PREVIOUS_ENTRY_COUNT);
    for entry in previous {
        ensure_eq!(
            entry.recording(),
            RecordingAxis::Ideal,
            "{}",
            entry.manifest_name()
        );
    }
    let added = catalogue
        .get(PREVIOUS_ENTRY_COUNT..)
        .or_fail()?
        .get(..RECORDING_NAMES.len())
        .or_fail()?;
    let names: Vec<String> = added
        .iter()
        .map(|entry| entry.manifest_name().as_str().to_owned())
        .collect();
    let expected: Vec<String> = RECORDING_NAMES
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    ensure_eq!(
        names,
        expected,
        "the recording singletons follow the others"
    );
    ensure_eq!(catalogue.len(), 152);
    Ok(())
}

#[test]
fn previous_entries_come_first_and_stay_ideal() {
    assert_eq!(check_previous_entries_come_first_and_stay_ideal(), Ok(()));
}

fn check_recording_names_are_unique_and_exact() -> Result<(), Fail> {
    let catalogue = generate_catalogue().or_fail()?;
    let mut names = BTreeSet::new();
    for entry in &catalogue {
        let name = entry.manifest_name();
        ensure!(
            names.insert(name.as_str().to_owned()),
            "entry names must be unique: {name}"
        );
    }
    ensure_eq!(recording_entries()?.len(), 10);
    Ok(())
}

#[test]
fn recording_names_are_unique_and_exact() {
    assert_eq!(check_recording_names_are_unique_and_exact(), Ok(()));
}

fn check_recording_name_segments_follow_the_scheme() -> Result<(), Fail> {
    let entry = recording_entries()?
        .into_iter()
        .find(|entry| entry.recording() == RecordingAxis::LowIso)
        .or_fail()?;
    let name = entry.manifest_name().as_str().to_owned();
    let segments: Vec<&str> = name.split('-').collect();
    ensure_eq!(
        segments,
        vec![
            "0064x0048",
            "10",
            "30fps",
            "gradient",
            "disc",
            "fixed",
            "raw",
            "iso",
            "identity",
            "unscaled",
            "nodef",
            "reclowiso",
        ]
    );
    Ok(())
}

#[test]
fn recording_name_segments_follow_the_scheme() {
    assert_eq!(check_recording_name_segments_follow_the_scheme(), Ok(()));
}

fn check_ideal_names_carry_no_recording_segment() -> Result<(), Fail> {
    let catalogue = generate_catalogue().or_fail()?;
    for entry in catalogue
        .iter()
        .filter(|entry| entry.recording() == RecordingAxis::Ideal)
    {
        let name = entry.manifest_name();
        for segment in RECORDING_SEGMENTS {
            ensure!(
                !name.as_str().contains(segment),
                "ideal entry {name} must not carry {segment}"
            );
        }
    }
    Ok(())
}

#[test]
fn ideal_names_carry_no_recording_segment() {
    assert_eq!(check_ideal_names_carry_no_recording_segment(), Ok(()));
}

fn check_medium_matches_preset(entry: &Entry) -> Result<(), Fail> {
    let spec = entry.to_video_spec().or_fail()?;
    match spec.recorder().medium() {
        Medium::Off => ensure_eq!(entry.recording(), RecordingAxis::Ideal),
        Medium::Sensor(noise) => match entry.recording() {
            RecordingAxis::LowIso => ensure_eq!(noise, low_iso_noise()?),
            RecordingAxis::HighIso => ensure_eq!(noise, high_iso_noise()?),
            RecordingAxis::Banded => ensure_eq!(noise, banded_noise()?),
            RecordingAxis::Ideal | RecordingAxis::FineGrain | RecordingAxis::CoarseGrain => {
                return Err(Fail::here(format_args!(
                    "{:?} must not record through a sensor",
                    entry.recording()
                )));
            }
        },
        Medium::Film(grain) => match entry.recording() {
            RecordingAxis::FineGrain => ensure_eq!(grain, fine_grain_expected()?),
            RecordingAxis::CoarseGrain => ensure_eq!(grain, coarse_grain_expected()?),
            RecordingAxis::Ideal
            | RecordingAxis::LowIso
            | RecordingAxis::HighIso
            | RecordingAxis::Banded => {
                return Err(Fail::here(format_args!(
                    "{:?} must not record through film",
                    entry.recording()
                )));
            }
        },
    }
    Ok(())
}

fn check_recording_entries_build_their_medium() -> Result<(), Fail> {
    for entry in recording_entries()? {
        check_medium_matches_preset(&entry)?;
    }
    Ok(())
}

#[test]
fn recording_entries_build_their_medium() {
    assert_eq!(check_recording_entries_build_their_medium(), Ok(()));
}

fn check_recording_changes_the_media_bytes() -> Result<(), Fail> {
    for entry in recording_entries()? {
        let mut ideal = entry.clone();
        ideal.set_recording(RecordingAxis::Ideal);
        let recorded = render(&entry.to_video_spec().or_fail()?).or_fail()?;
        let plain = render(&ideal.to_video_spec().or_fail()?).or_fail()?;
        ensure!(
            recorded.media_bytes().as_bytes() != plain.media_bytes().as_bytes(),
            "recording must change the media of {}",
            entry.manifest_name()
        );
    }
    Ok(())
}

#[test]
fn recording_changes_the_media_bytes() {
    assert_eq!(check_recording_changes_the_media_bytes(), Ok(()));
}

fn check_recording_entries_render_identically_twice() -> Result<(), Fail> {
    for entry in recording_entries()? {
        let spec = entry.to_video_spec().or_fail()?;
        let first = render(&spec).or_fail()?;
        let second = render(&spec).or_fail()?;
        ensure_eq!(
            first.manifest_json().as_str(),
            second.manifest_json().as_str(),
            "manifests of {}",
            entry.manifest_name()
        );
        ensure_eq!(
            first.media_bytes().as_bytes(),
            second.media_bytes().as_bytes(),
            "media of {}",
            entry.manifest_name()
        );
    }
    Ok(())
}

#[test]
fn recording_entries_render_identically_twice() {
    assert_eq!(check_recording_entries_render_identically_twice(), Ok(()));
}

/// The shot-noise deviation of a sensor's preset at a mid-gray light of 128 code values.
///
/// The raw deviation is the integer square root of `slope_raw * 128 * 256`. For the low preset
/// the slope is 18 raw, so the product is 18 * 128 * 256 = 589824 = 768^2, and the deviation is
/// 768 raw, which is 3.0 code values. For the high preset the slope is 128 raw, so the product is
/// 128 * 128 * 256 = 4194304 = 2048^2, and the deviation is 2048 raw, which is 8.0 code values.
fn check_shot_noise_presets_are_three_and_eight_code_values_at_mid_gray() -> Result<(), Fail> {
    let light = Light(128);
    ensure_eq!(
        shot_sigma(low_iso_noise()?.shot(), light).or_fail()?,
        Amplitude(768)
    );
    ensure_eq!(
        shot_sigma(banded_noise()?.shot(), light).or_fail()?,
        Amplitude(768)
    );
    ensure_eq!(
        shot_sigma(high_iso_noise()?.shot(), light).or_fail()?,
        Amplitude(2048)
    );
    Ok(())
}

#[test]
fn shot_noise_presets_are_three_and_eight_code_values_at_mid_gray() {
    assert_eq!(
        check_shot_noise_presets_are_three_and_eight_code_values_at_mid_gray(),
        Ok(())
    );
}
