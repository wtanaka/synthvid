//! The cosine and sine of a camera's roll, as exact numerators over 2^15.
//!
//! A camera rolled by `theta` turns rotates the offsets of a point with the matrix
//! `[[cosine, -sine], [sine, cosine]]`, where `cosine` and `sine` are the cosine and sine of
//! `-theta`. That is the rotation the scene's view transform applies to its objects, so a roll of
//! a quarter turn maps the offset `(10, -5)` to `(-5, -10)`: the cosine is 0 and the sine is -1.

use synthvid_scene::{cos_turns, sin_turns, Ratio, Turns};

use super::error::ProjectionError;
use super::round::WideInt;

/// The denominator of every trigonometric numerator, 2^15.
///
/// The denominator of an exact value must divide this, so a value is one whole number of
/// 1/32768 turns of the unit circle.
const TRIG_DENOM: i64 = 0x8000;

/// The numerator of an exact trigonometric value over 2^15.
///
/// The value `TrigNumerator(32768)` is 1 and `TrigNumerator(-32768)` is -1.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TrigNumerator(pub i64);

impl From<TrigNumerator> for WideInt {
    fn from(numerator: TrigNumerator) -> Self {
        Self(i128::from(numerator.0))
    }
}

/// The cosine and sine of a camera's roll, each as a [`TrigNumerator`].
///
/// The fields are private, so a pair can only come from [`TrigPair::of`], which takes the
/// values from the trigonometry of the scene.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct TrigPair {
    /// The cosine of the negated roll angle, over 2^15.
    cosine: TrigNumerator,
    /// The sine of the negated roll angle, over 2^15.
    sine: TrigNumerator,
}

impl TrigPair {
    /// Takes the cosine and sine of the negated roll angle, the rotation applied to offsets.
    ///
    /// # Errors
    ///
    /// Returns `Err(ProjectionError::Overflow)` if an exact value has a denominator that does
    /// not divide 2^15, or if the scaled numerator does not fit an `i64`.
    pub fn of(roll: Turns) -> Result<Self, ProjectionError> {
        let turn = roll.get().checked_neg()?;
        let cosine = scale_to_trig_denom(cos_turns(turn)?)?;
        let sine = scale_to_trig_denom(sin_turns(turn)?)?;
        Ok(Self { cosine, sine })
    }

    /// Returns the cosine of the negated roll angle, over 2^15.
    #[must_use]
    pub const fn cosine(self) -> TrigNumerator {
        self.cosine
    }

    /// Returns the sine of the negated roll angle, over 2^15.
    #[must_use]
    pub const fn sine(self) -> TrigNumerator {
        self.sine
    }
}

/// Rewrites an exact value as its numerator over 2^15.
///
/// # Errors
///
/// Returns `Err(ProjectionError::Overflow)` if the denominator of `value` does not divide 2^15,
/// since such a value has no exact numerator over 2^15, or if the numerator does not fit an
/// `i64`.
fn scale_to_trig_denom(value: Ratio) -> Result<TrigNumerator, ProjectionError> {
    let denominator = value.denom().get();
    let remainder = TRIG_DENOM
        .checked_rem(denominator)
        .ok_or(ProjectionError::Overflow)?;
    if remainder != 0 {
        return Err(ProjectionError::Overflow);
    }
    let factor = TRIG_DENOM
        .checked_div(denominator)
        .ok_or(ProjectionError::Overflow)?;
    let scaled = value
        .numer()
        .0
        .checked_mul(factor)
        .ok_or(ProjectionError::Overflow)?;
    Ok(TrigNumerator(scaled))
}

#[cfg(test)]
mod tests;
