//! The three-dimensional camera rig that a deep shot is seen through.
//!
//! A [`Rig`] is a perspective camera: a [`Pose`] that places it in the scene, a [`Lens`] that
//! focuses its light, and the [`Recorder`] that records the focused light. A [`DeepShot`] is what
//! the rig looks at: a [`Stage`] of flat layers at depths, the rig, and the [`PixelGrid`] that
//! samples each sensel. Building a rig cannot fail. Building a shot can fail when the lens aperture
//! cannot be sampled, or when the lens is focused closer than eight focal lengths, where the
//! far-field approximation of the image distance by the focal length loses accuracy. The
//! shot keeps the sampled points so that every frame reuses them.

use core::error::Error;
use core::fmt;

use synthvid_scene::ratio::helpers::{compare_doubled_with_product, DoubledOrder};
use synthvid_scene::{PixelGrid, Rotation, Stage, Track};

use crate::deep::{admission, Admission, ApertureError};
use crate::optics::{Aperture, FocalLength, Focus, FrameHeight, Lens};
use crate::recorder::Recorder;

/// Where the camera is, and how far it is rolled about its optical axis.
///
/// The track gives the camera's position in closed form on every frame. The roll is the scene's
/// [`Rotation`], an angle in turns. The two are separate fields, so a position cannot be passed
/// in place of an angle.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Pose {
    /// The camera's position as a function of frame, across, down, and depth.
    track: Track,
    /// The roll of the camera about its optical axis, as a function of frame.
    roll: Rotation,
}

impl Pose {
    /// Creates a pose from the camera's track and its roll.
    #[must_use]
    pub const fn new(track: Track, roll: Rotation) -> Self {
        Self { track, roll }
    }

    /// Returns the camera's track.
    #[must_use]
    pub const fn track(self) -> Track {
        self.track
    }

    /// Returns the roll of the camera about its optical axis.
    #[must_use]
    pub const fn roll(self) -> Rotation {
        self.roll
    }
}

/// A perspective camera: where it is, how its lens focuses, and what records the light.
///
/// The three parts are separate typed values, so they cannot be passed in each other's place.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Rig {
    /// Where the camera is and how far it is rolled.
    pose: Pose,
    /// How the lens focuses light and which aperture it admits it through.
    lens: Lens,
    /// What records the focused light, with its output dimensions and timing.
    recorder: Recorder,
}

impl Rig {
    /// Creates a rig from its pose, lens, and recorder. The pose and recorder are copied into the
    /// rig.
    #[must_use]
    pub const fn new(pose: &Pose, lens: Lens, recorder: &Recorder) -> Self {
        Self {
            pose: *pose,
            lens,
            recorder: *recorder,
        }
    }

    /// Returns where the camera is and how far it is rolled.
    #[must_use]
    pub const fn pose(self) -> Pose {
        self.pose
    }

    /// Returns the lens that focuses the camera's light.
    #[must_use]
    pub const fn lens(self) -> Lens {
        self.lens
    }

    /// Returns what records the focused light.
    #[must_use]
    pub const fn recorder(self) -> Recorder {
        self.recorder
    }
}

/// Why a deep shot could not be built from its stage, rig, and sample grid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeepShotError {
    /// The lens aperture could not be sampled. The error is boxed so that the sizes of the
    /// variants stay within the workspace limit on how far apart they may be.
    Aperture(Box<ApertureError>),
    /// The lens is focused closer than eight focal lengths. At eight focal lengths the image
    /// distance is `8/7` of the focal length, so taking it as the focal length is off by one
    /// seventh, and a closer focus is refused.
    FocusTooClose,
}

impl fmt::Display for DeepShotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aperture(error) => write!(f, "lens aperture could not be sampled: {error}"),
            Self::FocusTooClose => write!(
                f,
                "lens focus must lie at least eight focal lengths from the lens"
            ),
        }
    }
}

impl Error for DeepShotError {}

impl From<ApertureError> for DeepShotError {
    fn from(error: ApertureError) -> Self {
        Self::Aperture(Box::new(error))
    }
}

/// Refuses a lens focused closer than eight focal lengths.
///
/// The focal length is `fn * H / 16` world units, where `fn` is the focal length in sixteenths of a
/// frame height and `H` is the frame height of the iris. A focus `s` is refused when
/// `s < 8 * f`, which is `2 * s < fn * H` in exact integers. At a focus of eight focal lengths the
/// image distance `s * f / (s - f)` is `8 * f / 7`, so taking it as `f` is off by one-seventh of
/// `f`, and the error shrinks for every farther focus. A focus at infinity is always accepted, and
/// so is a pinhole, which has no focus.
///
/// # Errors
///
/// Returns `Err(DeepShotError::FocusTooClose)` when the focus lies closer than eight focal lengths.
fn check_focus(lens: Lens) -> Result<(), DeepShotError> {
    match lens.aperture() {
        Aperture::Pinhole => Ok(()),
        Aperture::Iris(iris) => match iris.focus() {
            Focus::Infinity => Ok(()),
            Focus::At(distance) => {
                let FocalLength(factor) = lens.focal_length();
                let FrameHeight(height) = iris.frame_height();
                match compare_doubled_with_product(distance.get(), factor, height) {
                    DoubledOrder::Below => Err(DeepShotError::FocusTooClose),
                    DoubledOrder::AtOrAbove => Ok(()),
                }
            }
        },
    }
}

/// A scene seen through a rig: the stage of layers, the camera that looks at it, and the number
/// of samples taken across each sensel.
///
/// The lens admission of the rig is computed once, when the shot is built, from the rig's lens.
/// Every frame of the shot reuses it.
///
/// A frame fails to render when a layer's projected extent, or a lens shift of a layer, has a
/// magnitude above 2^28 lattice units, which is 2^20 sensels. Such a layer is refused rather than
/// clipped, so the layers of a shot must be kept farther from the camera or smaller.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DeepShot {
    /// The backdrop and the flat layers at their depths.
    stage: Stage,
    /// The camera that looks at the stage.
    rig: Rig,
    /// The number of samples along each axis of a sensel.
    pixels: PixelGrid,
    /// The points of the rig's lens that light passes through, computed from the rig's lens.
    admission: Admission,
}

impl DeepShot {
    /// Creates a shot from its stage, rig, and sample grid.
    ///
    /// # Errors
    ///
    /// Returns `Err(DeepShotError::FocusTooClose)` when the lens of an iris is focused closer than
    /// eight focal lengths. Returns `Err(DeepShotError::Aperture(_))` with the errors of
    /// sampling the rig's lens aperture, when the lens is an iris.
    pub fn new(stage: Stage, rig: &Rig, pixels: PixelGrid) -> Result<Self, DeepShotError> {
        check_focus(rig.lens())?;
        let admission = admission(rig.lens().aperture())?;
        Ok(Self {
            stage,
            rig: *rig,
            pixels,
            admission,
        })
    }

    /// Returns how the rig's lens admits light, sampled once for the shot.
    pub(crate) const fn admission(&self) -> &Admission {
        &self.admission
    }

    /// Returns the backdrop and the flat layers at their depths.
    #[must_use]
    pub const fn stage(&self) -> &Stage {
        &self.stage
    }

    /// Returns the camera that looks at the stage.
    #[must_use]
    pub const fn rig(&self) -> Rig {
        self.rig
    }

    /// Returns the number of samples along each axis of a sensel.
    #[must_use]
    pub const fn pixels(&self) -> PixelGrid {
        self.pixels
    }
}

#[cfg(test)]
mod tests;
