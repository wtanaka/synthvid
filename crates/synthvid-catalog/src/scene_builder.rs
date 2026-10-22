//! Scene construction helpers for catalog entries.

use crate::cover::Entry;
use crate::cover::{BackgroundAxis, FrameRateValue, MotionAxis, ShapeAxis};
use core::error::Error;
use core::fmt;
use core::num::{NonZeroI64, NonZeroU16};
use synthvid_scene::Overflow;
use synthvid_scene::{
    Background, BlobCount, BlobRadii, BlobRadius, Camera, Channel, Direction, FrameCount,
    FrameIndex, FrameSpan, Magnification, Motion, Numerator, Point, Ratio, Rgb8, Rotation, Scale,
    Seed, Shape, Turns, Vector, Zoom,
};
use synthvid_video::json_names::JsonEntryNameError;

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
    EntryName(JsonEntryNameError),
    /// The entry's defect cannot meet its coding and container.
    VideoSpec(synthvid_video::VideoSpecError),
}

impl fmt::Display for SceneBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameCount(error) => write!(formatter, "frame count invalid: {error}"),
            Self::OscillationPeriod(error) => {
                write!(formatter, "oscillation period invalid: {error}")
            }
            Self::Scale(error) => write!(formatter, "scale declaration failed: {error}"),
            Self::CameraMagnification(error) => {
                write!(formatter, "camera magnification failed: {error}")
            }
            Self::VisibilitySpan(error) => {
                write!(formatter, "object visibility span invalid: {error}")
            }
            Self::RatioConstruction => write!(formatter, "ratio construction failed"),
            Self::FrameRate(error) => write!(formatter, "frame rate invalid: {error}"),
            Self::EntryName(error) => write!(formatter, "entry name invalid: {error}"),
            Self::VideoSpec(error) => write!(formatter, "video specification invalid: {error}"),
        }
    }
}

impl Error for SceneBuildError {}

/// The denominator 1.
const ONE: NonZeroU16 = NonZeroU16::MIN;
/// The denominator 2.
const TWO: NonZeroU16 = match NonZeroU16::new(2) {
    Some(value) => value,
    // Unreachable: 2 is a fixed nonzero literal.
    None => NonZeroU16::MIN,
};
/// The denominator 4.
const FOUR: NonZeroU16 = match NonZeroU16::new(4) {
    Some(value) => value,
    // Unreachable: 4 is a fixed nonzero literal.
    None => NonZeroU16::MIN,
};
/// The denominator 100.
const HUNDRED: NonZeroU16 = match NonZeroU16::new(100) {
    Some(value) => value,
    // Unreachable: 100 is a fixed nonzero literal.
    None => NonZeroU16::MIN,
};
/// The denominator 360.
const THREE_SIXTY: NonZeroU16 = match NonZeroU16::new(360) {
    Some(value) => value,
    // Unreachable: 360 is a fixed nonzero literal.
    None => NonZeroU16::MIN,
};
/// The denominator 1001 of the NTSC frame rate.
const NTSC_DENOMINATOR: NonZeroU16 = match NonZeroU16::new(1001) {
    Some(value) => value,
    // Unreachable: 1001 is a fixed nonzero literal.
    None => NonZeroU16::MIN,
};

/// Scene units per world unit declared by a scaled entry.
///
/// Deliberately not an integer, so that a world coordinate is never equal to
/// its scene coordinate by accident. This ratio (5/2) is always valid.
const SCALED_UNITS_PER_WORLD_UNIT: RatioParts = RatioParts {
    numerator: Numerator(5),
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
    let camera_origin = Point {
        across: Ratio::from_integer(Numerator(0)),
        down: Ratio::from_integer(Numerator(0)),
    };
    let camera_motion = Motion::Fixed(camera_origin);
    let zero_ratio = Ratio::from_integer(Numerator(0));
    let camera_rotation = Rotation::Fixed(Turns::new(zero_ratio));
    let one_ratio = Ratio::from_integer(Numerator(1));
    let camera_zoom =
        Zoom::Fixed(Magnification::new(one_ratio).map_err(SceneBuildError::CameraMagnification)?);
    Ok(Camera::new(camera_motion, camera_rotation, camera_zoom))
}

/// Checker cell size (16 pixels), always valid and nonzero.
const CHECKER_CELL: NonZeroU16 = match NonZeroU16::new(16) {
    Some(value) => value,
    // Unreachable: 16 is a fixed nonzero literal.
    None => NonZeroU16::MIN,
};

/// Grid line spacing (32 pixels), always valid and nonzero.
const GRID_SPACING: NonZeroU16 = match NonZeroU16::new(32) {
    Some(value) => value,
    // Unreachable: 32 is a fixed nonzero literal.
    None => NonZeroU16::MIN,
};

/// Builds a background from the entry's background axis.
pub(crate) const fn build_background(entry: &Entry) -> Background {
    match entry.background() {
        BackgroundAxis::Solid => Background::Solid(Rgb8 {
            red: Channel(128),
            green: Channel(128),
            blue: Channel(128),
        }),
        BackgroundAxis::Checker => Background::Checker {
            cell: CHECKER_CELL,
            even: Rgb8 {
                red: Channel(200),
                green: Channel(200),
                blue: Channel(200),
            },
            odd: Rgb8 {
                red: Channel(100),
                green: Channel(100),
                blue: Channel(100),
            },
        },
        BackgroundAxis::Gradient => Background::Gradient {
            from: Rgb8 {
                red: Channel(50),
                green: Channel(50),
                blue: Channel(50),
            },
            to: Rgb8 {
                red: Channel(200),
                green: Channel(200),
                blue: Channel(200),
            },
            direction: Direction::Horizontal,
        },
        BackgroundAxis::Blobs => Background::Blobs {
            seed: Seed::new(42),
            count: BlobCount(10),
            radii: BlobRadii::spanning([BlobRadius(4), BlobRadius(20)]),
        },
        BackgroundAxis::Grid => Background::Grid {
            spacing: GRID_SPACING,
            line: Rgb8 {
                red: Channel(100),
                green: Channel(100),
                blue: Channel(100),
            },
            ground: Rgb8 {
                red: Channel(180),
                green: Channel(180),
                blue: Channel(180),
            },
        },
    }
}

/// Builds an object from the entry's shape and motion axes.
pub(crate) fn build_object_from_entry(
    entry: &Entry,
) -> Result<synthvid_scene::Object, SceneBuildError> {
    let shape = match entry.shape() {
        ShapeAxis::Disc => Shape::Disc {
            radius: Ratio::from_integer(Numerator(20)),
        },
        ShapeAxis::Rect => Shape::Rect {
            half_width: Ratio::from_integer(Numerator(30)),
            half_height: Ratio::from_integer(Numerator(20)),
        },
        ShapeAxis::Polygon => {
            let vertices = vec![
                Point {
                    across: Ratio::from_integer(Numerator(0)),
                    down: Ratio::from_integer(Numerator(-30)),
                },
                Point {
                    across: Ratio::from_integer(Numerator(30)),
                    down: Ratio::from_integer(Numerator(0)),
                },
                Point {
                    across: Ratio::from_integer(Numerator(0)),
                    down: Ratio::from_integer(Numerator(30)),
                },
                Point {
                    across: Ratio::from_integer(Numerator(-30)),
                    down: Ratio::from_integer(Numerator(0)),
                },
            ];
            Shape::Polygon { vertices }
        }
        ShapeAxis::Cross => Shape::Cross {
            arm: Ratio::from_integer(Numerator(25)),
            thickness: Ratio::from_integer(Numerator(8)),
        },
    };

    let motion = build_motion_from_entry(entry)?;

    let visible = FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(entry.frame_count().get()))
        .map_err(SceneBuildError::VisibilitySpan)?;

    Ok(synthvid_scene::Object::new(
        shape,
        Rgb8 {
            red: Channel(255),
            green: Channel(100),
            blue: Channel(100),
        },
        motion,
        visible,
    ))
}

/// Parameters for creating a ratio with named fields.
#[derive(Copy, Clone, Debug)]
pub(crate) struct RatioParts {
    /// The numerator of the ratio.
    pub(crate) numerator: Numerator,
    /// The denominator of the ratio, non-zero by construction.
    pub(crate) denominator: NonZeroU16,
}

/// Helper to create a `Ratio`; only the exact arithmetic can fail.
pub(crate) fn make_ratio(parts: RatioParts) -> Result<Ratio, SceneBuildError> {
    Ratio::new(parts.numerator, NonZeroI64::from(parts.denominator))
        .map_err(|Overflow| SceneBuildError::RatioConstruction)
}

/// An entry's frame width and height, as numerators of the ratios that
/// place an object relative to the frame.
#[derive(Copy, Clone, Debug)]
struct FrameExtent {
    /// The frame width in pixels.
    width: Numerator,
    /// The frame height in pixels.
    height: Numerator,
}

/// Extracts width and height from an entry's dimensions.
fn frame_extent(entry: &Entry) -> FrameExtent {
    FrameExtent {
        width: Numerator(NonZeroI64::from(entry.dimensions().width.get()).get()),
        height: Numerator(NonZeroI64::from(entry.dimensions().height.get()).get()),
    }
}

/// Builds a fixed motion at the center of the screen.
fn build_fixed_motion(entry: &Entry) -> Result<Motion, SceneBuildError> {
    let FrameExtent { width, height } = frame_extent(entry);
    Ok(Motion::Fixed(Point {
        across: make_ratio(RatioParts {
            numerator: width,
            denominator: TWO,
        })?,
        down: make_ratio(RatioParts {
            numerator: height,
            denominator: TWO,
        })?,
    }))
}

/// Builds a linear motion with constant velocity.
fn build_linear_motion(entry: &Entry) -> Result<Motion, SceneBuildError> {
    let FrameExtent { width, height } = frame_extent(entry);
    Ok(Motion::Linear {
        origin: Point {
            across: make_ratio(RatioParts {
                numerator: width,
                denominator: FOUR,
            })?,
            down: make_ratio(RatioParts {
                numerator: height,
                denominator: TWO,
            })?,
        },
        velocity: Vector {
            across: make_ratio(RatioParts {
                numerator: Numerator(1),
                denominator: TWO,
            })?,
            down: Ratio::from_integer(Numerator(0)),
        },
    })
}

/// Builds a ballistic motion with constant acceleration.
fn build_ballistic_motion(entry: &Entry) -> Result<Motion, SceneBuildError> {
    let FrameExtent { width, height } = frame_extent(entry);
    Ok(Motion::Ballistic {
        origin: Point {
            across: make_ratio(RatioParts {
                numerator: width,
                denominator: FOUR,
            })?,
            down: make_ratio(RatioParts {
                numerator: height,
                denominator: FOUR,
            })?,
        },
        velocity: Vector {
            across: make_ratio(RatioParts {
                numerator: Numerator(1),
                denominator: ONE,
            })?,
            down: make_ratio(RatioParts {
                numerator: Numerator(1),
                denominator: ONE,
            })?,
        },
        acceleration: Vector {
            across: Ratio::from_integer(Numerator(0)),
            down: make_ratio(RatioParts {
                numerator: Numerator(1),
                denominator: HUNDRED,
            })?,
        },
    })
}

/// Builds a circular motion around the center.
fn build_circular_motion(entry: &Entry) -> Result<Motion, SceneBuildError> {
    let FrameExtent { width, height } = frame_extent(entry);
    Ok(Motion::Circular {
        centre: Point {
            across: make_ratio(RatioParts {
                numerator: width,
                denominator: TWO,
            })?,
            down: make_ratio(RatioParts {
                numerator: height,
                denominator: TWO,
            })?,
        },
        radius: Ratio::from_integer(Numerator(50)),
        turns_per_frame: make_ratio(RatioParts {
            numerator: Numerator(1),
            denominator: THREE_SIXTY,
        })?,
        phase: Ratio::from_integer(Numerator(0)),
    })
}

/// Builds an oscillating motion along the x-axis.
fn build_oscillating_motion(entry: &Entry) -> Result<Motion, SceneBuildError> {
    let FrameExtent { width, height } = frame_extent(entry);
    Ok(Motion::Oscillating {
        origin: Point {
            across: make_ratio(RatioParts {
                numerator: width,
                denominator: TWO,
            })?,
            down: make_ratio(RatioParts {
                numerator: height,
                denominator: TWO,
            })?,
        },
        amplitude: Vector {
            across: Ratio::from_integer(Numerator(40)),
            down: Ratio::from_integer(Numerator(0)),
        },
        period: FrameCount::new(120).map_err(SceneBuildError::OscillationPeriod)?,
        phase: Ratio::from_integer(Numerator(0)),
    })
}

/// Builds a walking motion with random steps.
fn build_walk_motion(entry: &Entry) -> Result<Motion, SceneBuildError> {
    let FrameExtent { width, height } = frame_extent(entry);
    Ok(Motion::Walk {
        origin: Point {
            across: make_ratio(RatioParts {
                numerator: width,
                denominator: TWO,
            })?,
            down: make_ratio(RatioParts {
                numerator: height,
                denominator: TWO,
            })?,
        },
        step: make_ratio(RatioParts {
            numerator: Numerator(1),
            denominator: TWO,
        })?,
        seed: Seed::new(123),
    })
}

/// Builds motion from the entry's motion axis.
pub(crate) fn build_motion_from_entry(entry: &Entry) -> Result<Motion, SceneBuildError> {
    match entry.motion() {
        MotionAxis::Fixed => build_fixed_motion(entry),
        MotionAxis::Linear => build_linear_motion(entry),
        MotionAxis::Ballistic => build_ballistic_motion(entry),
        MotionAxis::Circular => build_circular_motion(entry),
        MotionAxis::Oscillating => build_oscillating_motion(entry),
        MotionAxis::Walk => build_walk_motion(entry),
    }
}

/// Converts a `FrameRateValue` to a `FrameRate`.
pub(crate) fn frame_rate_from_axis(
    fr: FrameRateValue,
) -> Result<synthvid_scene::FrameRate, SceneBuildError> {
    use synthvid_scene::FrameRate;

    match fr {
        FrameRateValue::Thirty => FrameRate::from_fps(30).map_err(SceneBuildError::FrameRate),
        FrameRateValue::Sixty => FrameRate::from_fps(60).map_err(SceneBuildError::FrameRate),
        FrameRateValue::OneTwenty => FrameRate::from_fps(120).map_err(SceneBuildError::FrameRate),
        FrameRateValue::TwoForty => FrameRate::from_fps(240).map_err(SceneBuildError::FrameRate),
        FrameRateValue::Ntsc => {
            let ntsc_fr = make_ratio(RatioParts {
                numerator: Numerator(30000),
                denominator: NTSC_DENOMINATOR,
            })?;
            FrameRate::new(ntsc_fr).map_err(SceneBuildError::FrameRate)
        }
        FrameRateValue::Absurd => {
            let absurd_fr = make_ratio(RatioParts {
                numerator: Numerator(8000),
                denominator: ONE,
            })?;
            FrameRate::new(absurd_fr).map_err(SceneBuildError::FrameRate)
        }
    }
}
