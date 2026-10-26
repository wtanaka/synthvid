//! The recording media the recording singletons record through.
//!
//! The covering array holds every entry at [`RecordingAxis::Ideal`]. Each non-ideal preset
//! here is one sensor or film medium with fixed parameters, and only the recording singletons
//! carry one.

use super::cover_axes::RecordingAxis;
use crate::scene_builder::{make_ratio, RatioParts, SceneBuildError};
use core::num::NonZeroU16;
use synthvid_scene::{Numerator, Seed};
use synthvid_video::medium::{PatternSeed, SensorSeeds, TemporalSeed};
use synthvid_video::{
    ColumnSigma, FilmGrain, GrainBlur, GrainDensity, GrainRadius, GrainSeed, GrainSpread, Medium,
    PrnuSpread, ReadSigma, SensorNoise, ShotGain, Q8_8,
};

/// The non-ideal presets, in the order the recording singletons take them.
pub(super) const RECORDED_PRESETS: [RecordingAxis; 5] = [
    RecordingAxis::LowIso,
    RecordingAxis::HighIso,
    RecordingAxis::Banded,
    RecordingAxis::FineGrain,
    RecordingAxis::CoarseGrain,
];

/// The denominator of the fine grain's radius: a grain has a radius of one four-hundredth of the
/// frame height.
pub(super) const FINE_GRAIN_DENOMINATOR: NonZeroU16 = match NonZeroU16::new(400) {
    Some(value) => value,
    // Unreachable: 400 is a fixed nonzero literal.
    None => NonZeroU16::MIN,
};

/// The denominator of the coarse grain's radius: a grain has a radius of one sixteenth of the
/// frame height.
pub(super) const COARSE_GRAIN_DENOMINATOR: NonZeroU16 = match NonZeroU16::new(16) {
    Some(value) => value,
    // Unreachable: 16 is a fixed nonzero literal.
    None => NonZeroU16::MIN,
};

/// The grain density of both film presets, 256 in Q8.8: a density of 1.0.
pub(super) const GRAIN_DENSITY_ONE: NonZeroU16 = match NonZeroU16::new(256) {
    Some(value) => value,
    // Unreachable: 256 is a fixed nonzero literal.
    None => NonZeroU16::MIN,
};

impl RecordingAxis {
    /// The medium a camera records through under this preset. [`RecordingAxis::Ideal`] is
    /// [`Medium::Off`].
    ///
    /// # Errors
    ///
    /// Returns `SceneBuildError::RatioConstruction` if a grain radius ratio cannot be built, and
    /// `SceneBuildError::GrainRadius` if a grain radius is not positive. A grain density above its
    /// limit is `SceneBuildError::GrainDensity`, and a sensor with no nonzero term is
    /// `SceneBuildError::SensorNoise`.
    pub(crate) fn medium(self) -> Result<Medium, SceneBuildError> {
        match self {
            Self::Ideal => Ok(Medium::Off),
            // A sensor at low sensitivity: a shot noise of 3 code values and a read noise of
            // 0.5 code values at a light of 128, and no fixed-pattern terms, so the noise is
            // temporal and the same at every sensel.
            Self::LowIso => Ok(Medium::Sensor(
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
                .map_err(|error| SceneBuildError::SensorNoise(Box::new(error)))?,
            )),
            // A sensor at high sensitivity: at a light of 128 the shot noise is 8 code values,
            // about 2.7 times the 3 of the low preset, and the read noise is 640 raw, five times
            // the 128 of the low preset, so the frame is visibly grainy.
            Self::HighIso => Ok(Medium::Sensor(
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
                .map_err(|error| SceneBuildError::SensorNoise(Box::new(error)))?,
            )),
            // A sensor with a fixed pattern: per-sensel gain variation and a fixed offset in
            // each column, which repeat on every frame, on top of the low preset's temporal
            // noise.
            Self::Banded => Ok(Medium::Sensor(
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
                .map_err(|error| SceneBuildError::SensorNoise(Box::new(error)))?,
            )),
            // Film with fine grain: grains with a radius of a four-hundredth of the frame height,
            // all of one size, at a density of 1.0, and sharp.
            Self::FineGrain => Ok(Medium::Film(FilmGrain::new(
                grain_radius(FINE_GRAIN_DENOMINATOR)?,
                GrainSpread(Q8_8(0)),
                GrainDensity::new(GRAIN_DENSITY_ONE)
                    .map_err(|error| SceneBuildError::GrainDensity(Box::new(error)))?,
                GrainBlur(Q8_8(0)),
                GrainSeed::new(Seed::new(3001)),
            ))),
            // Film with coarse grain: grains with a radius of a sixteenth of the frame height,
            // spread in size and blurred by half a grain radius, at a density of 1.0.
            Self::CoarseGrain => Ok(Medium::Film(FilmGrain::new(
                grain_radius(COARSE_GRAIN_DENOMINATOR)?,
                GrainSpread(Q8_8(77)),
                GrainDensity::new(GRAIN_DENSITY_ONE)
                    .map_err(|error| SceneBuildError::GrainDensity(Box::new(error)))?,
                GrainBlur(Q8_8(128)),
                GrainSeed::new(Seed::new(3002)),
            ))),
        }
    }
}

/// A grain radius of one over `denominator` of the frame height.
fn grain_radius(denominator: NonZeroU16) -> Result<GrainRadius, SceneBuildError> {
    let radius = make_ratio(RatioParts {
        numerator: Numerator(1),
        denominator,
    })?;
    GrainRadius::new(radius).map_err(|error| SceneBuildError::GrainRadius(Box::new(error)))
}
