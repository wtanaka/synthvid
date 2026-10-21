//! Capacity limits shared by every "classic" container format this crate
//! writes.
//!
//! Both the ISO base media `mdat` box and the AVI `movi` LIST (and its
//! `idx1` chunk offsets) address their payload with a 4-byte size or offset
//! field. Neither format's 64-bit extension is implemented here (ISO's
//! `largesize`, AVI's `OpenDML` `avi2`), so raw (uncompressed) frame data,
//! which grows linearly with pixel count and frame count, can exceed that
//! 32-bit limit well before any other resource does. This is the single
//! place that limit is expressed, so every caller checks the same number.

use core::num::NonZeroU64;
use synthvid_scene::{Dimensions, FrameCount};

/// The largest payload a classic 32-bit-size container can address:
/// `u32::MAX`, `2^32 - 1`.
pub const MAX_CLASSIC_CONTAINER_PAYLOAD: u64 = 0xFFFF_FFFF;

/// Whether some raw frames fit a classic 32-bit-size container.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ContainerFit {
    /// The frames fit within the container's payload limit.
    Fits,
    /// The frames exceed the container's payload limit.
    TooLarge,
}

/// Whether `frame_count` raw (uncompressed, 3 bytes per pixel) frames of
/// `dimensions` fit within a classic 32-bit-size container's payload limit.
///
/// Reserves 16 bytes per frame for the surrounding per-frame overhead every
/// classic container writer here adds (an AVI chunk header, or an ISO
/// `stsz`/`stco` table entry), so this stays conservative rather than exact.
///
/// # Example
///
/// ```rust
/// use synthvid_encode::{raw_frames_fit_classic_container, ContainerFit};
/// use synthvid_scene::{Dimensions, FrameCount, Height, Width};
///
/// let (Ok(width), Ok(height), Ok(frames)) = (Width::new(16), Height::new(16), FrameCount::new(1))
/// else {
///     return;
/// };
/// let fit = raw_frames_fit_classic_container(Dimensions::new(width, height), frames);
/// assert_eq!(fit, ContainerFit::Fits);
/// ```
#[must_use]
pub fn raw_frames_fit_classic_container(
    dimensions: Dimensions,
    frame_count: FrameCount,
) -> ContainerFit {
    let total = NonZeroU64::from(dimensions.width.get())
        .get()
        .checked_mul(NonZeroU64::from(dimensions.height.get()).get())
        .and_then(|pixel_bytes| pixel_bytes.checked_mul(3))
        .and_then(|frame_bytes| frame_bytes.checked_add(16))
        .and_then(|bytes_per_frame| {
            bytes_per_frame.checked_mul(NonZeroU64::from(frame_count.0).get())
        });
    match total {
        Some(bytes) if bytes <= MAX_CLASSIC_CONTAINER_PAYLOAD => ContainerFit::Fits,
        // More than `u64::MAX` bytes (`None`) is far beyond the 4 GiB payload
        // limit, so an overflow is a correct "too large", not a failure to
        // compute one.
        Some(_) | None => ContainerFit::TooLarge,
    }
}

#[cfg(test)]
mod tests;
