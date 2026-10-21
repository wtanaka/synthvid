use super::*;
use synthvid_scene::{Height, Width};
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

#[test]
fn limit_is_the_largest_32_bit_size() {
    assert_eq!(MAX_CLASSIC_CONTAINER_PAYLOAD, u64::from(u32::MAX));
}

/// A 1x1 frame costs 3 sample bytes plus 16 bytes of overhead: 19. The
/// largest count that fits is `floor((2^32 - 1) / 19) = 226_050_910`
/// (`226_050_910 * 19 = 4_294_967_290`), and one more frame is
/// `4_294_967_309` bytes, 14 past the limit.
#[test]
fn boundary_is_exact_for_one_pixel_frames() {
    assert_eq!(check_boundary_is_exact_for_one_pixel_frames(), Ok(()));
}

fn check_boundary_is_exact_for_one_pixel_frames() -> Result<(), Fail> {
    let pixel = Dimensions::new(Width::new(1).or_fail()?, Height::new(1).or_fail()?);
    ensure_eq!(
        raw_frames_fit_classic_container(pixel, FrameCount::new(226_050_910).or_fail()?),
        ContainerFit::Fits
    );
    ensure_eq!(
        raw_frames_fit_classic_container(pixel, FrameCount::new(226_050_911).or_fail()?),
        ContainerFit::TooLarge
    );
    Ok(())
}

/// `65_535 * 65_535 * 3 + 16 = 12_884_508_691` bytes: one maximal frame
/// alone is about three times the limit.
#[test]
fn one_maximal_frame_is_too_large() {
    assert_eq!(check_one_maximal_frame_is_too_large(), Ok(()));
}

fn check_one_maximal_frame_is_too_large() -> Result<(), Fail> {
    let largest = Dimensions::new(
        Width::new(u16::MAX).or_fail()?,
        Height::new(u16::MAX).or_fail()?,
    );
    ensure_eq!(
        raw_frames_fit_classic_container(largest, FrameCount::new(1).or_fail()?),
        ContainerFit::TooLarge
    );
    Ok(())
}

/// `12_884_508_691 * (2^32 - 1)` bytes is past `u64::MAX`, so the byte
/// count overflows; an overflow is reported as too large, not as a fit.
#[test]
fn overflowing_byte_count_is_too_large() {
    assert_eq!(check_overflowing_byte_count_is_too_large(), Ok(()));
}

fn check_overflowing_byte_count_is_too_large() -> Result<(), Fail> {
    let largest = Dimensions::new(
        Width::new(u16::MAX).or_fail()?,
        Height::new(u16::MAX).or_fail()?,
    );
    ensure_eq!(
        raw_frames_fit_classic_container(largest, FrameCount::new(u32::MAX).or_fail()?),
        ContainerFit::TooLarge
    );
    Ok(())
}
