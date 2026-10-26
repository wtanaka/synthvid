//! The points of a lens aperture that rays are cast through.
//!
//! An aperture grid of `A` samples per axis names the candidate points whose unit-aperture
//! coordinates are `(a / A, b / A)`, with `a` and `b` odd and within `A - 1` of zero. The outline
//! of the aperture is drawn on the unit circle: a disc of radius 1, or a regular polygon whose
//! vertex `k` lies at the cosine and sine of `k / n` turns.
//!
//! A candidate is kept when both it and its point reflection through the center lie inside or on
//! the outline. The kept set is therefore centrally symmetric, so its numerators sum to zero on
//! each axis, and a defocused layer does not shift sideways by the mean aperture coordinate.
//!
//! A polygon with four or more blades always keeps points. Its inradius is `cos(pi / n)`, at least
//! `0.707`, and the candidates nearest the center lie at distance `sqrt(2) / A`, at most `0.707`,
//! so those candidates and their reflections are inside. A triangle has inradius `1/2`, which still
//! holds the nearest candidates on the four by four and eight by eight grids. On the two by two
//! grid the nearest candidates lie at `0.707`, and a triangle keeps no candidate whose reflection
//! is also inside, so that outline is refused with [`ApertureError::NoSymmetricPoints`].
//!
//! The inside tests are exact. Each unit coordinate is scaled by 2^15, which is exact because `A`
//! divides 2^15, and the vertices are the 2^15-scaled numerators of the trigonometry of the scene.
//! The tests are the lattice coverage predicates, so a point on an edge counts as inside.
//!
//! The points are computed once per shot, from its lens. They come out in a fixed order: rows of
//! the down numerator in ascending order, and within each row the across numerator in ascending
//! order.

use core::error::Error;
use core::fmt;
use core::num::{NonZeroI64, NonZeroU64};

use synthvid_scene::{
    covers, Containment, DiscRadius, LatticeError, LatticePoint, LatticeShape, Numerator, Overflow,
    Ratio, SubSensel, Turns,
};

use crate::optics::{ApertureGrid, ApertureShape, BladeCount};

use super::error::ProjectionError;
use super::lens::{LensNumerator, LensNumerators, LensSample};
use super::prepare::LensPointCount;
use super::roll::TrigPair;

/// The lattice length of one unit of the aperture, 2^15.
const UNIT: i64 = 0x8000;

/// The number of samples `A` taken along each axis of a grid.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct SampleCount(i64);

/// The position of a polygon vertex, counted from zero in boundary order.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct VertexIndex(i64);

/// The lattice length between two adjacent numerators on one axis, `2^15 / A`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct NumeratorSpacing(i64);

/// The odd numerators along one axis of a two by two grid, in ascending order.
const TWO_AXIS: [LensNumerator; 2] = [LensNumerator(-1), LensNumerator(1)];

/// The odd numerators along one axis of a four by four grid, in ascending order.
const FOUR_AXIS: [LensNumerator; 4] = [
    LensNumerator(-3),
    LensNumerator(-1),
    LensNumerator(1),
    LensNumerator(3),
];

/// The odd numerators along one axis of an eight by eight grid, in ascending order.
const EIGHT_AXIS: [LensNumerator; 8] = [
    LensNumerator(-7),
    LensNumerator(-5),
    LensNumerator(-3),
    LensNumerator(-1),
    LensNumerator(1),
    LensNumerator(3),
    LensNumerator(5),
    LensNumerator(7),
];

/// Why the aperture points of a shape could not be computed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ApertureError {
    /// Exact arithmetic could not represent an intermediate value or a result.
    Overflow,
    /// An exact inside test on the lattice failed.
    Lattice(LatticeError),
    /// A polygon vertex could not be placed from the trigonometry of the scene.
    Trig(ProjectionError),
    /// No candidate of the grid lies inside the outline together with its point reflection, so
    /// the outline keeps no symmetric set of samples.
    NoSymmetricPoints,
}

impl fmt::Display for ApertureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => write!(f, "aperture arithmetic could not represent a result"),
            Self::Lattice(error) => write!(f, "aperture inside test failed: {error}"),
            Self::Trig(error) => write!(f, "aperture vertex could not be placed: {error}"),
            Self::NoSymmetricPoints => write!(
                f,
                "no aperture sample lies inside the outline together with its reflection"
            ),
        }
    }
}

impl Error for ApertureError {}

/// Every overflow of aperture arithmetic is the same failure, so the variant alone carries the
/// fact and the overflow itself has nothing further to report.
impl From<Overflow> for ApertureError {
    fn from(_: Overflow) -> Self {
        Self::Overflow
    }
}

impl From<LatticeError> for ApertureError {
    fn from(error: LatticeError) -> Self {
        Self::Lattice(error)
    }
}

impl From<ProjectionError> for ApertureError {
    fn from(error: ProjectionError) -> Self {
        Self::Trig(error)
    }
}

/// The lens samples kept for one aperture outline on one grid.
///
/// The kept candidates are a set of bits over the candidates of the grid in row order: bit `i` is
/// set when candidate `i` is kept. A grid has at most eight by eight candidates, so the set fits in
/// 64 bits. The set is never empty, which the type records, and it is centrally symmetric, so its
/// count is even and its numerators sum to zero on each axis. The fields are private, so the set
/// can only come from [`aperture_points`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct AperturePoints {
    /// The grid whose candidates the bits index.
    grid: ApertureGrid,
    /// The kept candidates, with bit `i` set for candidate `i` in row order.
    kept: NonZeroU64,
}

impl AperturePoints {
    /// Returns the grid the samples were taken from.
    #[must_use]
    pub const fn grid(&self) -> ApertureGrid {
        self.grid
    }

    /// Returns the kept samples in their fixed order, which is the row order of the grid.
    #[must_use]
    pub fn samples(&self) -> Vec<LensSample> {
        let mut remaining = self.kept.get();
        let mut samples = Vec::new();
        for candidate in candidates(self.grid) {
            // Bit 0 of `remaining` belongs to the candidate visited now, and each step moves the
            // next bit down into position 0. An empty set has no bit set, so its trailing zeros
            // count is 64 and never 0.
            if remaining.trailing_zeros() == 0 {
                samples.push(LensSample::of_candidate(candidate, self.grid));
            }
            remaining = remaining.unbounded_shr(1);
        }
        samples
    }

    /// Returns how many samples were kept: the number of set bits. The set is never empty, so the
    /// count is never zero, and it is even because the samples come in reflected pairs.
    #[must_use]
    pub const fn count(&self) -> LensPointCount {
        LensPointCount::new(self.kept.count_ones())
    }
}

/// Returns the kept lens samples of an aperture outline on a grid.
///
/// A candidate is kept when both it and its point reflection through the center lie inside or on
/// the boundary of the outline. The outline is built once, and every candidate is tested against
/// it. The kept set is centrally symmetric, and a set with no kept candidate is refused rather than
/// returned empty.
///
/// # Errors
///
/// Returns `Err(ApertureError::Overflow)` if a coordinate leaves the `i64` range.
/// Returns `Err(ApertureError::Lattice(_))` if an exact inside test overflows.
/// Returns `Err(ApertureError::Trig(_))` if a polygon vertex cannot be placed.
/// Returns `Err(ApertureError::NoSymmetricPoints)` if no candidate is kept.
pub fn aperture_points(
    shape: ApertureShape,
    grid: ApertureGrid,
) -> Result<AperturePoints, ApertureError> {
    let outline = lattice_shape(shape)?;
    let spacing = numerator_spacing(sample_count(grid))?;
    let mut bits = 0_u64;
    // The candidates are visited from last to first. Each step shifts the bits collected so far up
    // by one, so the candidate visited now lands in bit 0, and after the last step candidate `i`
    // sits in bit `i`. The shift keeps bit 0 clear, so adding one cannot overflow.
    for candidate in candidates(grid).rev() {
        let covered = symmetric_covered(&outline, spacing, candidate)?;
        bits = bits.unbounded_shl(1);
        match covered {
            Containment::Inside => {
                bits = bits.checked_add(1).ok_or(ApertureError::Overflow)?;
            }
            Containment::Outside => {}
        }
    }
    let kept = NonZeroU64::new(bits).ok_or(ApertureError::NoSymmetricPoints)?;
    Ok(AperturePoints { grid, kept })
}

/// The odd numerators along one axis of a grid, in ascending order.
const fn axis_numerators(grid: ApertureGrid) -> &'static [LensNumerator] {
    match grid {
        ApertureGrid::Two => &TWO_AXIS,
        ApertureGrid::Four => &FOUR_AXIS,
        ApertureGrid::Eight => &EIGHT_AXIS,
    }
}

/// The candidates of a grid in row order: the down numerator is the outer loop and the across
/// numerator the inner loop, both in ascending order. Candidate `i` of this order is bit `i` of
/// the kept set of an aperture.
fn candidates(grid: ApertureGrid) -> impl DoubleEndedIterator<Item = LensNumerators> {
    let axis = axis_numerators(grid);
    axis.iter().flat_map(move |&down| {
        axis.iter()
            .map(move |&across| LensNumerators { across, down })
    })
}

/// The number of samples `A` along each axis of a grid.
const fn sample_count(grid: ApertureGrid) -> SampleCount {
    match grid {
        ApertureGrid::Two => SampleCount(2),
        ApertureGrid::Four => SampleCount(4),
        ApertureGrid::Eight => SampleCount(8),
    }
}

/// The lattice length between adjacent numerators, which is `2^15 / A`.
///
/// # Errors
///
/// Returns `Err(ApertureError::Overflow)` if the grid has no sample count to divide by.
fn numerator_spacing(count: SampleCount) -> Result<NumeratorSpacing, ApertureError> {
    let SampleCount(samples) = count;
    UNIT.checked_div(samples)
        .map(NumeratorSpacing)
        .ok_or(ApertureError::Overflow)
}

/// Returns whether the candidate with the given numerators lies inside or on an outline that is
/// already built on the lattice.
///
/// The candidate is placed at `g * 2^15 / A` on each axis and tested exactly against the outline.
///
/// # Errors
///
/// Returns `Err(ApertureError::Overflow)` if a coordinate leaves the `i64` range.
/// Returns `Err(ApertureError::Lattice(_))` if the exact inside test overflows.
fn covered_on(
    outline: &LatticeShape,
    spacing: NumeratorSpacing,
    numerators: LensNumerators,
) -> Result<Containment, ApertureError> {
    let point = lattice_point(numerators, spacing)?;
    Ok(covers(outline, point)?)
}

/// Returns whether a candidate and its point reflection through the center both lie inside or on
/// an outline that is already built on the lattice.
///
/// The reflection is tested only when the candidate itself is inside, so a candidate outside the
/// outline is never kept on account of its reflection.
///
/// # Errors
///
/// Returns `Err(ApertureError::Overflow)` if a coordinate leaves the `i64` range.
/// Returns `Err(ApertureError::Lattice(_))` if an exact inside test overflows.
fn symmetric_covered(
    outline: &LatticeShape,
    spacing: NumeratorSpacing,
    numerators: LensNumerators,
) -> Result<Containment, ApertureError> {
    match covered_on(outline, spacing, numerators)? {
        Containment::Inside => covered_on(outline, spacing, reflected(numerators)?),
        Containment::Outside => Ok(Containment::Outside),
    }
}

/// The point reflection of a candidate through the center, which negates both numerators.
///
/// # Errors
///
/// Returns `Err(ApertureError::Overflow)` if a numerator is the most negative `i64`, whose
/// negation is not representable.
fn reflected(numerators: LensNumerators) -> Result<LensNumerators, ApertureError> {
    let across = numerators
        .across
        .0
        .checked_neg()
        .ok_or(ApertureError::Overflow)?;
    let down = numerators
        .down
        .0
        .checked_neg()
        .ok_or(ApertureError::Overflow)?;
    Ok(LensNumerators {
        across: LensNumerator(across),
        down: LensNumerator(down),
    })
}

/// Returns whether the candidate with the given numerators lies inside or on the outline of an
/// aperture on a grid, before the reflection test of [`aperture_points`] is applied. The outline
/// is built for each call, so this is used by the tests alone.
///
/// # Errors
///
/// Returns `Err(ApertureError::Overflow)` if a coordinate leaves the `i64` range.
/// Returns `Err(ApertureError::Lattice(_))` if the exact inside test overflows.
/// Returns `Err(ApertureError::Trig(_))` if a polygon vertex cannot be placed.
#[cfg(test)]
fn candidate_inside(
    shape: ApertureShape,
    grid: ApertureGrid,
    numerators: LensNumerators,
) -> Result<Containment, ApertureError> {
    let outline = lattice_shape(shape)?;
    let spacing = numerator_spacing(sample_count(grid))?;
    covered_on(&outline, spacing, numerators)
}

/// Scales a candidate's numerators onto the lattice, so that numerator `g` lies at `g * 2^15 / A`.
///
/// # Errors
///
/// Returns `Err(ApertureError::Overflow)` if a scaled coordinate leaves the `i64` range.
fn lattice_point(
    numerators: LensNumerators,
    spacing: NumeratorSpacing,
) -> Result<LatticePoint, ApertureError> {
    Ok(LatticePoint {
        across: scaled(numerators.across, spacing)?,
        down: scaled(numerators.down, spacing)?,
    })
}

/// Scales one numerator onto the lattice.
///
/// # Errors
///
/// Returns `Err(ApertureError::Overflow)` if the product leaves the `i64` range.
fn scaled(numerator: LensNumerator, spacing: NumeratorSpacing) -> Result<SubSensel, ApertureError> {
    let LensNumerator(raw) = numerator;
    let NumeratorSpacing(unit) = spacing;
    let product = raw.checked_mul(unit).ok_or(ApertureError::Overflow)?;
    Ok(SubSensel(product))
}

/// Builds the outline of an aperture on the lattice, in units of 2^15.
///
/// # Errors
///
/// Returns `Err(ApertureError::Lattice(_))` if the disc cannot be built, and the errors of
/// [`vertex`] if a polygon vertex cannot be placed.
fn lattice_shape(shape: ApertureShape) -> Result<LatticeShape, ApertureError> {
    match shape {
        ApertureShape::Disc => {
            let center = LatticePoint {
                across: SubSensel(0),
                down: SubSensel(0),
            };
            Ok(LatticeShape::disc(center, DiscRadius(UNIT.unsigned_abs())))
        }
        ApertureShape::Polygon(blades) => {
            let head = [
                vertex(blades, VertexIndex(0))?,
                vertex(blades, VertexIndex(1))?,
                vertex(blades, VertexIndex(2))?,
            ];
            let tail = (3..NonZeroI64::from(blades.get()).get())
                .map(|index| vertex(blades, VertexIndex(index)))
                .collect::<Result<Vec<LatticePoint>, ApertureError>>()?;
            Ok(LatticeShape::polygon(head, tail))
        }
    }
}

/// The lattice position of one vertex of a polygon with `blades` vertices.
///
/// Vertex `k` lies at the cosine and sine of `k / blades` turns, scaled by 2^15. The roll that
/// [`TrigPair::of`] takes is the negation of the vertex angle, so the pair it returns is exactly
/// the cosine and sine of that angle.
///
/// # Errors
///
/// Returns `Err(ApertureError::Overflow)` if the angle cannot be represented.
/// Returns `Err(ApertureError::Trig(_))` if the trigonometry cannot be scaled to 2^15.
fn vertex(blades: BladeCount, index: VertexIndex) -> Result<LatticePoint, ApertureError> {
    let VertexIndex(position) = index;
    let negated = position.checked_neg().ok_or(ApertureError::Overflow)?;
    let angle = Ratio::new(Numerator(negated), NonZeroI64::from(blades.get()))?;
    let pair = TrigPair::of(Turns::new(angle))?;
    Ok(LatticePoint {
        across: SubSensel(pair.cosine().0),
        down: SubSensel(pair.sine().0),
    })
}

#[cfg(test)]
mod tests;
