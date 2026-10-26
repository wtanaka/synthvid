//! The preparation of a deep shot's layers for one frame.
//!
//! [`prepare_frame`] evaluates the camera and every layer at one frame, in closed form, and
//! returns the layers that the camera sees, ordered front to back. Each visible layer is projected
//! once onto the sensel lattice: its outline becomes a [`LatticeShape`]. The image shifts of the
//! layers are gathered into one row for each point of the lens aperture, so each row names the
//! shift of every layer through that point. Nothing is sampled here.
//!
//! A layer at or behind the plane of the camera is not drawn. That is the correct image of the
//! scene, so such a layer is omitted rather than reported as an error.

use core::num::{NonZeroI64, NonZeroU32};

use synthvid_scene::scene::Membership;
use synthvid_scene::{
    rotation_at, track_at, Cutout, Dimensions, DiscRadius, Extent, FrameIndex, LatticeBounds,
    LatticeError, LatticePoint, LatticeShape, LatticeShift, Layer, LocalPoint, ObjectIndex,
    Overflow, Position, Rgb8, SubSensel,
};

use crate::optics::FocalLength;
use crate::rig::DeepShot;

use super::{
    lens_shift, project_radius, project_vertex, Admission, DeepRenderError, Depth, LensShiftInput,
    PlaneOffset, ProjectionError, TrigPair, WorldOffset,
};

/// One visible layer, prepared for sampling on one frame.
///
/// The fields are private. The bounds are the smallest inclusive box on the lattice that contains
/// the outline, and they are taken from the outline when the layer is built, so they cannot
/// disagree with it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedLayer {
    /// The fill color of the layer.
    fill: Rgb8,
    /// The outline of the layer on the sensel lattice.
    shape: LatticeShape,
    /// The smallest inclusive box on the lattice that contains the outline.
    bounds: LatticeBounds,
}

impl PreparedLayer {
    /// Builds a prepared layer, taking its bounds from its outline.
    ///
    /// # Errors
    ///
    /// Returns `Err(LatticeError::Overflow)` if the bounds of the outline leave the lattice range.
    fn new(fill: Rgb8, shape: LatticeShape) -> Result<Self, LatticeError> {
        let bounds = shape.bounds()?;
        Ok(Self {
            fill,
            shape,
            bounds,
        })
    }

    /// Returns the fill color of the layer.
    #[must_use]
    pub const fn fill(&self) -> Rgb8 {
        self.fill
    }

    /// Returns the outline of the layer on the sensel lattice.
    #[must_use]
    pub const fn shape(&self) -> &LatticeShape {
        &self.shape
    }

    /// Returns the smallest inclusive box on the lattice that contains the outline.
    #[must_use]
    pub const fn bounds(&self) -> LatticeBounds {
        self.bounds
    }
}

/// The number of points of the lens that light passes through.
///
/// That is one for a pinhole, and the number of kept aperture points for an iris. The count is
/// never zero, since a lens admits light through at least one point.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct LensPointCount(NonZeroU32);

impl LensPointCount {
    /// Builds a count from a number of lens points that is never zero.
    pub(crate) const fn new(count: NonZeroU32) -> Self {
        Self(count)
    }

    /// Returns the number of lens points as a non-zero integer.
    #[must_use]
    pub const fn non_zero(self) -> NonZeroU32 {
        self.0
    }
}

/// A deep shot prepared for one frame: the backdrop, the visible layers from front to back, and
/// the rows of lens shifts that the layers were prepared with.
///
/// The lens is borrowed from the shot it was prepared from. The number of lens points is not
/// stored: it is read from the lens, and every row holds one shift per visible layer, so the rows
/// always match both the lens and the layers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedFrame<'shot> {
    /// The color drawn before any layer.
    backdrop: Rgb8,
    /// The visible layers, from the nearest to the farthest.
    layers: Vec<PreparedLayer>,
    /// One row per point of the lens, in the order of the lens points. Each row holds one shift per
    /// visible layer, in the same front-to-back order as the layers.
    lens_rows: Vec<Vec<LatticeShift>>,
    /// The lens that the rows were computed for, borrowed from the shot.
    admission: &'shot Admission,
}

impl PreparedFrame<'_> {
    /// Returns the color drawn before any layer.
    #[must_use]
    pub const fn backdrop(&self) -> Rgb8 {
        self.backdrop
    }

    /// Returns the visible layers, from the nearest to the farthest.
    #[must_use]
    pub fn layers(&self) -> &[PreparedLayer] {
        &self.layers
    }

    /// Returns one row per point of the lens. Each row holds the shift of every visible layer
    /// through that point, in the order of [`PreparedFrame::layers`].
    #[must_use]
    pub fn lens_rows(&self) -> &[Vec<LatticeShift>] {
        &self.lens_rows
    }

    /// Returns the number of lens points, which is the number of rows.
    #[must_use]
    pub fn lens_points(&self) -> LensPointCount {
        self.admission.lens_points()
    }
}

/// The terms that every projection of one frame shares.
#[derive(Copy, Clone, Debug)]
struct FrameTerms {
    /// The frame being prepared, which names it in errors.
    frame: FrameIndex,
    /// The cosine and sine of the camera's roll.
    trig: TrigPair,
    /// The camera's focal length.
    focal: FocalLength,
    /// The output dimensions of the recorder, in sensels.
    dimensions: Dimensions,
}

/// Where one visible layer sits relative to the camera.
#[derive(Copy, Clone, Debug)]
struct LayerSite {
    /// The layer's position in the stage, which names it in errors.
    index: ObjectIndex,
    /// The offset of the layer's position from the camera axis, across and down.
    base: PlaneOffset,
    /// The layer's depth in front of the camera, which is always positive.
    depth: Depth,
}

/// A prepared layer with the depth and declaration index that place it in the draw order.
struct DrawOrder {
    /// The depth of the layer in front of the camera.
    depth: Depth,
    /// The layer's position in the stage.
    index: ObjectIndex,
    /// The prepared layer itself.
    layer: PreparedLayer,
}

/// The half sizes of a rectangle, named so that the across and down extents cannot be swapped.
#[derive(Copy, Clone, Debug)]
struct HalfSizes {
    /// Half the width of the rectangle, across the frame.
    across: Extent,
    /// Half the height of the rectangle, down the frame.
    down: Extent,
}

/// The zero offset from the camera axis, which is the center of a disc.
const ON_AXIS: PlaneOffset = PlaneOffset {
    across: WorldOffset(0),
    down: WorldOffset(0),
};

/// The zero shift of a pinhole, and of a layer on the plane of focus.
const NO_SHIFT: LatticeShift = LatticeShift {
    across: SubSensel(0),
    down: SubSensel(0),
};

/// Prepares every visible layer of a deep shot for one frame.
///
/// The camera is evaluated with its track and its roll, and each layer with its track. A layer
/// is visible when its span contains the frame, and it is drawn when its depth lies strictly in
/// front of the camera. Each drawn layer is projected once. The layers come back ordered by
/// ascending depth, and for equal depths the layer declared later comes first, since it lies in
/// front of the earlier one. Each point of the lens then gets a row of shifts, one for each layer
/// in that order.
///
/// # Errors
///
/// Returns `Err(DeepRenderError::Track(_))` if the camera or a layer track cannot be evaluated.
/// Returns `Err(DeepRenderError::Roll(_))` if the camera's roll cannot be turned into a rotation.
/// Returns `Err(DeepRenderError::Projection { .. })` if a layer or its lens shift cannot be
/// projected onto the lattice, naming the frame and the layer.
/// Returns `Err(DeepRenderError::Lattice(_))` or `Err(DeepRenderError::Aperture(_))` if an exact
/// lattice or aperture computation fails.
/// Returns `Err(DeepRenderError::Overflow)` if an exact intermediate value leaves its range.
pub fn prepare_frame(
    shot: &DeepShot,
    frame: FrameIndex,
) -> Result<PreparedFrame<'_>, DeepRenderError> {
    let rig = shot.rig();
    let pose = rig.pose();
    let camera = track_at(&pose.track(), frame)?;
    let roll = rotation_at(&pose.roll(), frame)?;
    let trig = TrigPair::of(roll).map_err(DeepRenderError::Roll)?;
    let lens = rig.lens();
    let admission = shot.admission();
    let terms = FrameTerms {
        frame,
        trig,
        focal: lens.focal_length(),
        dimensions: rig.recorder().dimensions(),
    };
    let stage = shot.stage();
    let mut ordered = Vec::new();
    for (position, layer) in stage.layers.iter().enumerate() {
        let index = ObjectIndex(u32::try_from(position).map_err(Overflow::from)?);
        if let Some(entry) = prepare_layer(&terms, camera, layer, index)? {
            ordered.push(entry);
        }
    }
    ordered.sort_by(|left, right| {
        left.depth
            .cmp(&right.depth)
            .then(right.index.cmp(&left.index))
    });
    let lens_rows = lens_rows(&terms, admission, &ordered)?;
    let layers = ordered.into_iter().map(|entry| entry.layer).collect();
    Ok(PreparedFrame {
        backdrop: stage.backdrop,
        layers,
        lens_rows,
        admission,
    })
}

/// Prepares one layer of the stage for a frame, or returns `None` when the layer is not drawn.
///
/// A layer is not drawn when its span does not contain the frame, or when its depth relative to
/// the camera is zero or negative, since such a layer lies at or behind the camera.
///
/// # Errors
///
/// Returns `Err(DeepRenderError::Track(_))` if the layer's track cannot be evaluated, and the
/// errors of [`layer_shape`] if the layer cannot be projected.
fn prepare_layer(
    terms: &FrameTerms,
    camera: Position,
    layer: &Layer,
    index: ObjectIndex,
) -> Result<Option<DrawOrder>, DeepRenderError> {
    match layer.visible.contains(terms.frame) {
        Membership::Inside => {}
        Membership::Outside => return Ok(None),
    }
    let placed = track_at(&layer.track, terms.frame)?;
    let relative = i64::from(placed.depth.get().0)
        .checked_sub(i64::from(camera.depth.get().0))
        .ok_or(Overflow)?;
    if relative <= 0 {
        return Ok(None);
    }
    let depth =
        Depth(NonZeroU32::new(u32::try_from(relative).map_err(Overflow::from)?).ok_or(Overflow)?);
    let site = LayerSite {
        index,
        base: widen_position(placed).minus(widen_position(camera))?,
        depth,
    };
    let shape = layer_shape(terms, &site, &layer.cutout)?;
    let prepared = PreparedLayer::new(layer.fill, shape)?;
    Ok(Some(DrawOrder {
        depth,
        index,
        layer: prepared,
    }))
}

/// Builds the outline of a layer on the lattice, projecting each of its vertices once.
///
/// A disc projects its center and its radius, a rectangle projects its four corners in order,
/// and a polygon projects its vertices in declaration order.
///
/// # Errors
///
/// Returns `Err(DeepRenderError::Projection { .. })` if a vertex or the radius cannot be projected,
/// and `Err(DeepRenderError::Overflow)` if a projected radius is negative.
fn layer_shape(
    terms: &FrameTerms,
    site: &LayerSite,
    cutout: &Cutout,
) -> Result<LatticeShape, DeepRenderError> {
    match cutout {
        Cutout::Disc { radius } => {
            let center = project_at(terms, site, ON_AXIS)?;
            let reach = project_radius(*radius, site.depth, terms.focal, terms.dimensions)
                .map_err(|error| projection_failure(terms, site.index, error))?;
            // A projected radius comes from a positive extent, so it is never negative.
            let lattice_radius = DiscRadius(u64::try_from(reach.0).map_err(Overflow::from)?);
            Ok(LatticeShape::disc(center, lattice_radius))
        }
        Cutout::Rect {
            half_width,
            half_height,
        } => rect_shape(
            terms,
            site,
            HalfSizes {
                across: *half_width,
                down: *half_height,
            },
        ),
        Cutout::Polygon(outline) => {
            let (head, tail) = outline.head_and_tail();
            let [first, second, third] = head;
            let corner = |vertex: &LocalPoint| project_at(terms, site, widen_local(*vertex));
            let corners = [corner(first)?, corner(second)?, corner(third)?];
            let rest = tail
                .iter()
                .map(&corner)
                .collect::<Result<Vec<LatticePoint>, DeepRenderError>>()?;
            Ok(LatticeShape::polygon(corners, rest))
        }
    }
}

/// Projects the four corners of a rectangle, in the order upper left, upper right, lower right,
/// and lower left, so that a camera roll rotates the rectangle exactly.
///
/// # Errors
///
/// Returns the errors of [`project_at`] for any corner, and `Err(DeepRenderError::Overflow)` if
/// a half size cannot be negated.
fn rect_shape(
    terms: &FrameTerms,
    site: &LayerSite,
    half: HalfSizes,
) -> Result<LatticeShape, DeepRenderError> {
    let width = WorldOffset(NonZeroI64::from(half.across.get()).get());
    let height = WorldOffset(NonZeroI64::from(half.down.get()).get());
    let minus_width = WorldOffset(width.0.checked_neg().ok_or(Overflow)?);
    let minus_height = WorldOffset(height.0.checked_neg().ok_or(Overflow)?);
    let first = project_at(
        terms,
        site,
        PlaneOffset {
            across: minus_width,
            down: minus_height,
        },
    )?;
    let second = project_at(
        terms,
        site,
        PlaneOffset {
            across: width,
            down: minus_height,
        },
    )?;
    let third = project_at(
        terms,
        site,
        PlaneOffset {
            across: width,
            down: height,
        },
    )?;
    let fourth = project_at(
        terms,
        site,
        PlaneOffset {
            across: minus_width,
            down: height,
        },
    )?;
    Ok(LatticeShape::polygon([first, second, third], vec![fourth]))
}

/// Projects one vertex of a layer, given as an offset from the layer's position.
///
/// The vertex lies at the layer's position plus `local`, measured from the camera axis.
///
/// # Errors
///
/// Returns `Err(DeepRenderError::Overflow)` if the offset leaves the `i64` range, and
/// `Err(DeepRenderError::Projection { .. })` if the vertex cannot be projected.
fn project_at(
    terms: &FrameTerms,
    site: &LayerSite,
    local: PlaneOffset,
) -> Result<LatticePoint, DeepRenderError> {
    let offset = site.base.plus(local)?;
    project_vertex(
        offset,
        site.depth,
        terms.trig,
        terms.focal,
        terms.dimensions,
    )
    .map_err(|error| projection_failure(terms, site.index, error))
}

/// Builds the rows of lens shifts: one row for each point of the lens, each holding the shift of
/// every layer through that point, in the order of the layers.
///
/// The points come in the order of the admission, and a pinhole has a single point with a zero
/// shift for each layer. Each row therefore has one shift per layer, and there is one row per
/// point, by construction.
///
/// # Errors
///
/// Returns `Err(DeepRenderError::Projection { .. })` if a layer's shift cannot be computed through
/// a point of the lens, naming the frame and the layer.
fn lens_rows(
    terms: &FrameTerms,
    admission: &Admission,
    layers: &[DrawOrder],
) -> Result<Vec<Vec<LatticeShift>>, DeepRenderError> {
    match admission {
        Admission::Pinhole => Ok(vec![layers.iter().map(|_| NO_SHIFT).collect()]),
        Admission::Iris(paired) => paired
            .points()
            .samples()
            .into_iter()
            .map(|sample| {
                layers
                    .iter()
                    .map(|entry| -> Result<LatticeShift, DeepRenderError> {
                        let input = LensShiftInput::from_iris_sample(
                            paired.iris(),
                            terms.focal,
                            terms.dimensions.height,
                            entry.depth,
                            sample,
                        );
                        lens_shift(input)
                            .map_err(|error| projection_failure(terms, entry.index, error))
                    })
                    .collect::<Result<Vec<LatticeShift>, DeepRenderError>>()
            })
            .collect(),
    }
}

/// Names the frame and the layer of a projection failure.
const fn projection_failure(
    terms: &FrameTerms,
    index: ObjectIndex,
    error: ProjectionError,
) -> DeepRenderError {
    DeepRenderError::Projection {
        frame: terms.frame,
        layer: index,
        error,
    }
}

/// Widens a camera-space position to its across and down offsets, which are exact.
fn widen_position(position: Position) -> PlaneOffset {
    PlaneOffset {
        across: WorldOffset(i64::from(position.across.get().0)),
        down: WorldOffset(i64::from(position.down.get().0)),
    }
}

/// Widens a vertex offset within a layer to across and down offsets, which are exact.
fn widen_local(point: LocalPoint) -> PlaneOffset {
    PlaneOffset {
        across: WorldOffset(i64::from(point.across.get().0)),
        down: WorldOffset(i64::from(point.down.get().0)),
    }
}

impl PlaneOffset {
    /// Returns the componentwise sum of this offset and `other`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if either component leaves the `i64` range.
    fn plus(self, other: Self) -> Result<Self, Overflow> {
        Ok(Self {
            across: WorldOffset(self.across.0.checked_add(other.across.0).ok_or(Overflow)?),
            down: WorldOffset(self.down.0.checked_add(other.down.0).ok_or(Overflow)?),
        })
    }

    /// Returns the componentwise difference `self - other`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if either component leaves the `i64` range.
    fn minus(self, other: Self) -> Result<Self, Overflow> {
        Ok(Self {
            across: WorldOffset(self.across.0.checked_sub(other.across.0).ok_or(Overflow)?),
            down: WorldOffset(self.down.0.checked_sub(other.down.0).ok_or(Overflow)?),
        })
    }
}

#[cfg(test)]
mod tests;
