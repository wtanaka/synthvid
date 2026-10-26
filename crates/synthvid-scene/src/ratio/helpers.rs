//! Helper functions for constructing and analyzing [`Ratio`] values.

use core::num::{NonZeroI64, NonZeroU128, NonZeroU16, NonZeroU32, NonZeroU8};

use super::{Numerator, Overflow, Ratio};
use crate::frame::{PixelColumn, PixelRow};
use crate::gauss::{NormalDraw, StreamIndex, UniformByte};
use crate::lattice::{PixelGrid, SampleIndex, SubSensel};
use crate::rng::{low_64, RandomWord, Rng};
use crate::stage::{Coord, PerFrame, PerFrameSquared};
use crate::units::{FrameIndex, Seed};

/// The low 64 bits of a signed 128-bit value, read back as an `i64`.
///
/// Exact whenever the value lies in the `i64` range, which each caller guarantees from the
/// bounds of its input type.
macro_rules! low_i64 {
    ($wide:expr_2021) => {{
        let [b0, b1, b2, b3, b4, b5, b6, b7, ..] = ($wide).to_le_bytes();
        i64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7])
    }};
}

/// Constructs a `NonZeroI64` with value 2.
///
/// Counts the set bits of `0b11`: a bitwise or into a `NonZeroU8` stays
/// non-zero, and so does its count of set bits, so there is no `Option` to
/// discharge. The count converts via `From`.
fn nonzero_two() -> NonZeroI64 {
    NonZeroI64::from((NonZeroU8::MIN | 0b10).count_ones())
}

/// Constructs a `NonZeroI64` with value 4.
///
/// Counts the set bits of `0b1111`, as [`nonzero_two`] does for `0b11`.
fn nonzero_four() -> NonZeroI64 {
    NonZeroI64::from((NonZeroU8::MIN | 0b1110).count_ones())
}

/// Returns the exact ratio `1 / 2`.
///
/// The construction is total: 1/2 is already in lowest terms (gcd(1, 2) = 1),
/// and both values fit in `i64`. Constructs the ratio directly without
/// going through the reduction path.
#[must_use]
pub fn half_ratio() -> Ratio {
    Ratio {
        numer: Numerator(1),
        denom: nonzero_two(),
    }
}

/// Returns the exact ratio `1 / 4`, the squared pixel-center threshold.
///
/// The construction is total: 1/4 is already in lowest terms (gcd(1, 4) = 1),
/// and both values fit in `i64`. Constructs the ratio directly without
/// going through the reduction path.
#[must_use]
pub fn quarter_ratio() -> Ratio {
    Ratio {
        numer: Numerator(1),
        denom: nonzero_four(),
    }
}

/// The index of a pixel along one axis: a column or a row.
///
/// Every `u16` names a pixel; the axis it lies on is the caller's to know.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct AxisPixel(pub u16);

/// Returns the exact ratio `(2 * index + 1) / 2`, the center of pixel `index`.
///
/// Doubling a `u16` and setting the low bit cannot leave the range of a `u32`
/// -- the largest result is `131_071` -- and widening that to `i64` is
/// infallible, so this needs no checked arithmetic and cannot fail. The
/// numerator is odd, so the ratio is already in lowest terms.
#[must_use]
pub fn pixel_center_ratio(pixel: AxisPixel) -> Ratio {
    let AxisPixel(index) = pixel;
    Ratio {
        numer: Numerator(i64::from((u32::from(index) << 1) | 1)),
        denom: nonzero_two(),
    }
}

/// Returns the greatest integer less than or equal to `value`, as a ratio
/// over one.
///
/// Uses Euclidean division; the denominator is strictly positive so the
/// result always fits in `i64` and this only returns `Err(Overflow)` in theory.
///
/// # Errors
///
/// Returns [`Overflow`] if division overflows.
pub fn floor_ratio(value: Ratio) -> Result<Ratio, Overflow> {
    value
        .numer()
        .0
        .checked_div_euclid(value.denom().get())
        .map(Numerator)
        .map(Ratio::from_integer)
        .ok_or(Overflow)
}

/// Returns the smallest integer greater than or equal to `value`, as a ratio
/// over one.
///
/// Computed as the Euclidean floor plus one when there is a remainder, so
/// no negation of `i64::MIN` is ever required.
///
/// # Errors
///
/// Returns [`Overflow`] if division or increment overflows.
pub fn ceil_ratio(value: Ratio) -> Result<Ratio, Overflow> {
    let quot = value
        .numer()
        .0
        .checked_div_euclid(value.denom().get())
        .ok_or(Overflow)?;
    let remnant = value
        .numer()
        .0
        .checked_rem_euclid(value.denom().get())
        .ok_or(Overflow)?;
    if remnant == 0 {
        Ok(Ratio::from_integer(Numerator(quot)))
    } else {
        quot.checked_add(1)
            .map(Numerator)
            .map(Ratio::from_integer)
            .ok_or(Overflow)
    }
}

/// A right shift by the base-2 logarithm of a grid's samples per axis.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct GridShift(pub u32);

/// The sum of twelve uniform bytes centered on zero: `12 * 127.5`, which is `1530`.
///
/// A [`UniformByte`] is uniform on `0..=255`, so its mean is 127.5 and the mean of a sum of
/// twelve is 1530. Subtracting this from the sum gives a mean of zero and a range of
/// `[-1530, 1530]`.
const IRWIN_HALL_CENTER: i32 = 1530;

/// The multiplier of the frame index in the stream seed state.
const FRAME_STRIDE: u128 = 0x94D0_49BB_1331_11EB;

/// The multiplier of the stream index in the stream seed state.
const INDEX_STRIDE: u128 = 0xD6E8_FEB8_6659_FD93;

/// Computes the seed of one stream at one frame from a base seed.
///
/// The state is `base + frame * FRAME_STRIDE + index * INDEX_STRIDE`, taken modulo 2^64, where
/// both strides are odd constants defined in this module. Multiplying by an odd constant is a
/// bijection on the 64-bit residues. The state then passes through one [`Rng`] output.
///
/// Swapping a frame value and an index value changes the state by
/// `(frame - index) * (FRAME_STRIDE - INDEX_STRIDE)` modulo 2^64. The difference of the strides
/// is `8` times an odd number, so this change is zero exactly when `frame - index` is a multiple
/// of 2^61. A stream index is a full `u64`, so a pair such as frame 0 and index 2^61 has that
/// property; its swap, frame 2^61, is not a valid frame index, so the swap does not produce a
/// collision between two valid pairs. Distinct pairs can share a state in general, since the
/// frame and index together have more values than the 64-bit state.
#[must_use]
pub(crate) fn derive_stream_seed(base: Seed, frame: FrameIndex, index: StreamIndex) -> Seed {
    let StreamIndex(stream) = index;
    // The state is defined modulo 2^64: each product and sum is formed in 128 bits, where it
    // cannot overflow, and truncated to its low 64 bits.
    let frame_term = low_64!(u128::from(frame.get()) * FRAME_STRIDE);
    let index_term = low_64!(u128::from(stream) * INDEX_STRIDE);
    let state = low_64!([
        u128::from(base.get()),
        u128::from(frame_term),
        u128::from(index_term),
    ]
    .into_iter()
    .sum::<u128>());
    let RandomWord(word) = Rng::from_seed(Seed::new(state)).next_u64();
    Seed::new(word)
}

/// Sums twelve uniform draws and centers the sum on zero by the Irwin-Hall method.
///
/// The sum of twelve bytes lies in `[0, 3060]`, so it fits in `i32`, and subtracting the center
/// of 1530 gives a sample in `[-1530, 1530]`. Each byte is uniform on `[0, 256)` with variance
/// `(256^2 - 1) / 12`, so the sum has variance 65535.
#[must_use]
pub(crate) fn irwin_hall_sample(draws: [UniformByte; 12]) -> NormalDraw {
    let total: i32 = draws.iter().map(|UniformByte(byte)| i32::from(*byte)).sum();
    // The sum lies in [0, 3060], so subtracting the center stays within `i32`.
    NormalDraw([total, -IRWIN_HALL_CENTER].into_iter().sum())
}

/// Returns the across coordinate of the left edge of a column, in lattice units.
///
/// A sensel spans 256 lattice units and a column is a `u16`, so the product lies in
/// `[0, 2^24)`, which is inside the `i64` range.
#[must_use]
pub(crate) fn column_start(column: PixelColumn) -> SubSensel {
    let PixelColumn(cell) = column;
    SubSensel(low_i64!(u128::from(cell) * 256))
}

/// Returns the down coordinate of the top edge of a row, in lattice units.
///
/// A sensel spans 256 lattice units and a row is a `u16`, so the product lies in
/// `[0, 2^24)`, which is inside the `i64` range.
#[must_use]
pub(crate) fn row_start(row: PixelRow) -> SubSensel {
    let PixelRow(cell) = row;
    SubSensel(low_i64!(u128::from(cell) * 256))
}

/// Returns the coordinate of one sample along an axis, given the edge of its sensel.
///
/// Computes `start + 128 * (2 * index + 1) / G`, where `G` is the grid's samples per axis. Each
/// arm divides by `G` as a shift by `log2(G)`: `G` is a power of two that divides 128, and the
/// product `128 * (2 * index + 1)` has at least that many trailing zero bits, so every shift is
/// exact. The edge lies below `2^24` and the offset below `2^11`, so the sum lies in the `i64`
/// range.
#[must_use]
pub(crate) fn sample_axis(start: SubSensel, index: SampleIndex, grid: PixelGrid) -> SubSensel {
    let SubSensel(edge) = start;
    let SampleIndex(slot) = index;
    // `2 * slot + 1` is the shifted slot with its low bit set, and the shift keeps it exact.
    let odd = (i128::from(slot) << 1_u32) | 1;
    let span = odd << 7_u32;
    let offset = span >> grid.log2_per_axis().0;
    SubSensel(low_i64!([i128::from(edge), offset]
        .into_iter()
        .sum::<i128>()))
}

/// One axis of a track before the world bound is checked, held exactly in 128 bits.
///
/// Every `i128` is a legal value, so the field is public to the crate.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactAxis(pub(crate) i128);

// The bounds on `track_axis` assume the widths of its inputs: a velocity and a half acceleration
// are `i32`, and a frame index is `u32`. Each assignment below fails to compile if one of those
// field types changes, so a wider type cannot silently break the bounds.
const _: fn(PerFrame) -> i32 = |velocity| velocity.0;
const _: fn(PerFrameSquared) -> i32 = |half_acceleration| half_acceleration.0;
const _: fn(FrameIndex) -> u32 = |frame| frame.0;

/// Evaluates one axis of a track as `origin + velocity * frame + half_acceleration * frame^2`.
///
/// The origin is a world coordinate, so its magnitude is at most `2^20`. The frame index is below
/// `2^32`, so the velocity term has magnitude below `2^31 * 2^32 = 2^63` and the acceleration term
/// has magnitude below `2^31 * 2^64 = 2^95`. The sum therefore has magnitude below `2^96`, and
/// every intermediate lies well inside the `i128` range. The widths behind these bounds are
/// checked at compile time by the assignments above.
#[must_use]
pub(crate) fn track_axis(
    origin: Coord,
    velocity: PerFrame,
    half_acceleration: PerFrameSquared,
    frame: FrameIndex,
) -> ExactAxis {
    let elapsed = i128::from(frame.get());
    let start = i128::from(origin.get().0);
    // Each product is formed as an iterator product of exact `i128` factors, as the sums in
    // `derive_stream_seed` are, and the bounds above keep every one of them in range.
    let linear = [i128::from(velocity.0), elapsed]
        .into_iter()
        .product::<i128>();
    let quadratic = [i128::from(half_acceleration.0), elapsed, elapsed]
        .into_iter()
        .product::<i128>();
    ExactAxis([start, linear, quadratic].into_iter().sum::<i128>())
}

/// Where twice a distance lies relative to the product of a factor and a length.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DoubledOrder {
    /// Twice the distance is below the product.
    Below,
    /// Twice the distance is at least the product.
    AtOrAbove,
}

/// Compares twice `distance` with the product of `factor` and `length`, exactly.
///
/// Each side is formed in `u128`: twice a `u32` is below `2^33`, and a `u8` times a `u16` is
/// below `2^24`. Both sides therefore sit far below the `u128` range, so the multiplications cannot
/// overflow and the comparison is exact and cannot fail.
#[must_use]
pub fn compare_doubled_with_product(
    distance: NonZeroU32,
    factor: NonZeroU8,
    length: NonZeroU16,
) -> DoubledOrder {
    let doubled = [NonZeroU128::from(distance).get(), 2]
        .into_iter()
        .product::<u128>();
    let product = [
        NonZeroU128::from(factor).get(),
        NonZeroU128::from(length).get(),
    ]
    .into_iter()
    .product::<u128>();
    if doubled < product {
        DoubledOrder::Below
    } else {
        DoubledOrder::AtOrAbove
    }
}
