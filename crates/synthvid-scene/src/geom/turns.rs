//! The four rotations a quarter-turn transform can take.
//!
//! A quarter turn is a closed set of four values, so it is an enum rather
//! than an integer that every caller must reduce modulo four.

/// A rotation about the origin by a whole number of quarter turns.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum QuarterTurn {
    /// No rotation.
    Zero,
    /// One quarter turn, mapping `(x, y)` to `(-y, x)`.
    One,
    /// Two quarter turns, mapping `(x, y)` to `(-x, -y)`.
    Two,
    /// Three quarter turns, mapping `(x, y)` to `(y, -x)`.
    Three,
}
