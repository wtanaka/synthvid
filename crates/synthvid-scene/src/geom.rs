//! Exact geometry.
//!
//! [`Point`], [`Vector`], and [`Affine`] carry [`Ratio`]
//! components, so transforms compose and invert exactly rather than
//! accumulating rounding error.

use crate::ratio::int_ratio;
use crate::ratio::Ratio;

/// A point in scene units.
///
/// Scene units coincide with pixel units: the pixel with integer indices
/// `(x, y)` covers the unit square `[x, x + 1)` by `[y, y + 1)` and its
/// centre is at `(x + 1 / 2, y + 1 / 2)`. The origin `(0, 0)` is the top-left
/// corner of the frame and `y` grows downwards, matching frame row order.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Point {
    /// Horizontal coordinate in scene units.
    pub x: Ratio,
    /// Vertical coordinate in scene units.
    pub y: Ratio,
}

impl Point {
    /// Creates a new [`Point`] from exact rational coordinates.
    #[must_use]
    pub const fn new(x: Ratio, y: Ratio) -> Self {
        Self { x, y }
    }
}

/// A displacement in scene units.
///
/// Unlike [`Point`], a [`Vector`] has no position; it is the difference of
/// two points. An [`Affine`] transform acts on it through its linear part
/// only, ignoring translation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Vector {
    /// Horizontal component in scene units.
    pub x: Ratio,
    /// Vertical component in scene units.
    pub y: Ratio,
}

impl Vector {
    /// Creates a new [`Vector`] from exact rational components.
    #[must_use]
    pub const fn new(x: Ratio, y: Ratio) -> Self {
        Self { x, y }
    }
}

/// A 2D affine transform with exact rational entries.
///
/// Stored as the 2x3 matrix
/// ```text
/// [ a  b  tx ]
/// [ c  d  ty ]
/// ```
/// acting as `x' = a * x + b * y + tx`, `y' = c * x + d * y + ty`.
/// All entries are [`Ratio`], so composition and inversion are exact.
/// Any operation whose intermediate or final value would overflow returns
/// `None` rather than wrapping.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Affine {
    /// Linear entry mapping input `x` to output `x`.
    pub a: Ratio,
    /// Linear entry mapping input `y` to output `x`.
    pub b: Ratio,
    /// Translation added to output `x`.
    pub tx: Ratio,
    /// Linear entry mapping input `x` to output `y`.
    pub c: Ratio,
    /// Linear entry mapping input `y` to output `y`.
    pub d: Ratio,
    /// Translation added to output `y`.
    pub ty: Ratio,
}

impl Affine {
    /// Creates a new [`Affine`] transform from its six exact entries.
    #[must_use]
    pub const fn new(a: Ratio, b: Ratio, tx: Ratio, c: Ratio, d: Ratio, ty: Ratio) -> Self {
        Self { a, b, tx, c, d, ty }
    }

    /// Returns the identity transform.
    #[must_use]
    pub fn identity() -> Self {
        let zero = int_ratio(0);
        let one = int_ratio(1);
        Self {
            a: one,
            b: zero,
            tx: zero,
            c: zero,
            d: one,
            ty: zero,
        }
    }

    /// Returns a pure translation by the given displacement.
    #[must_use]
    pub fn translation(displacement: Vector) -> Self {
        let zero = int_ratio(0);
        let one = int_ratio(1);
        Self {
            a: one,
            b: zero,
            tx: displacement.x,
            c: zero,
            d: one,
            ty: displacement.y,
        }
    }

    /// Returns an axis-aligned scaling by the given factors.
    #[must_use]
    pub fn scaling(sx: Ratio, sy: Ratio) -> Self {
        let zero = int_ratio(0);
        Self {
            a: sx,
            b: zero,
            tx: zero,
            c: zero,
            d: sy,
            ty: zero,
        }
    }

    /// Returns a rotation about the origin by `turns` quarter turns.
    ///
    /// Positive values rotate counter-clockwise in a standard
    /// mathematical frame (`x` right, `y` up). In frame coordinates, where
    /// `y` grows downwards, the visual direction is mirrored, but the
    /// mapping itself is fixed: one quarter turn maps `(x, y)` to
    /// `(-y, x)`. All entries are `-1`, `0`, or `1`, so the rotation is
    /// exact. The turn count is reduced modulo 4 with Euclidean remainder.
    #[must_use]
    pub fn quarter_turns(turns: i32) -> Self {
        let zero = int_ratio(0);
        let one = int_ratio(1);
        let neg_one = int_ratio(-1);
        let step = turns.rem_euclid(4);
        if step == 0 {
            Self {
                a: one,
                b: zero,
                tx: zero,
                c: zero,
                d: one,
                ty: zero,
            }
        } else if step == 1 {
            Self {
                a: zero,
                b: neg_one,
                tx: zero,
                c: one,
                d: zero,
                ty: zero,
            }
        } else if step == 2 {
            Self {
                a: neg_one,
                b: zero,
                tx: zero,
                c: zero,
                d: neg_one,
                ty: zero,
            }
        } else {
            Self {
                a: zero,
                b: one,
                tx: zero,
                c: neg_one,
                d: zero,
                ty: zero,
            }
        }
    }

    /// Applies this transform to a point.
    ///
    /// Returns `None` if any intermediate or final value overflows.
    #[must_use]
    pub fn apply(self, point: Point) -> Option<Point> {
        let ax = self.a.checked_mul(point.x)?;
        let by = self.b.checked_mul(point.y)?;
        let x_lin = ax.checked_add(by)?;
        let x = x_lin.checked_add(self.tx)?;
        let cx = self.c.checked_mul(point.x)?;
        let dy = self.d.checked_mul(point.y)?;
        let y_lin = cx.checked_add(dy)?;
        let y = y_lin.checked_add(self.ty)?;
        Some(Point { x, y })
    }

    /// Applies only the linear part of this transform to a vector.
    ///
    /// Translation is ignored. Returns `None` on overflow.
    #[must_use]
    pub fn apply_vector(self, vector: Vector) -> Option<Vector> {
        let ax = self.a.checked_mul(vector.x)?;
        let by = self.b.checked_mul(vector.y)?;
        let x = ax.checked_add(by)?;
        let cx = self.c.checked_mul(vector.x)?;
        let dy = self.d.checked_mul(vector.y)?;
        let y = cx.checked_add(dy)?;
        Some(Vector { x, y })
    }

    /// Composes two transforms.
    ///
    /// `self.compose(other)` returns the transform that applies `other`
    /// first and then `self`, that is `(self . other)(p) = self(other(p))`.
    /// Returns `None` if any intermediate or final value overflows.
    #[must_use]
    pub fn compose(self, other: Self) -> Option<Self> {
        Some(Self {
            a: self
                .a
                .checked_mul(other.a)?
                .checked_add(self.b.checked_mul(other.c)?)?,
            b: self
                .a
                .checked_mul(other.b)?
                .checked_add(self.b.checked_mul(other.d)?)?,
            tx: self
                .a
                .checked_mul(other.tx)?
                .checked_add(self.b.checked_mul(other.ty)?)?
                .checked_add(self.tx)?,
            c: self
                .c
                .checked_mul(other.a)?
                .checked_add(self.d.checked_mul(other.c)?)?,
            d: self
                .c
                .checked_mul(other.b)?
                .checked_add(self.d.checked_mul(other.d)?)?,
            ty: self
                .c
                .checked_mul(other.tx)?
                .checked_add(self.d.checked_mul(other.ty)?)?
                .checked_add(self.ty)?,
        })
    }

    /// Returns the transform applied after `self`.
    ///
    /// `self.then(next)` equals `next.compose(self)`: apply `self` first,
    /// then `next`. Returns `None` on overflow.
    #[must_use]
    pub fn then(self, next: Self) -> Option<Self> {
        next.compose(self)
    }

    /// Inverts this transform.
    ///
    /// Returns `None` when the linear part is singular (determinant zero)
    /// or when any intermediate or final value overflows.
    #[must_use]
    pub fn inverse(self) -> Option<Self> {
        let det = self
            .a
            .checked_mul(self.d)?
            .checked_sub(self.b.checked_mul(self.c)?)?;
        if det == int_ratio(0) {
            return None;
        }
        let scale = int_ratio(1).checked_div(det)?;
        Some(Self {
            a: self.d.checked_mul(scale)?,
            b: self.b.checked_neg()?.checked_mul(scale)?,
            tx: self
                .d
                .checked_mul(scale)?
                .checked_mul(self.tx)?
                .checked_add(
                    self.b
                        .checked_neg()?
                        .checked_mul(scale)?
                        .checked_mul(self.ty)?,
                )?
                .checked_neg()?,
            c: self.c.checked_neg()?.checked_mul(scale)?,
            d: self.a.checked_mul(scale)?,
            ty: self
                .c
                .checked_neg()?
                .checked_mul(scale)?
                .checked_mul(self.tx)?
                .checked_add(self.a.checked_mul(scale)?.checked_mul(self.ty)?)?
                .checked_neg()?,
        })
    }
}

/// A 2D similarity transform with exact rational entries.
///
/// Stored as the pair `(a, b)` with translation `(tx, ty)`, acting as
/// `x' = a * x + b * y + tx`, `y' = -b * x + a * y + ty`.
///
/// Every pair `(a, b)` describes a rotation combined with a uniform scale
/// (plus translation), so a similarity can never shear: a disc mapped through
/// one stays a disc. Any operation whose intermediate or final value would
/// overflow returns `None` rather than wrapping. Use
/// [`Similarity::to_affine`] where a general [`Affine`] is genuinely wanted.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Similarity {
    /// Linear entry shared by output `x` from input `x` and output `y` from input `y`.
    pub a: Ratio,
    /// Linear entry mapping input `y` to output `x`; its negation maps input `x` to output `y`.
    pub b: Ratio,
    /// Translation added to output `x`.
    pub tx: Ratio,
    /// Translation added to output `y`.
    pub ty: Ratio,
}

impl Similarity {
    /// Creates a new [`Similarity`] from its linear entries and translation.
    ///
    /// Any `(a, b)` pair is a similarity (rotation with uniform scale), so
    /// this constructor cannot introduce shear.
    #[must_use]
    pub const fn new(a: Ratio, b: Ratio, tx: Ratio, ty: Ratio) -> Self {
        Self { a, b, tx, ty }
    }

    /// Returns the identity similarity.
    #[must_use]
    pub fn identity() -> Self {
        let zero = int_ratio(0);
        let one = int_ratio(1);
        Self {
            a: one,
            b: zero,
            tx: zero,
            ty: zero,
        }
    }

    /// Returns a pure translation by the given displacement.
    #[must_use]
    pub fn translation(displacement: Vector) -> Self {
        let zero = int_ratio(0);
        let one = int_ratio(1);
        Self {
            a: one,
            b: zero,
            tx: displacement.x,
            ty: displacement.y,
        }
    }

    /// Returns a uniform scaling by the given factor on both axes.
    #[must_use]
    pub fn uniform_scale(factor: Ratio) -> Self {
        let zero = int_ratio(0);
        Self {
            a: factor,
            b: zero,
            tx: zero,
            ty: zero,
        }
    }

    /// Returns a rotation about the origin by `turns` quarter turns.
    ///
    /// Positive values rotate counter-clockwise in a standard mathematical
    /// frame (`x` right, `y` up), matching [`Affine::quarter_turns`]: one
    /// quarter turn maps `(x, y)` to `(-y, x)`. The turn count is reduced
    /// modulo 4 with Euclidean remainder, so the rotation is exact.
    #[must_use]
    pub fn quarter_turns(turns: i32) -> Self {
        let zero = int_ratio(0);
        let one = int_ratio(1);
        let neg_one = int_ratio(-1);
        let step = turns.rem_euclid(4);
        if step == 0 {
            Self {
                a: one,
                b: zero,
                tx: zero,
                ty: zero,
            }
        } else if step == 1 {
            Self {
                a: zero,
                b: neg_one,
                tx: zero,
                ty: zero,
            }
        } else if step == 2 {
            Self {
                a: neg_one,
                b: zero,
                tx: zero,
                ty: zero,
            }
        } else {
            Self {
                a: zero,
                b: one,
                tx: zero,
                ty: zero,
            }
        }
    }

    /// Applies this similarity to a point.
    ///
    /// Returns `None` if any intermediate or final value overflows.
    #[must_use]
    pub fn apply(self, point: Point) -> Option<Point> {
        let neg_b = self.b.checked_neg()?;
        let ax = self.a.checked_mul(point.x)?;
        let by = self.b.checked_mul(point.y)?;
        let x_lin = ax.checked_add(by)?;
        let x = x_lin.checked_add(self.tx)?;
        let nx = neg_b.checked_mul(point.x)?;
        let ay = self.a.checked_mul(point.y)?;
        let y_lin = nx.checked_add(ay)?;
        let y = y_lin.checked_add(self.ty)?;
        Some(Point { x, y })
    }

    /// Applies only the linear part of this similarity to a vector.
    ///
    /// Translation is ignored. Returns `None` on overflow.
    #[must_use]
    pub fn apply_vector(self, vector: Vector) -> Option<Vector> {
        let neg_b = self.b.checked_neg()?;
        let ax = self.a.checked_mul(vector.x)?;
        let by = self.b.checked_mul(vector.y)?;
        let x = ax.checked_add(by)?;
        let nx = neg_b.checked_mul(vector.x)?;
        let ay = self.a.checked_mul(vector.y)?;
        let y = nx.checked_add(ay)?;
        Some(Vector { x, y })
    }

    /// Composes two similarities.
    ///
    /// `self.compose(other)` returns the similarity that applies `other`
    /// first and then `self`. Similarities are closed under composition, so
    /// the result is again a similarity. Returns `None` on overflow.
    #[must_use]
    pub fn compose(self, other: Self) -> Option<Self> {
        let a_part = self.a.checked_mul(other.a)?;
        let b_part = self.b.checked_mul(other.b)?;
        let a = a_part.checked_sub(b_part)?;
        let c_part = self.a.checked_mul(other.b)?;
        let d_part = self.b.checked_mul(other.a)?;
        let b = c_part.checked_add(d_part)?;
        let ax = self.a.checked_mul(other.tx)?;
        let by = self.b.checked_mul(other.ty)?;
        let tx = ax.checked_add(by)?.checked_add(self.tx)?;
        let neg_b = self.b.checked_neg()?;
        let nx = neg_b.checked_mul(other.tx)?;
        let ay = self.a.checked_mul(other.ty)?;
        let ty = nx.checked_add(ay)?.checked_add(self.ty)?;
        Some(Self { a, b, tx, ty })
    }

    /// Returns the similarity applied after `self`.
    ///
    /// `self.then(next)` equals `next.compose(self)`: apply `self` first,
    /// then `next`. Returns `None` on overflow.
    #[must_use]
    pub fn then(self, next: Self) -> Option<Self> {
        next.compose(self)
    }

    /// Inverts this similarity.
    ///
    /// Returns `None` when the linear part is singular (both `a` and `b`
    /// are zero, so the determinant `a * a + b * b` is zero) or when any
    /// intermediate or final value overflows.
    #[must_use]
    pub fn inverse(self) -> Option<Self> {
        let aa = self.a.checked_mul(self.a)?;
        let bb = self.b.checked_mul(self.b)?;
        let det = aa.checked_add(bb)?;
        if det == int_ratio(0) {
            return None;
        }
        let scale = int_ratio(1).checked_div(det)?;
        let a_inv = self.a.checked_mul(scale)?;
        let b_inv = self.b.checked_neg()?.checked_mul(scale)?;
        let fwd_x = a_inv
            .checked_mul(self.tx)?
            .checked_add(b_inv.checked_mul(self.ty)?)?;
        let back = b_inv.checked_neg()?;
        let fwd_y = back
            .checked_mul(self.tx)?
            .checked_add(a_inv.checked_mul(self.ty)?)?;
        Some(Self {
            a: a_inv,
            b: b_inv,
            tx: fwd_x.checked_neg()?,
            ty: fwd_y.checked_neg()?,
        })
    }

    /// Converts this similarity to the general [`Affine`] transform.
    ///
    /// The affine entries are `(a, b, tx, -b, a, ty)`. Returns `None` only
    /// when negating `b` overflows.
    #[must_use]
    pub fn to_affine(self) -> Option<Affine> {
        let neg_b = self.b.checked_neg()?;
        Some(Affine::new(self.a, self.b, self.tx, neg_b, self.a, self.ty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::make_ratio;

    #[test]
    fn test_affine_apply_compose_inverse() {
        let Some(x) = make_ratio(3, 2) else { return };
        let Some(y) = make_ratio(-5, 4) else { return };
        let probe = Point::new(x, y);
        let same = Affine::identity().apply(probe);
        assert_eq!(same, Some(probe), "identity must map a point to itself");
        let Some(dx) = make_ratio(2, 1) else { return };
        let Some(dy) = make_ratio(3, 1) else { return };
        let shift = Affine::translation(Vector::new(dx, dy));
        let Some(moved) = shift.apply(probe) else {
            return;
        };
        let Some(expect_x) = x.checked_add(dx) else {
            return;
        };
        let Some(expect_y) = y.checked_add(dy) else {
            return;
        };
        assert_eq!(
            moved,
            Point::new(expect_x, expect_y),
            "translation must add the displacement"
        );
        let Some(back) = shift.inverse() else { return };
        let Some(there) = shift.apply(probe) else {
            return;
        };
        let Some(roundtrip) = back.apply(there) else {
            return;
        };
        assert_eq!(
            roundtrip, probe,
            "inverse must undo the translation exactly"
        );
        let Some(there_and_back) = shift.then(back) else {
            return;
        };
        assert_eq!(
            there_and_back,
            Affine::identity(),
            "a transform followed by its inverse must be identity"
        );
        let Some(sx) = make_ratio(2, 1) else { return };
        let Some(sy) = make_ratio(3, 1) else { return };
        let scale = Affine::scaling(sx, sy);
        let Some(scaled_x) = x.checked_mul(sx) else {
            return;
        };
        let Some(scaled_y) = y.checked_mul(sy) else {
            return;
        };
        let Some(scaled) = scale.apply_vector(Vector::new(x, y)) else {
            return;
        };
        assert_eq!(
            scaled,
            Vector::new(scaled_x, scaled_y),
            "scaling must multiply vector components"
        );
        let Some(vec_moved) = shift.apply_vector(Vector::new(x, y)) else {
            return;
        };
        assert_eq!(
            vec_moved,
            Vector::new(x, y),
            "translation must not affect vectors"
        );
    }

    #[test]
    fn test_affine_quarter_turns() {
        let Some(one) = make_ratio(1, 1) else { return };
        let Some(zero) = make_ratio(0, 1) else { return };
        let Some(neg_one) = make_ratio(-1, 1) else {
            return;
        };
        let probe = Point::new(one, zero);
        let Some(first) = Affine::quarter_turns(1).apply(probe) else {
            return;
        };
        assert_eq!(
            first,
            Point::new(zero, one),
            "one quarter turn must map (1, 0) to (0, 1)"
        );
        let Some(second) = Affine::quarter_turns(2).apply(probe) else {
            return;
        };
        assert_eq!(
            second,
            Point::new(neg_one, zero),
            "two quarter turns must map (1, 0) to (-1, 0)"
        );
        let Some(third) = Affine::quarter_turns(3).apply(probe) else {
            return;
        };
        assert_eq!(
            third,
            Point::new(zero, neg_one),
            "three quarter turns must map (1, 0) to (0, -1)"
        );
        let Some(full) = Affine::quarter_turns(4).apply(probe) else {
            return;
        };
        assert_eq!(full, probe, "four quarter turns must be identity");
        let Some(backward) = Affine::quarter_turns(-1).apply(probe) else {
            return;
        };
        assert_eq!(
            backward, third,
            "minus one quarter turn must equal three forward turns"
        );
    }

    #[test]
    fn test_similarity_roundtrip_and_affine() {
        let Some(one) = make_ratio(1, 1) else { return };
        let Some(zero) = make_ratio(0, 1) else { return };
        let Some(two) = make_ratio(2, 1) else { return };
        let Some(three) = make_ratio(3, 1) else {
            return;
        };
        let probe = Point::new(two, three);
        let shift = Similarity::translation(Vector::new(two, three));
        let Some(moved) = shift.apply(probe) else {
            return;
        };
        let Some(back) = shift.inverse() else { return };
        let Some(home) = back.apply(moved) else {
            return;
        };
        assert_eq!(home, probe, "inverse must undo a translation exactly");
        let Some(spun) = Similarity::quarter_turns(1).apply(Point::new(one, zero)) else {
            return;
        };
        assert_eq!(
            spun,
            Point::new(zero, one),
            "a similarity quarter turn must match the affine one"
        );
        let scaled = Similarity::uniform_scale(two);
        let Some(wide) = scaled.apply_vector(Vector::new(one, zero)) else {
            return;
        };
        assert_eq!(
            wide,
            Vector::new(two, zero),
            "uniform scale must multiply both axes equally"
        );
        let Some(as_affine) = scaled.to_affine() else {
            return;
        };
        assert_eq!(
            as_affine.d, as_affine.a,
            "a similarity affine must scale uniformly"
        );
    }
}
