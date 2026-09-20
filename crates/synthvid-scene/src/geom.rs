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
}
