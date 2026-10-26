use super::*;
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

/// A quotient to round, named by its numerator and denominator so the two cannot be swapped.
#[derive(Copy, Clone)]
struct Quotient {
    /// The value divided by the denominator.
    numerator: WideInt,
    /// The value that divides the numerator, which must be non-zero.
    denominator: WideInt,
}

/// A rounding of the quotient, with the denominator checked to be non-zero.
fn fraction(quotient: Quotient) -> Result<RoundingInput, Fail> {
    Ok(RoundingInput {
        numerator: quotient.numerator,
        denominator: NonZeroWideInt::new(quotient.denominator).or_fail()?,
    })
}

#[test]
fn three_halves_rounds_up_to_two() {
    assert_eq!(check_three_halves_rounds_up_to_two(), Ok(()));
}

fn check_three_halves_rounds_up_to_two() -> Result<(), Fail> {
    let rounded = round_half_up(fraction(Quotient {
        numerator: WideInt(3),
        denominator: WideInt(2),
    })?);
    ensure_eq!(rounded, Ok(WideInt(2)));
    Ok(())
}

#[test]
fn negative_three_halves_rounds_up_to_minus_one() {
    assert_eq!(check_negative_three_halves_rounds_up_to_minus_one(), Ok(()));
}

fn check_negative_three_halves_rounds_up_to_minus_one() -> Result<(), Fail> {
    let rounded = round_half_up(fraction(Quotient {
        numerator: WideInt(-3),
        denominator: WideInt(2),
    })?);
    ensure_eq!(rounded, Ok(WideInt(-1)));
    Ok(())
}

#[test]
fn five_halves_rounds_up_to_three() {
    assert_eq!(check_five_halves_rounds_up_to_three(), Ok(()));
}

fn check_five_halves_rounds_up_to_three() -> Result<(), Fail> {
    let rounded = round_half_up(fraction(Quotient {
        numerator: WideInt(5),
        denominator: WideInt(2),
    })?);
    ensure_eq!(rounded, Ok(WideInt(3)));
    Ok(())
}

#[test]
fn negative_five_halves_rounds_up_to_minus_two() {
    assert_eq!(check_negative_five_halves_rounds_up_to_minus_two(), Ok(()));
}

fn check_negative_five_halves_rounds_up_to_minus_two() -> Result<(), Fail> {
    let rounded = round_half_up(fraction(Quotient {
        numerator: WideInt(-5),
        denominator: WideInt(2),
    })?);
    ensure_eq!(rounded, Ok(WideInt(-2)));
    Ok(())
}

#[test]
fn seven_thirds_rounds_down_to_two() {
    assert_eq!(check_seven_thirds_rounds_down_to_two(), Ok(()));
}

fn check_seven_thirds_rounds_down_to_two() -> Result<(), Fail> {
    let rounded = round_half_up(fraction(Quotient {
        numerator: WideInt(7),
        denominator: WideInt(3),
    })?);
    ensure_eq!(rounded, Ok(WideInt(2)));
    Ok(())
}

#[test]
fn negative_seven_thirds_rounds_down_to_minus_two() {
    assert_eq!(
        check_negative_seven_thirds_rounds_down_to_minus_two(),
        Ok(())
    );
}

fn check_negative_seven_thirds_rounds_down_to_minus_two() -> Result<(), Fail> {
    let rounded = round_half_up(fraction(Quotient {
        numerator: WideInt(-7),
        denominator: WideInt(3),
    })?);
    ensure_eq!(rounded, Ok(WideInt(-2)));
    Ok(())
}

#[test]
fn negative_denominator_keeps_the_value_of_the_fraction() {
    assert_eq!(
        check_negative_denominator_keeps_the_value_of_the_fraction(),
        Ok(())
    );
}

fn check_negative_denominator_keeps_the_value_of_the_fraction() -> Result<(), Fail> {
    let three_over_minus_two = round_half_up(fraction(Quotient {
        numerator: WideInt(3),
        denominator: WideInt(-2),
    })?);
    ensure_eq!(three_over_minus_two, Ok(WideInt(-1)));
    let minus_three_over_minus_two = round_half_up(fraction(Quotient {
        numerator: WideInt(-3),
        denominator: WideInt(-2),
    })?);
    ensure_eq!(minus_three_over_minus_two, Ok(WideInt(2)));
    Ok(())
}

#[test]
fn exact_quotient_is_unchanged() {
    assert_eq!(check_exact_quotient_is_unchanged(), Ok(()));
}

fn check_exact_quotient_is_unchanged() -> Result<(), Fail> {
    let rounded = round_half_up(fraction(Quotient {
        numerator: WideInt(-12),
        denominator: WideInt(3),
    })?);
    ensure_eq!(rounded, Ok(WideInt(-4)));
    Ok(())
}

#[test]
fn zero_denominator_is_refused_as_overflow() {
    assert_eq!(check_zero_denominator_is_refused_as_overflow(), Ok(()));
}

fn check_zero_denominator_is_refused_as_overflow() -> Result<(), Fail> {
    let denominator = NonZeroWideInt::new(WideInt(0));
    ensure_eq!(denominator, Err(ProjectionError::Overflow));
    Ok(())
}

#[test]
fn product_beyond_i128_is_overflow() {
    assert_eq!(check_product_beyond_i128_is_overflow(), Ok(()));
}

fn check_product_beyond_i128_is_overflow() -> Result<(), Fail> {
    let refused = WideInt(i128::MAX).times(WideInt(2));
    ensure_eq!(refused, Err(ProjectionError::Overflow));
    Ok(())
}
