//! The filtered noise field of one color channel of one frame.
//!
//! The field starts as a grid of white normal draws with the window less one extra column on the
//! right and extra row at the bottom. Each padded row is drawn from its own stream, in column
//! order, so no row depends on the rows before it. The field is then a box sum over a square of
//! the window's width, divided by that width. A sum of `w * w` independent draws has standard
//! deviation `w`, so the quotient has standard deviation one at every window width.
//!
//! The sums are separable: each padded row is first summed over the window's columns, then the
//! resulting rows are summed over the window's rows, one band at a time.

use core::num::NonZeroI64;

use synthvid_scene::{normal_draw, stream_seed, Overflow, Rng, StreamIndex};

use super::{FieldShape, GrainError, GrainWindow};
use crate::sensor::{channel_block_base, BlockLength, SignedQ8};

/// A sum of raw normal draws, in the same fixed-point scale as one draw.
///
/// A raw value of 256 is one standard deviation of a single draw, so a sum of `n` draws has a
/// standard deviation of `256 * sqrt(n)`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct DrawSum(pub i64);

impl DrawSum {
    /// Adds another sum to this one, failing rather than wrapping.
    fn plus(self, addend: Self) -> Result<Self, GrainError> {
        let Self(total_raw) = self;
        let Self(addend_raw) = addend;
        total_raw
            .checked_add(addend_raw)
            .map(Self)
            .ok_or(GrainError::Overflow)
    }
}

/// Adds a row of window sums to the running column sums, element by element.
fn add_row(column_sums: &mut [DrawSum], row: &[DrawSum]) -> Result<(), GrainError> {
    for (total, &addend) in column_sums.iter_mut().zip(row) {
        *total = total.plus(addend)?;
    }
    Ok(())
}

/// The window average of a sum of draws, floored: `sum / window`, in Q8.8.
fn filter(sum: DrawSum, window: GrainWindow) -> Result<SignedQ8, GrainError> {
    let DrawSum(total) = sum;
    let divisor = NonZeroI64::from(window.get()).get();
    total
        .checked_div_euclid(divisor)
        .map(SignedQ8)
        .ok_or(GrainError::Overflow)
}

/// Computes the filtered noise of one channel of one frame, one row of values per sensel row.
///
/// Each value is the window's square of draws, summed and divided by the window width, in Q8.8.
///
/// # Errors
///
/// Returns `Err(GrainError::Overflow)` if a count or a sum does not fit its integer type.
pub(super) fn filtered_noise(shape: FieldShape) -> Result<Vec<Vec<SignedQ8>>, GrainError> {
    let width = shape.dimensions.width.get().get();
    let height = shape.dimensions.height.get().get();
    let window = shape.window.get().get();
    let window_len = usize::from(window);
    let padded_columns = u64::from(width)
        .checked_add(u64::from(window))
        .and_then(|sum| sum.checked_sub(1))
        .ok_or(GrainError::Overflow)?;
    let padded_rows = u64::from(height)
        .checked_add(u64::from(window))
        .and_then(|sum| sum.checked_sub(1))
        .ok_or(GrainError::Overflow)?;
    // Stream `channel * padded_rows + row` is the stream of one padded row of one channel.
    let StreamIndex(channel_base) = channel_block_base(shape.plane, BlockLength(padded_rows))
        .map_err(|Overflow| GrainError::Overflow)?;

    let mut horizontal_sums = Vec::new();
    for padded_row in 0..padded_rows {
        let stream = channel_base
            .checked_add(padded_row)
            .map(StreamIndex)
            .ok_or(GrainError::Overflow)?;
        let mut rng = Rng::from_seed(stream_seed(shape.seed, shape.index, stream));
        let mut draws = Vec::new();
        for _ in 0..padded_columns {
            draws.push(normal_draw(&mut rng)?);
        }
        let sums = draws
            .windows(window_len)
            .map(|run| {
                run.iter().try_fold(DrawSum(0), |total, &draw| {
                    total.plus(DrawSum(i64::from(draw.0)))
                })
            })
            .collect::<Result<Vec<DrawSum>, GrainError>>()?;
        horizontal_sums.push(sums);
    }

    let mut column_sums = vec![DrawSum(0); usize::from(width)];
    let mut field = Vec::new();
    for band in horizontal_sums.windows(window_len) {
        column_sums.fill(DrawSum(0));
        for row in band {
            add_row(&mut column_sums, row)?;
        }
        let filtered = column_sums
            .iter()
            .map(|&sum| filter(sum, shape.window))
            .collect::<Result<Vec<SignedQ8>, GrainError>>()?;
        field.push(filtered);
    }
    Ok(field)
}
