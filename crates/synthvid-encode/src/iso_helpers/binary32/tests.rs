use super::*;
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

#[test]
fn binary32_of_one_third_rounds_its_significand_up() {
    assert_eq!(
        check_binary32_of_one_third_rounds_its_significand_up(),
        Ok(())
    );
}

fn check_binary32_of_one_third_rounds_its_significand_up() -> Result<(), Fail> {
    // 1/3 = 2^-2 * 1.3333...: the fraction 0x2AAAAA.AA... rounds up to 0x2AAAAB.
    let bits = binary32_bits(
        Numerator(NonZeroU32::new(1).or_fail()?),
        NonZeroU32::new(3).or_fail()?,
    )
    .or_fail()?;
    ensure_eq!(bits.0, 0x3EAA_AAAB);
    Ok(())
}

#[test]
fn binary32_ties_round_to_even() {
    assert_eq!(check_binary32_ties_round_to_even(), Ok(()));
}

fn check_binary32_ties_round_to_even() -> Result<(), Fail> {
    // 1 + 2^-24 is exactly halfway between 1 and the next float; 1 is even, so it stays.
    let bits = binary32_bits(
        Numerator(NonZeroU32::new(0x0100_0001).or_fail()?),
        NonZeroU32::new(0x0100_0000).or_fail()?,
    )
    .or_fail()?;
    ensure_eq!(bits.0, 0x3F80_0000);
    Ok(())
}

#[test]
fn binary32_carry_out_of_the_significand_moves_the_exponent() {
    assert_eq!(
        check_binary32_carry_out_of_the_significand_moves_the_exponent(),
        Ok(())
    );
}

fn check_binary32_carry_out_of_the_significand_moves_the_exponent() -> Result<(), Fail> {
    // 2 - 2^-25 rounds up to 2.0: the significand carries to 2^24 and the exponent rises.
    let bits = binary32_bits(
        Numerator(NonZeroU32::new(0x03FF_FFFF).or_fail()?),
        NonZeroU32::new(0x0200_0000).or_fail()?,
    )
    .or_fail()?;
    ensure_eq!(bits.0, 0x4000_0000);
    Ok(())
}

#[test]
fn binary32_odd_tie_rounds_up() {
    assert_eq!(check_binary32_odd_tie_rounds_up(), Ok(()));
}

fn check_binary32_odd_tie_rounds_up() -> Result<(), Fail> {
    // The value is 1 + 3 * 2^-24, which lies in [1, 2): biased exponent 127, so the
    // significand is value * 2^23 = 2^23 + 3/2. Its integer part 2^23 + 1 is odd, and the
    // remainder is exactly half a unit (1 over a divisor of 2), a tie. Ties go to the
    // even neighbour, which is 2^23 + 2, so the fraction field is 2 and the word is
    // 0x3F80_0000 + 2 = 0x3F80_0002.
    let bits = binary32_bits(
        Numerator(NonZeroU32::new(0x0100_0003).or_fail()?),
        NonZeroU32::new(0x0100_0000).or_fail()?,
    )
    .or_fail()?;
    ensure_eq!(bits.0, 0x3F80_0002);
    Ok(())
}

#[test]
fn binary32_of_u32_max_over_one_carries_into_the_next_binade() {
    assert_eq!(
        check_binary32_of_u32_max_over_one_carries_into_the_next_binade(),
        Ok(())
    );
}

fn check_binary32_of_u32_max_over_one_carries_into_the_next_binade() -> Result<(), Fail> {
    // 2^32 - 1 lies in [2^31, 2^32): biased exponent 158, ulp 2^8. The quotient
    // (2^32 - 1) / 2^8 = 2^24 - 1/256, which rounds up to 2^24. That carries the
    // significand out of its 24 bits, so the value becomes 2^32: biased exponent
    // 127 + 32 = 159 = 0x9F, fraction 0. The word is 0 | 1001_1111 | 0...0 = 0x4F80_0000.
    let bits = binary32_bits(
        Numerator(NonZeroU32::new(u32::MAX).or_fail()?),
        NonZeroU32::new(1).or_fail()?,
    )
    .or_fail()?;
    ensure_eq!(bits.0, 0x4F80_0000);
    Ok(())
}

#[test]
fn binary32_of_one_over_u32_max_is_just_above_two_to_the_minus_32() {
    assert_eq!(
        check_binary32_of_one_over_u32_max_is_just_above_two_to_the_minus_32(),
        Ok(())
    );
}

fn check_binary32_of_one_over_u32_max_is_just_above_two_to_the_minus_32() -> Result<(), Fail> {
    // 1 / (2^32 - 1) = 2^-32 * 1 / (1 - 2^-32) = 2^-32 * (1 + 2^-32 + 2^-64 + ...).
    // It lies in [2^-32, 2^-31): biased exponent 127 - 32 = 95 = 0x5F. The excess over
    // 2^-32 is a relative 2^-32, far below the ulp of 2^-23, so the significand rounds
    // down to exactly 1 and the fraction field is 0. The word is 0 | 0101_1111 | 0...0,
    // which is 0x5F << 23 = 0x2F80_0000.
    let bits = binary32_bits(
        Numerator(NonZeroU32::new(1).or_fail()?),
        NonZeroU32::new(u32::MAX).or_fail()?,
    )
    .or_fail()?;
    ensure_eq!(bits.0, 0x2F80_0000);
    Ok(())
}
