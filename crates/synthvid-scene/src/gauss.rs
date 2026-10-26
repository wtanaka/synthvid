//! Deterministic integer building blocks for noise rendering.
//!
//! Every function here is a pure function of its arguments, computed with
//! integers only, so the same inputs give the same output on every platform.
//! [`normal_draw`] approximates a standard normal sample by the Irwin-Hall
//! method, [`stream_seed`] derives an independent seed for each frame and
//! stream index, and [`isqrt`] is an exact integer square root.
//!
//! The modular and summing arithmetic behind [`stream_seed`] and
//! [`normal_from_uniform_draws`] lives in the `ratio::helpers` module, which
//! this module calls.

use core::num::NonZeroU64;

use crate::ratio::Overflow;
use crate::ratio::{derive_stream_seed, irwin_hall_sample};
use crate::rng::{BoundedDraw, DrawBound, Rng};
use crate::units::{FrameIndex, Seed};

/// The exclusive upper limit of a [`UniformByte`] draw.
const UNIFORM_LIMIT: u64 = 256;

/// An approximately standard-normal sample in fixed point.
///
/// The value 256 is one standard deviation and 0 is the mean. Every `i32` is a
/// legal sample, so the field is public and needs no checked constructor.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct NormalDraw(pub i32);

/// One uniform draw on `[0, 256)`.
///
/// Every `u8` is a legal uniform draw, so the field is public.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct UniformByte(pub u8);

/// The index of one independent noise stream, such as a sensel.
///
/// Every `u64` names a stream, so the field is public.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct StreamIndex(pub u64);

/// The input to [`isqrt`]. Every `u64` is a legal input.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct IntSqrtInput(pub u64);

/// The floor of the square root returned by [`isqrt`].
///
/// Every `u64` is a possible result, so the field is public.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct IntSqrtOutput(pub u64);

/// Derives a reproducible seed from a base seed, a frame index and a stream
/// index.
///
/// The state is `base + frame * FRAME_STRIDE + index * INDEX_STRIDE`, reduced
/// modulo 2^64, where the strides are odd constants defined in the
/// `ratio::helpers` module. It is a closed-form function of its three inputs,
/// so no earlier frame or stream is visited. The state then passes through one
/// [`Rng`] output, which mixes its bits so that nearby states give unrelated
/// seeds.
///
/// Swapping a frame value and an index value changes the state by
/// `(frame - index) * (FRAME_STRIDE - INDEX_STRIDE)` modulo 2^64. The strides
/// differ by 8 times an odd number, so this change is zero exactly when
/// `frame - index` is a multiple of 2^61. A stream index is a full `u64`, so
/// such a pair exists, for example frame 0 and index 2^61. Its swap, frame
/// 2^61, is not a valid [`FrameIndex`], so no two valid pairs collide by
/// swapping.
///
/// The output mixing is a bijection, so distinct states give distinct seeds.
/// Because 2^96 pairs cannot all map to 2^64 seeds, distinctness holds for
/// distinct states, not for every pair.
#[must_use]
pub fn stream_seed(base: Seed, frame: FrameIndex, index: StreamIndex) -> Seed {
    derive_stream_seed(base, frame, index)
}

/// Approximates a standard-normal sample from twelve uniform draws by the
/// Irwin-Hall method.
///
/// Each draw is uniform on the integers `0..=255`, with mean 127.5 and
/// variance `(256^2 - 1) / 12`. The sum of twelve such draws has mean 1530 and
/// variance `256^2 - 1 = 65535`, so subtracting 1530 gives a sample with mean
/// 0 and variance 65535, a standard deviation just under 256. The result is in
/// `[-1530, 1530]`.
#[must_use]
pub fn normal_from_uniform_draws(draws: [UniformByte; 12]) -> NormalDraw {
    irwin_hall_sample(draws)
}

/// Draws one approximately standard-normal sample from `rng`.
///
/// Consumes exactly twelve draws from `rng`, each uniform on `[0, 256)`, and
/// passes them to [`normal_from_uniform_draws`].
///
/// # Errors
///
/// Returns `Err(Overflow)` if the draw bound cannot be built as a non-zero
/// value or if a draw does not fit in a byte. Neither can happen for the
/// bound of 256 used here.
pub fn normal_draw(rng: &mut Rng) -> Result<NormalDraw, Overflow> {
    let bound = DrawBound(NonZeroU64::new(UNIFORM_LIMIT).ok_or(Overflow)?);
    let mut draws = [UniformByte(0); 12];
    for slot in &mut draws {
        let BoundedDraw(raw) = rng.next_bounded(bound);
        *slot = UniformByte(u8::try_from(raw).map_err(Overflow::from)?);
    }
    Ok(normal_from_uniform_draws(draws))
}

/// Returns `floor(sqrt(value))` exactly.
///
/// The root is the standard library's exact integer square root, `u64::isqrt`.
#[must_use]
pub const fn isqrt(value: IntSqrtInput) -> IntSqrtOutput {
    let IntSqrtInput(input) = value;
    IntSqrtOutput(input.isqrt())
}

#[cfg(test)]
mod tests;
