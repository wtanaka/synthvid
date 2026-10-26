use super::*;
use crate::testutil::Frac;
use synthvid_scene::Numerator;
use synthvid_testkit::{ensure, ensure_eq, ensure_ne, Fail, OrFail as _};

/// Sensor noise terms with every term present, built from the given seeds.
fn sensor_noise(seeds: SensorSeeds) -> Result<SensorNoise, Fail> {
    SensorNoise::new(
        ShotGain(Q8_8(37)),
        ReadSigma(Q8_8(91)),
        PrnuSpread(1_237),
        ColumnSigma(Q8_8(14)),
        seeds,
    )
    .or_fail()
}

/// Film grain terms built from awkward values, with a radius of 3/7 of the frame height.
fn film_grain() -> Result<FilmGrain, Fail> {
    let radius = GrainRadius::new(Frac(3, 7).ratio()?).or_fail()?;
    Ok(FilmGrain::new(
        radius,
        GrainSpread(Q8_8(77)),
        GrainDensity::new(NonZeroU16::new(341).or_fail()?).or_fail()?,
        GrainBlur(Q8_8(19)),
        GrainSeed::new(Seed::new(53)),
    ))
}

#[test]
fn q8_8_keeps_every_raw_value() {
    assert_eq!(check_q8_8_keeps_every_raw_value(), Ok(()));
}

fn check_q8_8_keeps_every_raw_value() -> Result<(), Fail> {
    for raw in [0_u16, 1, 255, 256, 855, u16::MAX] {
        ensure_eq!(Q8_8(raw).0, raw, "raw value {raw} must round-trip");
    }
    Ok(())
}

#[test]
fn noise_terms_round_trip_including_zero() {
    assert_eq!(check_noise_terms_round_trip_including_zero(), Ok(()));
}

fn check_noise_terms_round_trip_including_zero() -> Result<(), Fail> {
    ensure_eq!(ShotGain(Q8_8(37)).0, Q8_8(37));
    ensure_eq!(ShotGain(Q8_8(0)).0, Q8_8(0), "zero shot gain is legal");
    ensure_eq!(ReadSigma(Q8_8(91)).0, Q8_8(91));
    ensure_eq!(ReadSigma(Q8_8(0)).0, Q8_8(0), "zero read noise is legal");
    ensure_eq!(PrnuSpread(1_237).0, 1_237);
    ensure_eq!(PrnuSpread(0).0, 0, "zero PRNU spread is legal");
    ensure_eq!(ColumnSigma(Q8_8(14)).0, Q8_8(14));
    ensure_eq!(
        ColumnSigma(Q8_8(0)).0,
        Q8_8(0),
        "zero column sigma is legal"
    );
    Ok(())
}

#[test]
fn seed_newtypes_round_trip() {
    assert_eq!(check_seed_newtypes_round_trip(), Ok(()));
}

fn check_seed_newtypes_round_trip() -> Result<(), Fail> {
    let seed = Seed::new(0x9E37_79B9_7F4A_7C15);
    ensure_eq!(TemporalSeed::new(seed).get(), seed);
    ensure_eq!(PatternSeed::new(seed).get(), seed);
    Ok(())
}

#[test]
fn sensor_seeds_keep_each_seed_in_its_own_slot() {
    assert_eq!(check_sensor_seeds_keep_each_seed_in_its_own_slot(), Ok(()));
}

fn check_sensor_seeds_keep_each_seed_in_its_own_slot() -> Result<(), Fail> {
    let seeds = SensorSeeds::new(
        TemporalSeed::new(Seed::new(11)),
        PatternSeed::new(Seed::new(29)),
    );
    ensure_eq!(seeds.temporal(), TemporalSeed::new(Seed::new(11)));
    ensure_eq!(seeds.pattern(), PatternSeed::new(Seed::new(29)));
    Ok(())
}

#[test]
fn sensor_noise_round_trips_each_term() {
    assert_eq!(check_sensor_noise_round_trips_each_term(), Ok(()));
}

fn check_sensor_noise_round_trips_each_term() -> Result<(), Fail> {
    let seeds = SensorSeeds::new(
        TemporalSeed::new(Seed::new(11)),
        PatternSeed::new(Seed::new(29)),
    );
    let noise = sensor_noise(seeds)?;
    ensure_eq!(noise.shot(), ShotGain(Q8_8(37)));
    ensure_eq!(noise.read(), ReadSigma(Q8_8(91)));
    ensure_eq!(noise.prnu(), PrnuSpread(1_237));
    ensure_eq!(noise.column(), ColumnSigma(Q8_8(14)));
    ensure_eq!(noise.temporal_seed(), TemporalSeed::new(Seed::new(11)));
    ensure_eq!(noise.pattern_seed(), PatternSeed::new(Seed::new(29)));
    Ok(())
}

#[test]
fn sensor_noise_with_swapped_seeds_differs() {
    assert_eq!(check_sensor_noise_with_swapped_seeds_differs(), Ok(()));
}

fn check_sensor_noise_with_swapped_seeds_differs() -> Result<(), Fail> {
    let first = Seed::new(11);
    let second = Seed::new(29);
    let forward = SensorSeeds::new(TemporalSeed::new(first), PatternSeed::new(second));
    let swapped = SensorSeeds::new(TemporalSeed::new(second), PatternSeed::new(first));
    ensure_ne!(sensor_noise(forward)?, sensor_noise(swapped)?);
    Ok(())
}

#[test]
fn grain_radius_rejects_zero_and_negative() {
    assert_eq!(check_grain_radius_rejects_zero_and_negative(), Ok(()));
}

fn check_grain_radius_rejects_zero_and_negative() -> Result<(), Fail> {
    let zero = Ratio::from_integer(Numerator(0));
    ensure_eq!(GrainRadius::new(zero), Err(GrainRadiusError::NonPositive));
    let negative = Frac(-1, 200).ratio()?;
    ensure_eq!(
        GrainRadius::new(negative),
        Err(GrainRadiusError::NonPositive)
    );
    Ok(())
}

#[test]
fn grain_radius_accepts_positive_and_round_trips() {
    assert_eq!(
        check_grain_radius_accepts_positive_and_round_trips(),
        Ok(())
    );
}

fn check_grain_radius_accepts_positive_and_round_trips() -> Result<(), Fail> {
    let positive = Frac(1, 200).ratio()?;
    let radius = GrainRadius::new(positive).or_fail()?;
    ensure_eq!(radius.get(), positive);
    Ok(())
}

#[test]
fn grain_density_round_trips_its_raw_value() {
    assert_eq!(check_grain_density_round_trips_its_raw_value(), Ok(()));
}

fn check_grain_density_round_trips_its_raw_value() -> Result<(), Fail> {
    let density = GrainDensity::new(NonZeroU16::new(855).or_fail()?).or_fail()?;
    ensure_eq!(density.get(), Q8_8(855));
    Ok(())
}

#[test]
fn grain_spread_blur_and_seed_round_trip() {
    assert_eq!(check_grain_spread_blur_and_seed_round_trip(), Ok(()));
}

fn check_grain_spread_blur_and_seed_round_trip() -> Result<(), Fail> {
    ensure_eq!(GrainSpread(Q8_8(77)).0, Q8_8(77));
    ensure_eq!(GrainSpread(Q8_8(0)).0, Q8_8(0), "zero spread is legal");
    ensure_eq!(GrainBlur(Q8_8(19)).0, Q8_8(19));
    ensure_eq!(GrainBlur(Q8_8(0)).0, Q8_8(0), "zero blur is legal");
    ensure_eq!(GrainSeed::new(Seed::new(53)).get(), Seed::new(53));
    Ok(())
}

#[test]
fn film_grain_round_trips_each_term() {
    assert_eq!(check_film_grain_round_trips_each_term(), Ok(()));
}

fn check_film_grain_round_trips_each_term() -> Result<(), Fail> {
    let grain = film_grain()?;
    ensure_eq!(grain.radius().get(), Frac(3, 7).ratio()?);
    ensure_eq!(grain.spread(), GrainSpread(Q8_8(77)));
    ensure_eq!(
        grain.density(),
        GrainDensity::new(NonZeroU16::new(341).or_fail()?).or_fail()?
    );
    ensure_eq!(grain.blur(), GrainBlur(Q8_8(19)));
    ensure_eq!(grain.seed(), GrainSeed::new(Seed::new(53)));
    Ok(())
}

#[test]
fn medium_variants_are_distinguishable() {
    assert_eq!(check_medium_variants_are_distinguishable(), Ok(()));
}

fn check_medium_variants_are_distinguishable() -> Result<(), Fail> {
    let off = Medium::Off;
    let seeds = SensorSeeds::new(
        TemporalSeed::new(Seed::new(11)),
        PatternSeed::new(Seed::new(29)),
    );
    let sensor = Medium::Sensor(sensor_noise(seeds)?);
    let film = Medium::Film(film_grain()?);
    ensure_ne!(off, sensor);
    ensure_ne!(off, film);
    ensure_ne!(sensor, film);
    ensure_eq!(off, Medium::Off);
    let is_off = match off {
        Medium::Off => true,
        Medium::Sensor(_) | Medium::Film(_) => false,
    };
    ensure!(is_off, "the off medium must match the off variant");
    Ok(())
}
