//! Scene construction helpers for catalog entries.

use crate::cover::Entry;
use core::num::{NonZeroI64, NonZeroU16};
use synthvid_scene::{
    Background, Camera, Direction, FrameCount, FrameIndex, FrameSpan, Magnification, Motion, Point,
    Ratio, Rgb8, Rotation, Scale, Seed, Shape, Turns, Vector, Zoom,
};

/// Why a catalogue entry's scene could not be built.
///
/// A variant that wraps another error shows it in its message.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SceneBuildError {
    /// The entry's frame count is not a valid frame count.
    FrameCount(synthvid_scene::FrameCountError),
    /// The oscillation period is not a valid frame count.
    OscillationPeriod(synthvid_scene::FrameCountError),
    /// The declared scale is not a valid scale.
    Scale(synthvid_scene::ScaleError),
    /// The camera magnification is not a valid magnification.
    CameraMagnification(synthvid_scene::MagnificationError),
    /// An object's visibility span is not a valid span.
    VisibilitySpan(synthvid_scene::FrameSpanError),
    /// A ratio could not be built without overflow.
    RatioConstruction,
    /// The frame rate is not a valid frame rate.
    FrameRate(synthvid_scene::FrameRateError),
    /// The name built from the entry's axis values is not a valid entry name.
    EntryName(synthvid_video::json_names::JsonEntryNameError),
}

impl core::fmt::Display for SceneBuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FrameCount(e) => write!(f, "frame count invalid: {e}"),
            Self::OscillationPeriod(e) => write!(f, "oscillation period invalid: {e}"),
            Self::Scale(e) => write!(f, "scale declaration failed: {e}"),
            Self::CameraMagnification(e) => write!(f, "camera magnification failed: {e}"),
            Self::VisibilitySpan(e) => write!(f, "object visibility span invalid: {e}"),
            Self::RatioConstruction => write!(f, "ratio construction failed"),
            Self::FrameRate(e) => write!(f, "frame rate invalid: {e}"),
            Self::EntryName(e) => write!(f, "entry name invalid: {e}"),
        }
    }
}

impl core::error::Error for SceneBuildError {}

/// The denominator 1.
const ONE: NonZeroU16 = NonZeroU16::MIN;
/// The denominator 2.
const TWO: NonZeroU16 = NonZeroU16::MIN.saturating_add(1);
/// The denominator 4.
const FOUR: NonZeroU16 = NonZeroU16::MIN.saturating_add(3);
/// The denominator 100.
const HUNDRED: NonZeroU16 = NonZeroU16::MIN.saturating_add(99);
/// The denominator 360.
const THREE_SIXTY: NonZeroU16 = NonZeroU16::MIN.saturating_add(359);
/// The denominator 1001 of the NTSC frame rate.
const NTSC_DENOMINATOR: NonZeroU16 = NonZeroU16::MIN.saturating_add(1000);

/// Scene units per world unit declared by a scaled entry.
///
/// Deliberately not an integer, so that a world coordinate is never equal to
/// its scene coordinate by accident. This ratio (5/2) is always valid.
const SCALED_UNITS_PER_WORLD_UNIT: RatioParts = RatioParts {
    numerator: 5,
    denominator: TWO,
};

/// Builds the scale a scaled entry declares.
pub(crate) fn declared_scale() -> Result<Scale, SceneBuildError> {
    let ratio = make_ratio(SCALED_UNITS_PER_WORLD_UNIT)?;
    // SCALED_UNITS_PER_WORLD_UNIT (5/2) is always positive.
    Scale::new(ratio).map_err(SceneBuildError::Scale)
}

/// Builds a camera with fixed origin and identity rotation.
pub(crate) fn build_camera() -> Result<Camera, SceneBuildError> {
    let camera_origin = Point::new(Ratio::from_integer(0), Ratio::from_integer(0));
    let camera_motion = Motion::Fixed(camera_origin);
    let zero_ratio = Ratio::from_integer(0);
    let camera_rotation = Rotation::Fixed(Turns::new(zero_ratio));
    let one_ratio = Ratio::from_integer(1);
    let camera_zoom =
        Zoom::Fixed(Magnification::new(one_ratio).map_err(SceneBuildError::CameraMagnification)?);
    Ok(Camera::new(camera_motion, camera_rotation, camera_zoom))
}

/// Checker cell size (16 pixels), always valid and nonzero.
const CHECKER_CELL: NonZeroU16 = NonZeroU16::MIN.saturating_add(15);

/// Grid line spacing (32 pixels), always valid and nonzero.
const GRID_SPACING: NonZeroU16 = NonZeroU16::MIN.saturating_add(31);

/// Builds a background from the entry's background axis.
pub(crate) const fn build_background(entry: &Entry) -> Background {
    match entry.background {
        crate::cover::BackgroundAxis::Solid => Background::Solid(Rgb8::new(128, 128, 128)),
        crate::cover::BackgroundAxis::Checker => Background::Checker {
            cell: CHECKER_CELL,
            a: Rgb8::new(200, 200, 200),
            b: Rgb8::new(100, 100, 100),
        },
        crate::cover::BackgroundAxis::Gradient => Background::Gradient {
            from: Rgb8::new(50, 50, 50),
            to: Rgb8::new(200, 200, 200),
            direction: Direction::Horizontal,
        },
        crate::cover::BackgroundAxis::Blobs => Background::Blobs {
            seed: Seed::new(42),
            count: 10,
            min_radius: 4,
            max_radius: 20,
        },
        crate::cover::BackgroundAxis::Grid => Background::Grid {
            spacing: GRID_SPACING,
            line: Rgb8::new(100, 100, 100),
            ground: Rgb8::new(180, 180, 180),
        },
    }
}

/// Builds an object from the entry's shape and motion axes.
pub(crate) fn build_object_from_entry(
    entry: &Entry,
) -> Result<synthvid_scene::Object, SceneBuildError> {
    let shape = match entry.shape {
        crate::cover::ShapeAxis::Disc => Shape::Disc {
            radius: Ratio::from_integer(20),
        },
        crate::cover::ShapeAxis::Rect => Shape::Rect {
            half_width: Ratio::from_integer(30),
            half_height: Ratio::from_integer(20),
        },
        crate::cover::ShapeAxis::Polygon => {
            let vertices = vec![
                Point::new(Ratio::from_integer(0), Ratio::from_integer(-30)),
                Point::new(Ratio::from_integer(30), Ratio::from_integer(0)),
                Point::new(Ratio::from_integer(0), Ratio::from_integer(30)),
                Point::new(Ratio::from_integer(-30), Ratio::from_integer(0)),
            ];
            Shape::Polygon { vertices }
        }
        crate::cover::ShapeAxis::Cross => Shape::Cross {
            arm: Ratio::from_integer(25),
            thickness: Ratio::from_integer(8),
        },
    };

    let motion = build_motion_from_entry(entry)?;

    let visible = FrameSpan::new(FrameIndex::new(0), FrameIndex::new(entry.frame_count.get()))
        .map_err(SceneBuildError::VisibilitySpan)?;

    Ok(synthvid_scene::Object::new(
        shape,
        Rgb8::new(255, 100, 100),
        motion,
        visible,
    ))
}

/// Parameters for creating a ratio with named fields.
#[derive(Copy, Clone, Debug)]
pub(crate) struct RatioParts {
    /// The numerator of the ratio.
    pub(crate) numerator: i64,
    /// The denominator of the ratio, non-zero by construction.
    pub(crate) denominator: NonZeroU16,
}

/// Helper to create a `Ratio`; only the exact arithmetic can fail.
pub(crate) fn make_ratio(parts: RatioParts) -> Result<Ratio, SceneBuildError> {
    Ratio::new(parts.numerator, NonZeroI64::from(parts.denominator))
        .map_err(|_| SceneBuildError::RatioConstruction)
}

/// Extracts width and height from an entry's dimensions as i64 values.
fn dimensions_as_i64(entry: &Entry) -> (i64, i64) {
    let width_i64 = i64::from(entry.dimensions.width.get().get());
    let height_i64 = i64::from(entry.dimensions.height.get().get());
    (width_i64, height_i64)
}

/// Builds a fixed motion at the center of the screen.
fn build_fixed_motion(entry: &Entry) -> Result<Motion, SceneBuildError> {
    let (width, height) = dimensions_as_i64(entry);
    Ok(Motion::Fixed(Point::new(
        make_ratio(RatioParts {
            numerator: width,
            denominator: TWO,
        })?,
        make_ratio(RatioParts {
            numerator: height,
            denominator: TWO,
        })?,
    )))
}

/// Builds a linear motion with constant velocity.
fn build_linear_motion(entry: &Entry) -> Result<Motion, SceneBuildError> {
    let (width, height) = dimensions_as_i64(entry);
    Ok(Motion::Linear {
        origin: Point::new(
            make_ratio(RatioParts {
                numerator: width,
                denominator: FOUR,
            })?,
            make_ratio(RatioParts {
                numerator: height,
                denominator: TWO,
            })?,
        ),
        velocity: Vector::new(
            make_ratio(RatioParts {
                numerator: 1,
                denominator: TWO,
            })?,
            Ratio::from_integer(0),
        ),
    })
}

/// Builds a ballistic motion with constant acceleration.
fn build_ballistic_motion(entry: &Entry) -> Result<Motion, SceneBuildError> {
    let (width, height) = dimensions_as_i64(entry);
    Ok(Motion::Ballistic {
        origin: Point::new(
            make_ratio(RatioParts {
                numerator: width,
                denominator: FOUR,
            })?,
            make_ratio(RatioParts {
                numerator: height,
                denominator: FOUR,
            })?,
        ),
        velocity: Vector::new(
            make_ratio(RatioParts {
                numerator: 1,
                denominator: ONE,
            })?,
            make_ratio(RatioParts {
                numerator: 1,
                denominator: ONE,
            })?,
        ),
        acceleration: Vector::new(
            Ratio::from_integer(0),
            make_ratio(RatioParts {
                numerator: 1,
                denominator: HUNDRED,
            })?,
        ),
    })
}

/// Builds a circular motion around the center.
fn build_circular_motion(entry: &Entry) -> Result<Motion, SceneBuildError> {
    let (width, height) = dimensions_as_i64(entry);
    Ok(Motion::Circular {
        centre: Point::new(
            make_ratio(RatioParts {
                numerator: width,
                denominator: TWO,
            })?,
            make_ratio(RatioParts {
                numerator: height,
                denominator: TWO,
            })?,
        ),
        radius: Ratio::from_integer(50),
        turns_per_frame: make_ratio(RatioParts {
            numerator: 1,
            denominator: THREE_SIXTY,
        })?,
        phase: Ratio::from_integer(0),
    })
}

/// Builds an oscillating motion along the x-axis.
fn build_oscillating_motion(entry: &Entry) -> Result<Motion, SceneBuildError> {
    let (width, height) = dimensions_as_i64(entry);
    Ok(Motion::Oscillating {
        origin: Point::new(
            make_ratio(RatioParts {
                numerator: width,
                denominator: TWO,
            })?,
            make_ratio(RatioParts {
                numerator: height,
                denominator: TWO,
            })?,
        ),
        amplitude: Vector::new(Ratio::from_integer(40), Ratio::from_integer(0)),
        period: FrameCount::new(120).map_err(SceneBuildError::OscillationPeriod)?,
        phase: Ratio::from_integer(0),
    })
}

/// Builds a walking motion with random steps.
fn build_walk_motion(entry: &Entry) -> Result<Motion, SceneBuildError> {
    let (width, height) = dimensions_as_i64(entry);
    Ok(Motion::Walk {
        origin: Point::new(
            make_ratio(RatioParts {
                numerator: width,
                denominator: TWO,
            })?,
            make_ratio(RatioParts {
                numerator: height,
                denominator: TWO,
            })?,
        ),
        step: make_ratio(RatioParts {
            numerator: 1,
            denominator: TWO,
        })?,
        seed: Seed::new(123),
    })
}

/// Builds motion from the entry's motion axis.
pub(crate) fn build_motion_from_entry(entry: &Entry) -> Result<Motion, SceneBuildError> {
    match entry.motion {
        crate::cover::MotionAxis::Fixed => build_fixed_motion(entry),
        crate::cover::MotionAxis::Linear => build_linear_motion(entry),
        crate::cover::MotionAxis::Ballistic => build_ballistic_motion(entry),
        crate::cover::MotionAxis::Circular => build_circular_motion(entry),
        crate::cover::MotionAxis::Oscillating => build_oscillating_motion(entry),
        crate::cover::MotionAxis::Walk => build_walk_motion(entry),
    }
}

/// Converts a `FrameRateValue` to a `FrameRate`.
pub(crate) fn frame_rate_from_axis(
    fr: crate::cover::FrameRateValue,
) -> Result<synthvid_scene::FrameRate, SceneBuildError> {
    use crate::cover::FrameRateValue;
    use synthvid_scene::FrameRate;

    match fr {
        FrameRateValue::Thirty => FrameRate::from_fps(30).map_err(SceneBuildError::FrameRate),
        FrameRateValue::Sixty => FrameRate::from_fps(60).map_err(SceneBuildError::FrameRate),
        FrameRateValue::OneTwenty => FrameRate::from_fps(120).map_err(SceneBuildError::FrameRate),
        FrameRateValue::TwoForty => FrameRate::from_fps(240).map_err(SceneBuildError::FrameRate),
        FrameRateValue::Ntsc => {
            let ntsc_fr = make_ratio(RatioParts {
                numerator: 30000,
                denominator: NTSC_DENOMINATOR,
            })?;
            FrameRate::new(ntsc_fr).map_err(SceneBuildError::FrameRate)
        }
        FrameRateValue::Absurd => {
            let absurd_fr = make_ratio(RatioParts {
                numerator: 8000,
                denominator: ONE,
            })?;
            FrameRate::new(absurd_fr).map_err(SceneBuildError::FrameRate)
        }
    }
}
