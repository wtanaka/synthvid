//! The capture rates each capture singleton declares, and the box each one is written in.

use super::FrameRateValue;
use synthvid_scene::CaptureCarrier;

/// The number of frames in a capture rate, before reduction.
pub(super) struct CaptureNumerator(pub(super) u32);

/// The number of seconds in a capture rate, before reduction.
///
/// A zero denominator is possible in the const table, and is refused with
/// `CatalogError::InvalidFrameRate` when the rate is built. It is not a `NonZeroU32` here
/// because a `NonZeroU32` cannot be constructed in a const table without unwrap, expect or
/// panic, which the library forbids.
pub(super) struct CaptureDenominator(pub(super) u32);

/// A capture rate as the table writes it: `numer` frames per `denom` seconds, not
/// yet reduced.
pub(super) struct CaptureFraction {
    /// The number of frames.
    pub(super) numer: CaptureNumerator,
    /// The number of seconds.
    pub(super) denom: CaptureDenominator,
}

/// The playback rate, capture rate and carrier of one capture singleton.
pub(super) struct CapturePair {
    /// The rate the media is played back at.
    pub(super) playback: FrameRateValue,
    /// The rate the media was captured at.
    pub(super) capture: CaptureFraction,
    /// The box the capture rate is written in.
    pub(super) carrier: CaptureCarrier,
}

/// The playback rate, capture rate and carrier of each capture singleton.
///
/// A capture rate is any positive rational, so each pair is a fraction and is
/// reduced by the frame rate it becomes. Each pair targets one way a reader can
/// go wrong: a slow-motion ratio of eight; an awkward integer ratio; a fractional
/// rate, which a reader that divides only the numerators gets wrong; a PAL rate
/// written into an NTSC entry; a capture slower than playback; and a large
/// denominator.
///
/// The first six write the capture rate in the project box (`cfps`). The last three
/// write it in the `QuickTime`-style `meta` box, as a float32: 30 played at 240 captured,
/// an exact float32; NTSC played at 24000/1001 captured, which float32 cannot hold
/// exactly; and 30 played at 12 captured, a slower capture.
pub(super) const CAPTURE_PAIRS: [CapturePair; 9] = [
    CapturePair {
        playback: FrameRateValue::Thirty,
        capture: CaptureFraction {
            numer: CaptureNumerator(240),
            denom: CaptureDenominator(1),
        },
        carrier: CaptureCarrier::ProjectBox,
    },
    CapturePair {
        playback: FrameRateValue::Ntsc,
        capture: CaptureFraction {
            numer: CaptureNumerator(1000),
            denom: CaptureDenominator(1),
        },
        carrier: CaptureCarrier::ProjectBox,
    },
    CapturePair {
        playback: FrameRateValue::Ntsc,
        capture: CaptureFraction {
            numer: CaptureNumerator(24000),
            denom: CaptureDenominator(1001),
        },
        carrier: CaptureCarrier::ProjectBox,
    },
    CapturePair {
        playback: FrameRateValue::Ntsc,
        capture: CaptureFraction {
            numer: CaptureNumerator(25),
            denom: CaptureDenominator(1),
        },
        carrier: CaptureCarrier::ProjectBox,
    },
    CapturePair {
        playback: FrameRateValue::Thirty,
        capture: CaptureFraction {
            numer: CaptureNumerator(12),
            denom: CaptureDenominator(1),
        },
        carrier: CaptureCarrier::ProjectBox,
    },
    CapturePair {
        playback: FrameRateValue::Thirty,
        capture: CaptureFraction {
            numer: CaptureNumerator(120_000),
            denom: CaptureDenominator(1001),
        },
        carrier: CaptureCarrier::ProjectBox,
    },
    CapturePair {
        playback: FrameRateValue::Thirty,
        capture: CaptureFraction {
            numer: CaptureNumerator(240),
            denom: CaptureDenominator(1),
        },
        carrier: CaptureCarrier::QuickTimeMeta,
    },
    CapturePair {
        playback: FrameRateValue::Ntsc,
        capture: CaptureFraction {
            numer: CaptureNumerator(24000),
            denom: CaptureDenominator(1001),
        },
        carrier: CaptureCarrier::QuickTimeMeta,
    },
    CapturePair {
        playback: FrameRateValue::Thirty,
        capture: CaptureFraction {
            numer: CaptureNumerator(12),
            denom: CaptureDenominator(1),
        },
        carrier: CaptureCarrier::QuickTimeMeta,
    },
];
