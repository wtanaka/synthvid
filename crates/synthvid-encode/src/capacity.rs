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

use synthvid_scene::{Height, Width};

/// The largest payload a classic 32-bit-size container can address.
#[must_use]
pub fn max_classic_container_payload() -> u64 {
    u64::from(u32::MAX)
}

/// Returns true if `frame_count` raw (uncompressed, 3 bytes per pixel)
/// frames at `width` by `height` fit within a classic 32-bit-size
/// container's payload limit.
///
/// Reserves 16 bytes per frame for the surrounding per-frame overhead every
/// classic container writer here adds (an AVI chunk header, or an ISO
/// `stsz`/`stco` table entry), so this stays conservative rather than exact.
#[must_use]
pub fn raw_frames_fit_classic_container(width: Width, height: Height, frame_count: u32) -> bool {
    let pixel_bytes = u64::from(width.get().get()).saturating_mul(u64::from(height.get().get()));
    let bytes_per_frame = pixel_bytes.saturating_mul(3).saturating_add(16);
    let total = bytes_per_frame.saturating_mul(u64::from(frame_count));
    total <= max_classic_container_payload()
}
