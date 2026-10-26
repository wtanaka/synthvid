//! How a camera's recording medium records the light on the image plane.
//!
//! A medium is either an ideal recording with no noise ([`Medium::Off`]), a digital sensor
//! ([`Medium::Sensor`]), or photographic film ([`Medium::Film`]). Each variant holds only the
//! parameters of its own medium, so a sensor's noise terms cannot be attached to film and a
//! film's grain terms cannot be attached to a sensor.
//!
//! Every fractional quantity is a [`Q8_8`], so the scaling of a stored value is defined in one
//! place. A sensel is one element of the camera's output grid. A grain radius is a [`Ratio`]
//! of the frame height and must be strictly positive. A grain density must be nonzero and no
//! larger than the bound that keeps every grain deviation within a [`Q8_8`] value.

use core::error::Error;
use core::fmt;
use core::num::NonZeroU16;

use synthvid_scene::{Ratio, Seed};

/// An unsigned fixed-point number with eight integer bits and eight fractional bits.
///
/// The stored `u16` holds the value multiplied by 256, so a raw value of 256 is 1.0 and a
/// raw value of 128 is 0.5. Every `u16` is a valid `Q8_8`, so construction cannot fail.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Q8_8(pub u16);

/// Shot-noise variance slope of one sensel, in squared code values per code value.
///
/// A sensel whose light is `v` code values has shot-noise variance `slope * v` in squared code
/// values, so its standard deviation is `sqrt(slope * v)` code values. The slope is per sensel
/// and does not depend on the frame size. In raw Q8.8 the standard deviation is the integer square
/// root of `slope_raw * v * 256`. Every raw slope is valid: at `v = 255` and the largest slope the
/// argument is 4278124800, whose root is 65407, which fits a [`Q8_8`]. Zero means the term is
/// absent.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ShotGain(pub Q8_8);

/// Read-noise standard deviation per sensel, in code values.
///
/// Zero means the term is absent.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ReadSigma(pub Q8_8);

/// Per-sensel gain variation of the fixed pattern, in parts per 65536.
///
/// Zero means every sensel has the same gain.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PrnuSpread(pub u16);

/// Standard deviation of the fixed offset each column carries, in code values.
///
/// Zero means the term is absent.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ColumnSigma(pub Q8_8);

/// Seed of the temporal noise stream.
///
/// The stream is derived from this seed and the frame index, so it is fresh on every frame.
/// It is a distinct type from [`PatternSeed`] so the two cannot be swapped.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TemporalSeed(Seed);

impl TemporalSeed {
    /// Creates a temporal noise seed.
    #[must_use]
    pub const fn new(seed: Seed) -> Self {
        Self(seed)
    }

    /// Returns the underlying seed.
    #[must_use]
    pub const fn get(self) -> Seed {
        self.0
    }
}

/// Seed of the fixed pattern: the per-sensel gain variation and the column offsets.
///
/// The pattern is identical on every frame. It is a distinct type from [`TemporalSeed`] so the
/// two cannot be swapped.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PatternSeed(Seed);

impl PatternSeed {
    /// Creates a fixed-pattern seed.
    #[must_use]
    pub const fn new(seed: Seed) -> Self {
        Self(seed)
    }

    /// Returns the underlying seed.
    #[must_use]
    pub const fn get(self) -> Seed {
        self.0
    }
}

/// The two seeds of a digital sensor.
///
/// The temporal and fixed-pattern seeds are distinct types, so they cannot be given to a
/// sensor in each other's place.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SensorSeeds {
    /// Seed of the temporal noise stream.
    temporal: TemporalSeed,
    /// Seed of the fixed pattern.
    pattern: PatternSeed,
}

impl SensorSeeds {
    /// Creates the seeds of a digital sensor.
    #[must_use]
    pub const fn new(temporal: TemporalSeed, pattern: PatternSeed) -> Self {
        Self { temporal, pattern }
    }

    /// Returns the seed of the temporal noise stream.
    #[must_use]
    pub const fn temporal(self) -> TemporalSeed {
        self.temporal
    }

    /// Returns the seed of the fixed pattern.
    #[must_use]
    pub const fn pattern(self) -> PatternSeed {
        self.pattern
    }
}

/// Error returned when a sensor's noise has no nonzero term.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SensorNoiseError {
    /// Every term is zero, which records the same light as [`Medium::Off`].
    AllTermsAbsent,
}

impl fmt::Display for SensorNoiseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllTermsAbsent => write!(f, "sensor noise needs at least one nonzero term"),
        }
    }
}

impl Error for SensorNoiseError {}

/// The noise of a digital sensor.
///
/// Each term is a separate typed value, and a zero term is absent. At least one term is
/// nonzero, so a sensor with no noise cannot be described as a sensor.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SensorNoise {
    /// Shot-noise variance slope of one sensel.
    shot: ShotGain,
    /// Read-noise standard deviation per sensel.
    read: ReadSigma,
    /// Per-sensel gain variation of the fixed pattern.
    prnu: PrnuSpread,
    /// Standard deviation of the fixed per-column offset.
    column: ColumnSigma,
    /// Seed of the temporal noise stream.
    temporal_seed: TemporalSeed,
    /// Seed of the fixed pattern.
    pattern_seed: PatternSeed,
}

impl SensorNoise {
    /// Creates the noise of a digital sensor from its terms and seeds.
    ///
    /// # Errors
    ///
    /// Returns `Err(SensorNoiseError::AllTermsAbsent)` if every term is zero.
    pub const fn new(
        shot: ShotGain,
        read: ReadSigma,
        prnu: PrnuSpread,
        column: ColumnSigma,
        seeds: SensorSeeds,
    ) -> Result<Self, SensorNoiseError> {
        let ShotGain(Q8_8(shot_raw)) = shot;
        let ReadSigma(Q8_8(read_raw)) = read;
        let PrnuSpread(prnu_raw) = prnu;
        let ColumnSigma(Q8_8(column_raw)) = column;
        if shot_raw == 0 && read_raw == 0 && prnu_raw == 0 && column_raw == 0 {
            return Err(SensorNoiseError::AllTermsAbsent);
        }
        Ok(Self {
            shot,
            read,
            prnu,
            column,
            temporal_seed: seeds.temporal(),
            pattern_seed: seeds.pattern(),
        })
    }

    /// Returns the shot-noise slope.
    #[must_use]
    pub const fn shot(self) -> ShotGain {
        self.shot
    }

    /// Returns the read-noise standard deviation.
    #[must_use]
    pub const fn read(self) -> ReadSigma {
        self.read
    }

    /// Returns the per-sensel gain variation.
    #[must_use]
    pub const fn prnu(self) -> PrnuSpread {
        self.prnu
    }

    /// Returns the column-offset standard deviation.
    #[must_use]
    pub const fn column(self) -> ColumnSigma {
        self.column
    }

    /// Returns the seed of the temporal noise stream.
    #[must_use]
    pub const fn temporal_seed(self) -> TemporalSeed {
        self.temporal_seed
    }

    /// Returns the seed of the fixed pattern.
    #[must_use]
    pub const fn pattern_seed(self) -> PatternSeed {
        self.pattern_seed
    }
}

/// Error returned when attempting to construct a non-positive [`GrainRadius`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GrainRadiusError {
    /// Grain radius must be strictly positive.
    NonPositive,
}

impl fmt::Display for GrainRadiusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositive => write!(f, "grain radius must be strictly positive"),
        }
    }
}

impl Error for GrainRadiusError {}

/// Mean grain radius as a fraction of the frame height.
///
/// The value is strictly positive by construction: [`GrainRadius::new`] rejects zero and
/// negative ratios, so no grain can have a radius that is absent or inverted.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GrainRadius(Ratio);

impl GrainRadius {
    /// Creates a grain radius from a [`Ratio`] of the frame height, rejecting zero and
    /// negative values.
    ///
    /// # Errors
    ///
    /// Returns `Err(GrainRadiusError::NonPositive)` when `radius` is zero or negative.
    pub const fn new(radius: Ratio) -> Result<Self, GrainRadiusError> {
        if radius.numer().0 > 0 {
            Ok(Self(radius))
        } else {
            Err(GrainRadiusError::NonPositive)
        }
    }

    /// Returns the grain radius as a fraction of the frame height.
    #[must_use]
    pub const fn get(self) -> Ratio {
        self.0
    }
}

/// Spread of the grain radius around its mean, in grain radii.
///
/// Zero means every grain has the mean radius.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GrainSpread(pub Q8_8);

/// The largest raw density a grain may have: 8224, which is 32.125 in Q8.8.
///
/// A grain's standard deviation is `density_raw * isqrt(v * (255 - v) * 65536) / 4096` in raw
/// Q8.8. The root is largest at `v = 127` and `v = 128`, where it is 32639, so the deviation is
/// at most `floor(density_raw * 32639 / 4096)`. That is at most 65535, the largest [`Q8_8`] value,
/// exactly when `density_raw` is at most 8224, since `8224 * 32639 = 268423136` lies below
/// `65536 * 4096 = 268435456` and `8225 * 32639 = 268455775` does not.
const MAX_GRAIN_DENSITY_RAW: u16 = 8224;

/// Error returned when a grain density is above the largest value a grain can hold.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GrainDensityError {
    /// The raw density is above 8224, so some grain deviation would not fit a [`Q8_8`] value.
    AboveLimit,
}

impl fmt::Display for GrainDensityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AboveLimit => write!(
                f,
                "grain density must be at most {MAX_GRAIN_DENSITY_RAW} in raw Q8.8"
            ),
        }
    }
}

impl Error for GrainDensityError {}

/// Amplitude scale of the film grain, as a [`Q8_8`] value in which 256 is 1.0.
///
/// A channel value `v` in code values has a grain standard deviation of
/// `density * sqrt(v * (255 - v)) / 16` code values, so the density scales the amplitude of the
/// grain. It is not a count of grains per unit area. The inner value is the raw representation,
/// and it is never zero, so a film with no grain is not described by a density.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GrainDensity(NonZeroU16);

impl GrainDensity {
    /// Creates a grain density from its raw [`Q8_8`] representation, in which 256 is 1.0.
    ///
    /// # Errors
    ///
    /// Returns `Err(GrainDensityError::AboveLimit)` if the raw value is above 8224, the largest
    /// value for which every grain deviation fits a [`Q8_8`] value.
    pub const fn new(raw: NonZeroU16) -> Result<Self, GrainDensityError> {
        if raw.get() <= MAX_GRAIN_DENSITY_RAW {
            Ok(Self(raw))
        } else {
            Err(GrainDensityError::AboveLimit)
        }
    }

    /// Returns the grain density as a [`Q8_8`] value.
    #[must_use]
    pub const fn get(self) -> Q8_8 {
        Q8_8(self.0.get())
    }
}

/// Blur width of each grain, in grain radii.
///
/// Zero means the grains are not blurred.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GrainBlur(pub Q8_8);

/// Seed of the grain pattern.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GrainSeed(Seed);

impl GrainSeed {
    /// Creates a grain seed.
    #[must_use]
    pub const fn new(seed: Seed) -> Self {
        Self(seed)
    }

    /// Returns the underlying seed.
    #[must_use]
    pub const fn get(self) -> Seed {
        self.0
    }
}

/// The grain of photographic film.
///
/// The radius is already known to be positive, and the density is already known to be
/// non-zero, so construction cannot fail.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct FilmGrain {
    /// Mean grain radius as a fraction of the frame height.
    radius: GrainRadius,
    /// Spread of the radius around its mean.
    spread: GrainSpread,
    /// Amplitude scale of the grain.
    density: GrainDensity,
    /// Blur width of each grain.
    blur: GrainBlur,
    /// Seed of the grain pattern.
    seed: GrainSeed,
}

impl FilmGrain {
    /// Creates the grain of a film from its terms.
    #[must_use]
    pub const fn new(
        radius: GrainRadius,
        spread: GrainSpread,
        density: GrainDensity,
        blur: GrainBlur,
        seed: GrainSeed,
    ) -> Self {
        Self {
            radius,
            spread,
            density,
            blur,
            seed,
        }
    }

    /// Returns the mean grain radius.
    #[must_use]
    pub const fn radius(self) -> GrainRadius {
        self.radius
    }

    /// Returns the spread of the radius.
    #[must_use]
    pub const fn spread(self) -> GrainSpread {
        self.spread
    }

    /// Returns the grain density.
    #[must_use]
    pub const fn density(self) -> GrainDensity {
        self.density
    }

    /// Returns the blur width.
    #[must_use]
    pub const fn blur(self) -> GrainBlur {
        self.blur
    }

    /// Returns the seed of the grain pattern.
    #[must_use]
    pub const fn seed(self) -> GrainSeed {
        self.seed
    }
}

/// How a camera's recording medium records the light on the image plane.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Medium {
    /// An ideal recording with no noise.
    Off,
    /// A digital sensor, with the noise its sensels add.
    Sensor(SensorNoise),
    /// Photographic film, with the grain it records.
    Film(FilmGrain),
}

#[cfg(test)]
mod tests;
