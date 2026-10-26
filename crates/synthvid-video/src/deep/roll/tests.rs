use super::*;
use core::num::NonZeroI64;
use synthvid_scene::Numerator;
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

/// A roll of the given fraction of a turn, `numerator / denominator` turns.
fn roll(numerator: Numerator, denominator: NonZeroI64) -> Result<Turns, Fail> {
    Ok(Turns::new(Ratio::new(numerator, denominator).or_fail()?))
}

/// An exact value of the given fraction, for the denominator rule of [`scale_to_trig_denom`].
fn fraction(numerator: Numerator, denominator: NonZeroI64) -> Result<Ratio, Fail> {
    Ratio::new(numerator, denominator).or_fail()
}

#[test]
fn no_roll_has_unit_cosine_and_zero_sine() {
    assert_eq!(check_no_roll_has_unit_cosine_and_zero_sine(), Ok(()));
}

fn check_no_roll_has_unit_cosine_and_zero_sine() -> Result<(), Fail> {
    let pair = TrigPair::of(roll(Numerator(0), NonZeroI64::new(1).or_fail()?)?).or_fail()?;
    ensure_eq!(pair.cosine(), TrigNumerator(0x8000));
    ensure_eq!(pair.sine(), TrigNumerator(0));
    Ok(())
}

#[test]
fn quarter_turn_roll_has_zero_cosine_and_negative_unit_sine() {
    assert_eq!(
        check_quarter_turn_roll_has_zero_cosine_and_negative_unit_sine(),
        Ok(())
    );
}

fn check_quarter_turn_roll_has_zero_cosine_and_negative_unit_sine() -> Result<(), Fail> {
    let pair = TrigPair::of(roll(Numerator(1), NonZeroI64::new(4).or_fail()?)?).or_fail()?;
    ensure_eq!(pair.cosine(), TrigNumerator(0));
    ensure_eq!(pair.sine(), TrigNumerator(-0x8000));
    Ok(())
}

#[test]
fn negative_quarter_turn_roll_has_positive_unit_sine() {
    assert_eq!(
        check_negative_quarter_turn_roll_has_positive_unit_sine(),
        Ok(())
    );
}

fn check_negative_quarter_turn_roll_has_positive_unit_sine() -> Result<(), Fail> {
    let pair = TrigPair::of(roll(Numerator(-1), NonZeroI64::new(4).or_fail()?)?).or_fail()?;
    ensure_eq!(pair.cosine(), TrigNumerator(0));
    ensure_eq!(pair.sine(), TrigNumerator(0x8000));
    Ok(())
}

#[test]
fn half_turn_roll_negates_the_cosine_and_zeroes_the_sine() {
    assert_eq!(
        check_half_turn_roll_negates_the_cosine_and_zeroes_the_sine(),
        Ok(())
    );
}

fn check_half_turn_roll_negates_the_cosine_and_zeroes_the_sine() -> Result<(), Fail> {
    let pair = TrigPair::of(roll(Numerator(1), NonZeroI64::new(2).or_fail()?)?).or_fail()?;
    ensure_eq!(pair.cosine(), TrigNumerator(-0x8000));
    ensure_eq!(pair.sine(), TrigNumerator(0));
    Ok(())
}

#[test]
fn value_with_a_denominator_that_divides_2_to_the_15_is_rescaled() {
    assert_eq!(
        check_value_with_a_denominator_that_divides_2_to_the_15_is_rescaled(),
        Ok(())
    );
}

fn check_value_with_a_denominator_that_divides_2_to_the_15_is_rescaled() -> Result<(), Fail> {
    let scaled = scale_to_trig_denom(fraction(Numerator(3), NonZeroI64::new(2).or_fail()?)?);
    ensure_eq!(scaled, Ok(TrigNumerator(0xC000)));
    Ok(())
}

#[test]
fn value_with_a_denominator_that_does_not_divide_2_to_the_15_is_overflow() {
    assert_eq!(
        check_value_with_a_denominator_that_does_not_divide_2_to_the_15_is_overflow(),
        Ok(())
    );
}

fn check_value_with_a_denominator_that_does_not_divide_2_to_the_15_is_overflow() -> Result<(), Fail>
{
    let third = scale_to_trig_denom(fraction(Numerator(1), NonZeroI64::new(3).or_fail()?)?);
    ensure_eq!(third, Err(ProjectionError::Overflow));
    let beyond = scale_to_trig_denom(fraction(
        Numerator(1),
        NonZeroI64::new(0x0001_0000).or_fail()?,
    )?);
    ensure_eq!(beyond, Err(ProjectionError::Overflow));
    Ok(())
}

#[test]
fn trig_numerator_widens_without_change() {
    assert_eq!(check_trig_numerator_widens_without_change(), Ok(()));
}

fn check_trig_numerator_widens_without_change() -> Result<(), Fail> {
    ensure_eq!(WideInt::from(TrigNumerator(-0x8000)), WideInt(-0x8000));
    Ok(())
}
