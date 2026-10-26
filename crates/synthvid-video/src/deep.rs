//! Projection arithmetic for flat layers seen by a perspective camera.
//!
//! The camera sits at the origin and looks along the depth axis, so a point at offset `(dx, dy)`
//! from the axis and depth `z` lands at a sensel offset proportional to `(dx, dy) / z`. The offset
//! of each projected coordinate is computed exactly in 128-bit integers and rounded once, half up,
//! onto the 1/256-sensel lattice of the sensel grid. A rotation of the camera by its roll is
//! applied with the same convention as the view transform of the scene.
//!
//! A vertex is placed at the lattice center of the frame, `128 * W` and `128 * H` units, plus its
//! rounded offset, so the offset is the only rounded term of a vertex. Each lens shift is rounded
//! once on its own, and a sample is tested at its lattice point minus its rounded shift. The
//! effective position of an image point is therefore rounded twice, once in the projection and
//! once in the shift, so it can be off by up to one lattice unit, 1/256 sensel, rather than by
//! half of one.
//!
//! The guard band is checked on each vertex coordinate `128 * W + round(P)`, on each rounded shift,
//! and on each rounded radius, each on its own. It is not checked on the sum of a projected offset
//! and a shift. A result beyond the guard band is [`ProjectionError::BeyondGuardBand`], never a
//! clamped value. [`render_stage_into`] samples the prepared layers of a shot into the sensels of a
//! frame.

mod aperture;
mod error;
pub mod iris_points;
mod lens;
mod prepare;
mod project;
mod render;
mod render_error;
mod roll;
mod round;

pub use aperture::{aperture_points, ApertureError, AperturePoints};
pub use error::ProjectionError;
pub use lens::{LensNumerator, LensNumerators, LensSample, LensSampleError};
pub use prepare::{prepare_frame, LensPointCount, PreparedFrame, PreparedLayer};

use core::num::NonZeroU32;

use crate::optics::Aperture;

use iris_points::IrisPoints;
use lens::{lens_shift, LensShiftInput};

/// How the lens admits light, prepared once for a shot.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) enum Admission {
    /// A pinhole admits light through one point, so every layer has a single zero shift.
    Pinhole,
    /// An iris admits light through each kept point of its aperture, paired with that iris.
    Iris(IrisPoints),
}

impl Admission {
    /// Returns the number of lens points: one for a pinhole, and the kept points of an iris.
    pub(crate) fn lens_points(&self) -> LensPointCount {
        match self {
            Self::Pinhole => LensPointCount::new(NonZeroU32::MIN),
            Self::Iris(paired) => paired.points().count(),
        }
    }
}

/// Prepares how the lens of a shot admits light: a pinhole, or the kept points of an iris.
///
/// # Errors
///
/// Returns the errors of [`aperture_points`] for an iris.
pub(crate) fn admission(aperture: Aperture) -> Result<Admission, ApertureError> {
    match aperture {
        Aperture::Pinhole => Ok(Admission::Pinhole),
        Aperture::Iris(iris) => Ok(Admission::Iris(IrisPoints::new(iris)?)),
    }
}
pub use project::{project_radius, project_vertex, Depth, PlaneOffset, WorldOffset};
pub use render::render_stage_into;
pub use render_error::DeepRenderError;
pub use roll::{TrigNumerator, TrigPair};
pub use round::{round_half_up, NonZeroWideInt, RoundingInput, WideInt};
