//! Exact geometry.
//!
//! [`Point`], [`Vector`], and [`Affine`] carry [`Ratio`]
//! components, so transforms compose and invert exactly rather than
//! accumulating rounding error.

use crate::ratio::Overflow;
use crate::ratio::{Numerator, Ratio};

mod turns;

pub use turns::QuarterTurn;

/// A point in scene units.
///
/// Scene units coincide with pixel units: the pixel with integer indices
/// `(x, y)` covers the unit square `[x, x + 1)` by `[y, y + 1)` and its
/// center is at `(x + 1 / 2, y + 1 / 2)`. The origin `(0, 0)` is the top-left
/// corner of the frame and `y` grows downwards, matching frame row order.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Point {
    /// Horizontal coordinate in scene units.
    pub across: Ratio,
    /// Vertical coordinate in scene units.
    pub down: Ratio,
}

/// A displacement in scene units.
///
/// Unlike [`Point`], a [`Vector`] has no position; it is the difference of
/// two points. An [`Affine`] transform acts on it through its linear part
/// only, ignoring translation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Vector {
    /// Horizontal component in scene units.
    pub across: Ratio,
    /// Vertical component in scene units.
    pub down: Ratio,
}

/// A 2D affine transform with exact rational entries.
///
/// Stored as the 2x3 matrix
/// ```text
/// [ xx  xy  tx ]
/// [ yx  yy  ty ]
/// ```
/// acting as `x' = xx * x + xy * y + tx`, `y' = yx * x + yy * y + ty`.
/// Each linear entry is named by its position: the output row, then the
/// input column. In the conventional `[ a b; c d ]` notation these are
/// `a = xx`, `b = xy`, `c = yx`, and `d = yy`; the manifest keeps those
/// letters as its keys.
/// All entries are [`Ratio`], so composition and inversion are exact.
/// Any operation whose intermediate or final value would overflow returns
/// `None` rather than wrapping.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Affine {
    /// Linear entry mapping input `x` to output `x`.
    pub xx: Ratio,
    /// Linear entry mapping input `y` to output `x`.
    pub xy: Ratio,
    /// Translation added to output `x`.
    pub tx: Ratio,
    /// Linear entry mapping input `x` to output `y`.
    pub yx: Ratio,
    /// Linear entry mapping input `y` to output `y`.
    pub yy: Ratio,
    /// Translation added to output `y`.
    pub ty: Ratio,
}

impl Affine {
    /// Returns the identity transform.
    #[must_use]
    pub fn identity() -> Self {
        let zero = Ratio::from_integer(Numerator(0));
        let one = Ratio::from_integer(Numerator(1));
        Self {
            xx: one,
            xy: zero,
            tx: zero,
            yx: zero,
            yy: one,
            ty: zero,
        }
    }

    /// Returns a pure translation by the given displacement.
    #[must_use]
    pub fn translation(displacement: Vector) -> Self {
        let zero = Ratio::from_integer(Numerator(0));
        let one = Ratio::from_integer(Numerator(1));
        Self {
            xx: one,
            xy: zero,
            tx: displacement.across,
            yx: zero,
            yy: one,
            ty: displacement.down,
        }
    }

    /// Returns a rotation about the origin by the given quarter turns.
    ///
    /// Positive values rotate counter-clockwise in a standard
    /// mathematical frame (`x` right, `y` up). In frame coordinates, where
    /// `y` grows downwards, the visual direction is mirrored, but the
    /// mapping itself is fixed: one quarter turn maps `(x, y)` to
    /// `(-y, x)`. All entries are `-1`, `0`, or `1`, so the rotation is
    /// exact.
    #[must_use]
    pub fn quarter_turns(turn: QuarterTurn) -> Self {
        let zero = Ratio::from_integer(Numerator(0));
        let one = Ratio::from_integer(Numerator(1));
        let neg_one = Ratio::from_integer(Numerator(-1));
        match turn {
            QuarterTurn::Zero => Self {
                xx: one,
                xy: zero,
                tx: zero,
                yx: zero,
                yy: one,
                ty: zero,
            },
            QuarterTurn::One => Self {
                xx: zero,
                xy: neg_one,
                tx: zero,
                yx: one,
                yy: zero,
                ty: zero,
            },
            QuarterTurn::Two => Self {
                xx: neg_one,
                xy: zero,
                tx: zero,
                yx: zero,
                yy: neg_one,
                ty: zero,
            },
            QuarterTurn::Three => Self {
                xx: zero,
                xy: one,
                tx: zero,
                yx: neg_one,
                yy: zero,
                ty: zero,
            },
        }
    }

    /// Applies this transform to a point.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if any intermediate or final value overflows.
    pub fn apply(self, point: Point) -> Result<Point, Overflow> {
        let ax = self.xx.checked_mul(point.across)?;
        let by = self.xy.checked_mul(point.down)?;
        let x_lin = ax.checked_add(by)?;
        let out_x = x_lin.checked_add(self.tx)?;
        let cx = self.yx.checked_mul(point.across)?;
        let dy = self.yy.checked_mul(point.down)?;
        let y_lin = cx.checked_add(dy)?;
        let out_y = y_lin.checked_add(self.ty)?;
        Ok(Point {
            across: out_x,
            down: out_y,
        })
    }

    /// Applies only the linear part of this transform to a vector.
    ///
    /// Translation is ignored.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` on overflow.
    pub fn apply_vector(self, vector: Vector) -> Result<Vector, Overflow> {
        let ax = self.xx.checked_mul(vector.across)?;
        let by = self.xy.checked_mul(vector.down)?;
        let out_x = ax.checked_add(by)?;
        let cx = self.yx.checked_mul(vector.across)?;
        let dy = self.yy.checked_mul(vector.down)?;
        let out_y = cx.checked_add(dy)?;
        Ok(Vector {
            across: out_x,
            down: out_y,
        })
    }

    /// Composes two transforms.
    ///
    /// `self.compose(other)` returns the transform that applies `other`
    /// first and then `self`, that is `(self . other)(p) = self(other(p))`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if any intermediate or final value overflows.
    pub fn compose(self, other: Self) -> Result<Self, Overflow> {
        Ok(Self {
            xx: self
                .xx
                .checked_mul(other.xx)?
                .checked_add(self.xy.checked_mul(other.yx)?)?,
            xy: self
                .xx
                .checked_mul(other.xy)?
                .checked_add(self.xy.checked_mul(other.yy)?)?,
            tx: self
                .xx
                .checked_mul(other.tx)?
                .checked_add(self.xy.checked_mul(other.ty)?)?
                .checked_add(self.tx)?,
            yx: self
                .yx
                .checked_mul(other.xx)?
                .checked_add(self.yy.checked_mul(other.yx)?)?,
            yy: self
                .yx
                .checked_mul(other.xy)?
                .checked_add(self.yy.checked_mul(other.yy)?)?,
            ty: self
                .yx
                .checked_mul(other.tx)?
                .checked_add(self.yy.checked_mul(other.ty)?)?
                .checked_add(self.ty)?,
        })
    }

    /// Returns the transform applied after `self`.
    ///
    /// `self.then(next)` equals `next.compose(self)`: apply `self` first,
    /// then `next`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` on overflow.
    pub fn then(self, next: Self) -> Result<Self, Overflow> {
        next.compose(self)
    }

    /// Inverts this transform.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` when the linear part is singular (determinant zero)
    /// or when any intermediate or final value overflows.
    pub fn inverse(self) -> Result<Self, Overflow> {
        let det = self
            .xx
            .checked_mul(self.yy)?
            .checked_sub(self.xy.checked_mul(self.yx)?)?;
        if det == Ratio::from_integer(Numerator(0)) {
            return Err(Overflow);
        }
        let scale = Ratio::from_integer(Numerator(1)).checked_div(det)?;
        Ok(Self {
            xx: self.yy.checked_mul(scale)?,
            xy: self.xy.checked_neg()?.checked_mul(scale)?,
            tx: self
                .yy
                .checked_mul(scale)?
                .checked_mul(self.tx)?
                .checked_add(
                    self.xy
                        .checked_neg()?
                        .checked_mul(scale)?
                        .checked_mul(self.ty)?,
                )?
                .checked_neg()?,
            yx: self.yx.checked_neg()?.checked_mul(scale)?,
            yy: self.xx.checked_mul(scale)?,
            ty: self
                .yx
                .checked_neg()?
                .checked_mul(scale)?
                .checked_mul(self.tx)?
                .checked_add(self.xx.checked_mul(scale)?.checked_mul(self.ty)?)?
                .checked_neg()?,
        })
    }
}

/// A 2D similarity transform with exact rational entries.
///
/// Stored as the pair `(xx, xy)` with translation `(tx, ty)`, acting as
/// `x' = xx * x + xy * y + tx`, `y' = -xy * x + xx * y + ty`.
///
/// The entries are named by their position in the equivalent [`Affine`]
/// matrix, whose `yy` repeats `xx` and whose `yx` is `-xy`. In the
/// conventional `(a, b)` notation, `a = xx` and `b = xy`.
///
/// Every pair `(xx, xy)` describes a rotation combined with a uniform scale
/// (plus translation), so a similarity can never shear: a disc mapped through
/// one stays a disc. Any operation whose intermediate or final value would
/// overflow returns `None` rather than wrapping. Use
/// [`Similarity::to_affine`] where a general [`Affine`] is genuinely wanted.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Similarity {
    /// Linear entry shared by output `x` from input `x` and output `y` from input `y`.
    pub xx: Ratio,
    /// Linear entry mapping input `y` to output `x`; its negation maps input `x` to output `y`.
    pub xy: Ratio,
    /// Translation added to output `x`.
    pub tx: Ratio,
    /// Translation added to output `y`.
    pub ty: Ratio,
}

impl Similarity {
    /// Returns the identity similarity.
    #[must_use]
    pub fn identity() -> Self {
        let zero = Ratio::from_integer(Numerator(0));
        let one = Ratio::from_integer(Numerator(1));
        Self {
            xx: one,
            xy: zero,
            tx: zero,
            ty: zero,
        }
    }

    /// Returns a pure translation by the given displacement.
    #[must_use]
    pub fn translation(displacement: Vector) -> Self {
        let zero = Ratio::from_integer(Numerator(0));
        let one = Ratio::from_integer(Numerator(1));
        Self {
            xx: one,
            xy: zero,
            tx: displacement.across,
            ty: displacement.down,
        }
    }

    /// Returns a uniform scaling by the given factor on both axes.
    #[must_use]
    pub fn uniform_scale(factor: Ratio) -> Self {
        let zero = Ratio::from_integer(Numerator(0));
        Self {
            xx: factor,
            xy: zero,
            tx: zero,
            ty: zero,
        }
    }

    /// Returns a rotation about the origin by the given quarter turns.
    ///
    /// Positive values rotate counter-clockwise in a standard mathematical
    /// frame (`x` right, `y` up), matching [`Affine::quarter_turns`]: one
    /// quarter turn maps `(x, y)` to `(-y, x)`. The rotation is exact.
    #[must_use]
    pub fn quarter_turns(turn: QuarterTurn) -> Self {
        let zero = Ratio::from_integer(Numerator(0));
        let one = Ratio::from_integer(Numerator(1));
        let neg_one = Ratio::from_integer(Numerator(-1));
        match turn {
            QuarterTurn::Zero => Self {
                xx: one,
                xy: zero,
                tx: zero,
                ty: zero,
            },
            QuarterTurn::One => Self {
                xx: zero,
                xy: neg_one,
                tx: zero,
                ty: zero,
            },
            QuarterTurn::Two => Self {
                xx: neg_one,
                xy: zero,
                tx: zero,
                ty: zero,
            },
            QuarterTurn::Three => Self {
                xx: zero,
                xy: one,
                tx: zero,
                ty: zero,
            },
        }
    }

    /// Applies this similarity to a point.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if any intermediate or final value overflows.
    pub fn apply(self, point: Point) -> Result<Point, Overflow> {
        let neg_b = self.xy.checked_neg()?;
        let ax = self.xx.checked_mul(point.across)?;
        let by = self.xy.checked_mul(point.down)?;
        let x_lin = ax.checked_add(by)?;
        let out_x = x_lin.checked_add(self.tx)?;
        let nx = neg_b.checked_mul(point.across)?;
        let ay = self.xx.checked_mul(point.down)?;
        let y_lin = nx.checked_add(ay)?;
        let out_y = y_lin.checked_add(self.ty)?;
        Ok(Point {
            across: out_x,
            down: out_y,
        })
    }

    /// Applies only the linear part of this similarity to a vector.
    ///
    /// Translation is ignored.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` on overflow.
    pub fn apply_vector(self, vector: Vector) -> Result<Vector, Overflow> {
        let neg_b = self.xy.checked_neg()?;
        let ax = self.xx.checked_mul(vector.across)?;
        let by = self.xy.checked_mul(vector.down)?;
        let out_x = ax.checked_add(by)?;
        let nx = neg_b.checked_mul(vector.across)?;
        let ay = self.xx.checked_mul(vector.down)?;
        let out_y = nx.checked_add(ay)?;
        Ok(Vector {
            across: out_x,
            down: out_y,
        })
    }

    /// Composes two similarities.
    ///
    /// `self.compose(other)` returns the similarity that applies `other`
    /// first and then `self`. Similarities are closed under composition, so
    /// the result is again a similarity.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` on overflow.
    pub fn compose(self, other: Self) -> Result<Self, Overflow> {
        let a_part = self.xx.checked_mul(other.xx)?;
        let b_part = self.xy.checked_mul(other.xy)?;
        let out_a = a_part.checked_sub(b_part)?;
        let c_part = self.xx.checked_mul(other.xy)?;
        let d_part = self.xy.checked_mul(other.xx)?;
        let out_b = c_part.checked_add(d_part)?;
        let ax = self.xx.checked_mul(other.tx)?;
        let by = self.xy.checked_mul(other.ty)?;
        let tx = ax.checked_add(by)?.checked_add(self.tx)?;
        let neg_b = self.xy.checked_neg()?;
        let nx = neg_b.checked_mul(other.tx)?;
        let ay = self.xx.checked_mul(other.ty)?;
        let ty = nx.checked_add(ay)?.checked_add(self.ty)?;
        Ok(Self {
            xx: out_a,
            xy: out_b,
            tx,
            ty,
        })
    }

    /// Returns the similarity applied after `self`.
    ///
    /// `self.then(next)` equals `next.compose(self)`: apply `self` first,
    /// then `next`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` on overflow.
    pub fn then(self, next: Self) -> Result<Self, Overflow> {
        next.compose(self)
    }

    /// Inverts this similarity.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` when the linear part is singular (both `xx` and `xy`
    /// are zero, so the determinant `xx * xx + xy * xy` is zero) or when any
    /// intermediate or final value overflows.
    pub fn inverse(self) -> Result<Self, Overflow> {
        let aa = self.xx.checked_mul(self.xx)?;
        let bb = self.xy.checked_mul(self.xy)?;
        let det = aa.checked_add(bb)?;
        if det == Ratio::from_integer(Numerator(0)) {
            return Err(Overflow);
        }
        let scale = Ratio::from_integer(Numerator(1)).checked_div(det)?;
        let a_inv = self.xx.checked_mul(scale)?;
        let b_inv = self.xy.checked_neg()?.checked_mul(scale)?;
        let fwd_x = a_inv
            .checked_mul(self.tx)?
            .checked_add(b_inv.checked_mul(self.ty)?)?;
        let back = b_inv.checked_neg()?;
        let fwd_y = back
            .checked_mul(self.tx)?
            .checked_add(a_inv.checked_mul(self.ty)?)?;
        Ok(Self {
            xx: a_inv,
            xy: b_inv,
            tx: fwd_x.checked_neg()?,
            ty: fwd_y.checked_neg()?,
        })
    }

    /// Converts this similarity to the general [`Affine`] transform.
    ///
    /// The affine entries are `(xx, xy, tx, -xy, xx, ty)`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` only when negating `xy` overflows.
    pub fn to_affine(self) -> Result<Affine, Overflow> {
        let neg_b = self.xy.checked_neg()?;
        Ok(Affine {
            xx: self.xx,
            xy: self.xy,
            tx: self.tx,
            yx: neg_b,
            yy: self.xx,
            ty: self.ty,
        })
    }
}

#[cfg(test)]
mod tests;
