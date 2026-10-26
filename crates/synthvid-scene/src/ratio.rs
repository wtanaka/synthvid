//! Exact rational arithmetic.
//!
//! [`Ratio`] is the numeric type every other module computes in: a normalized
//! `i64 / NonZeroI64` fraction whose operations are checked and exact, so a
//! value never drifts between platforms or releases.

use core::cmp::Ordering;
use core::error::Error;
use core::fmt;
use core::num::{NonZeroI128, NonZeroI64, NonZeroU8, TryFromIntError};

use crate::trig::{cos_turns, sin_turns};

/// Exact arithmetic could not represent a result.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Overflow;

impl fmt::Display for Overflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exact arithmetic could not represent a result")
    }
}

impl Error for Overflow {}

/// A value that does not fit the narrower integer type it is converted to is
/// a result that type cannot represent. [`TryFromIntError`] carries nothing
/// beyond that fact, so the conversion loses no information.
impl From<TryFromIntError> for Overflow {
    fn from(_: TryFromIntError) -> Self {
        Self
    }
}

/// Constructs a `NonZeroI64` with value 1.
///
/// Uses `NonZeroU8::MIN` (which is 1) and converts it via `From`, which is
/// infallible for unsigned non-zero types. This avoids any `Option` or fallible
/// construction.
fn nonzero_one() -> NonZeroI64 {
    NonZeroI64::from(NonZeroU8::MIN)
}

/// An unsigned 128-bit magnitude, before reduction to lowest terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Magnitude(u128);

/// The numerator of a [`Ratio`], held apart from its denominator.
///
/// Every `i64` is a valid numerator; a whole number `n` is the numerator of
/// `n / 1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Numerator(pub i64);

/// The unreduced parts of a rational: two magnitudes and the sign they share.
#[derive(Clone, Copy, Debug)]
struct Parts {
    /// The absolute value of the numerator.
    num: Magnitude,
    /// The absolute value of the denominator.
    den: Magnitude,
    /// The sign of the whole rational.
    sign: Sign,
}

/// Greatest common divisor of the two magnitudes in `pair`.
const fn gcd(pair: (Magnitude, Magnitude)) -> Magnitude {
    let (Magnitude(mut dividend), Magnitude(mut divisor)) = pair;
    while divisor != 0 {
        let rem = match dividend.checked_rem(divisor) {
            Some(remainder) => remainder,
            None => 0,
        };
        dividend = divisor;
        divisor = rem;
    }
    Magnitude(dividend)
}

/// Normalizes unreduced [`Parts`] into a canonical [`Ratio`].
///
/// # Errors
///
/// Returns `Err(Overflow)` if the denominator magnitude is 0 or if the resulting
/// reduced rational cannot be represented with an `i64` numerator and positive
/// [`NonZeroI64`] denominator.
fn from_parts(parts: Parts) -> Result<Ratio, Overflow> {
    let Parts { num, den, sign } = parts;
    let Magnitude(abs_num) = num;
    let Magnitude(abs_den) = den;
    if abs_den == 0 {
        return Err(Overflow);
    }
    if abs_num == 0 {
        return Ok(Ratio {
            numer: Numerator(0),
            denom: nonzero_one(),
        });
    }

    let Magnitude(divisor) = gcd((num, den));
    if divisor == 0 {
        return Err(Overflow);
    }
    let red_num = abs_num.checked_div(divisor).ok_or(Overflow)?;
    let red_den = abs_den.checked_div(divisor).ok_or(Overflow)?;

    // Denominator must be strictly positive and fit in positive i64.
    let den_i64 = i64::try_from(red_den).map_err(Overflow::from)?;
    if den_i64 <= 0 {
        return Err(Overflow);
    }
    let non_zero_den = NonZeroI64::new(den_i64).ok_or(Overflow)?;

    // Magnitude of i64::MIN is 2^63.
    let min_i64_mag = u128::from(i64::MIN.unsigned_abs());

    let numer_i64 = if sign == Sign::Negative {
        match red_num.cmp(&min_i64_mag) {
            Ordering::Equal => i64::MIN,
            Ordering::Less => {
                let pos = i64::try_from(red_num).map_err(Overflow::from)?;
                0_i64.checked_sub(pos).ok_or(Overflow)?
            }
            Ordering::Greater => return Err(Overflow),
        }
    } else {
        i64::try_from(red_num).map_err(Overflow::from)?
    };

    Ok(Ratio {
        numer: Numerator(numer_i64),
        denom: non_zero_den,
    })
}

/// The sign of a rational, carried apart from its magnitude.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Sign {
    /// Strictly below zero.
    Negative,
    /// Zero or above.
    NonNegative,
}

/// A normalized rational number over 64-bit signed integers.
///
/// The denominator is guaranteed to be strictly positive ([`NonZeroI64`]) and
/// any sign is carried entirely in the numerator. Ratios are always kept in
/// lowest terms (`gcd(|numerator|, denominator) == 1`). Zero is uniquely
/// represented as `0 / 1`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Ratio {
    /// The numerator of the rational number in lowest terms.
    numer: Numerator,
    /// The denominator, strictly positive and in lowest terms.
    denom: NonZeroI64,
}

impl Ratio {
    /// Creates a normalized rational from a numerator and a non-zero denominator.
    ///
    /// Automatically reduces the rational to lowest terms and moves any sign to the numerator.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if the reduced rational cannot fit within `i64` bounds.
    pub fn new(numer: Numerator, denom: NonZeroI64) -> Result<Self, Overflow> {
        let num128 = i128::from(numer.0);
        let den128 = NonZeroI128::from(denom).get();
        let sign = if (num128 < 0) == (den128 < 0) {
            Sign::NonNegative
        } else {
            Sign::Negative
        };
        from_parts(Parts {
            num: Magnitude(num128.unsigned_abs()),
            den: Magnitude(den128.unsigned_abs()),
            sign,
        })
    }

    /// Creates a rational representing the integer `n / 1`.
    ///
    /// `gcd(n, 1) == 1`, so `n / 1` is already in lowest terms and needs no
    /// reduction. The denominator is built without an `Option` to discharge,
    /// so this construction is total for every `i64`, including `i64::MIN`.
    #[must_use]
    pub fn from_integer(numer: Numerator) -> Self {
        Self {
            numer,
            denom: nonzero_one(),
        }
    }

    /// Returns the numerator of the rational number.
    #[must_use]
    pub const fn numer(self) -> Numerator {
        self.numer
    }

    /// Returns the positive, non-zero denominator of the rational number.
    #[must_use]
    pub const fn denom(self) -> NonZeroI64 {
        self.denom
    }

    /// Checked addition.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if intermediate or final values overflow `i64`.
    pub fn checked_add(self, rhs: Self) -> Result<Self, Overflow> {
        let n1 = i128::from(self.numer.0);
        let d1 = NonZeroI128::from(self.denom).get();
        let n2 = i128::from(rhs.numer.0);
        let d2 = NonZeroI128::from(rhs.denom).get();

        let term1 = n1.checked_mul(d2).ok_or(Overflow)?;
        let term2 = n2.checked_mul(d1).ok_or(Overflow)?;
        let num128 = term1.checked_add(term2).ok_or(Overflow)?;
        let den128 = d1.checked_mul(d2).ok_or(Overflow)?;

        let sign = if num128 < 0 {
            Sign::Negative
        } else {
            Sign::NonNegative
        };
        from_parts(Parts {
            num: Magnitude(num128.unsigned_abs()),
            den: Magnitude(den128.unsigned_abs()),
            sign,
        })
    }

    /// Checked subtraction.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if intermediate or final values overflow `i64`.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, Overflow> {
        let n1 = i128::from(self.numer.0);
        let d1 = NonZeroI128::from(self.denom).get();
        let n2 = i128::from(rhs.numer.0);
        let d2 = NonZeroI128::from(rhs.denom).get();

        let term1 = n1.checked_mul(d2).ok_or(Overflow)?;
        let term2 = n2.checked_mul(d1).ok_or(Overflow)?;
        let num128 = term1.checked_sub(term2).ok_or(Overflow)?;
        let den128 = d1.checked_mul(d2).ok_or(Overflow)?;

        let sign = if num128 < 0 {
            Sign::Negative
        } else {
            Sign::NonNegative
        };
        from_parts(Parts {
            num: Magnitude(num128.unsigned_abs()),
            den: Magnitude(den128.unsigned_abs()),
            sign,
        })
    }

    /// Checked multiplication.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if intermediate or final values overflow `i64`.
    pub fn checked_mul(self, rhs: Self) -> Result<Self, Overflow> {
        let n1 = i128::from(self.numer.0);
        let d1 = NonZeroI128::from(self.denom).get();
        let n2 = i128::from(rhs.numer.0);
        let d2 = NonZeroI128::from(rhs.denom).get();

        let num128 = n1.checked_mul(n2).ok_or(Overflow)?;
        let den128 = d1.checked_mul(d2).ok_or(Overflow)?;

        let sign = if (num128 < 0) == (den128 < 0) {
            Sign::NonNegative
        } else {
            Sign::Negative
        };
        from_parts(Parts {
            num: Magnitude(num128.unsigned_abs()),
            den: Magnitude(den128.unsigned_abs()),
            sign,
        })
    }

    /// Checked division.
    ///
    /// # Errors
    ///
    /// Returns [`Overflow`] if the divisor is zero or division overflows.
    pub fn checked_div(self, rhs: Self) -> Result<Self, Overflow> {
        if rhs.numer.0 == 0 {
            return Err(Overflow);
        }
        let n1 = i128::from(self.numer.0);
        let d1 = NonZeroI128::from(self.denom).get();
        let n2 = i128::from(rhs.numer.0);
        let d2 = NonZeroI128::from(rhs.denom).get();

        let num128 = n1.checked_mul(d2).ok_or(Overflow)?;
        let den128 = d1.checked_mul(n2).ok_or(Overflow)?;

        let sign = if (num128 < 0) == (den128 < 0) {
            Sign::NonNegative
        } else {
            Sign::Negative
        };
        from_parts(Parts {
            num: Magnitude(num128.unsigned_abs()),
            den: Magnitude(den128.unsigned_abs()),
            sign,
        })
    }

    /// Checked negation.
    ///
    /// # Errors
    ///
    /// Returns [`Overflow`] if negation overflows.
    pub fn checked_neg(self) -> Result<Self, Overflow> {
        let neg_num = 0_i64.checked_sub(self.numer.0).ok_or(Overflow)?;
        Ok(Self {
            numer: Numerator(neg_num),
            denom: self.denom,
        })
    }

    /// Computes the sine of this angle given in turns.
    ///
    /// See [`sin_turns`] for precision, error bounds, and algorithm details.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if any intermediate arithmetic overflows.
    pub fn sin_turns(self) -> Result<Self, Overflow> {
        sin_turns(self)
    }

    /// Computes the cosine of this angle given in turns.
    ///
    /// See [`cos_turns`] for precision, error bounds, and algorithm details.
    ///
    /// # Errors
    ///
    /// Returns `Err(Overflow)` if any intermediate arithmetic overflows.
    pub fn cos_turns(self) -> Result<Self, Overflow> {
        cos_turns(self)
    }
}

impl Ord for Ratio {
    fn cmp(&self, other: &Self) -> Ordering {
        // Since both denominators are strictly positive, a/b <=> c/d
        // is equivalent to a*d <=> c*b.
        // i64 * i64 products never overflow i128.
        let left = i128::from(self.numer.0).checked_mul(NonZeroI128::from(other.denom).get());
        let right = i128::from(other.numer.0).checked_mul(NonZeroI128::from(self.denom).get());
        match (left, right) {
            (Some(left_product), Some(right_product)) => left_product.cmp(&right_product),
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for Ratio {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Ratio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numer.0, self.denom)
    }
}

pub mod helpers;
pub(crate) use helpers::{
    ceil_ratio, column_start, derive_stream_seed, floor_ratio, half_ratio, irwin_hall_sample,
    pixel_center_ratio, quarter_ratio, row_start, sample_axis, track_axis, GridShift,
};

#[cfg(test)]
mod tests;
