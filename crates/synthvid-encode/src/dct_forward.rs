//! The forward DCT itself: cosine table, fixed-point divisors, and the
//! row/column-pass accumulator types, split out of `dct.rs` to keep that
//! module under its line-count budget.

use core::num::NonZeroU64;

use super::{div_signed, saturate_to_i32, CenteredSample};

/// Precomputed cosine table for the 8-point DCT-II, scaled by 2^16 for
/// integer-only arithmetic.
///
/// The DCT-II basis function is `cos((2n+1)*k*π/(2N))` for `N=8`; this
/// table holds `cos((2x+1)*u*π/16)`, one row per frequency `u`.
///
/// `F(u,v) = (1/4) * C(u) * C(v) * sum_{x,y} f(x,y) * cos((2x+1)uπ/16) *
/// cos((2y+1)vπ/16)`
///
/// where C(0) = 1/√2 and C(k) = 1 for k > 0.
///
/// All constants are pre-computed as integers scaled by 2^16 (65536) to maintain precision
/// while using only integer arithmetic. The scaling preserves sufficient accuracy
/// for JPEG quantization and reconstruction without any transcendental functions.
///
/// Constants are indexed as `COSINES[u][x]` = cos((2*x+1)*u*π/16) * 2^16, stored as i32.
/// See: ITU-T T.81 (09/92) - Information technology – Digital compression and coding of
/// continuous-tone still images – Requirements and guidelines
const COSINES: [[i32; 8]; 8] = [
    // u=0: cos((2x+1)*0*π/16) = 1.0 for all x, scaled by 2^16
    [65536, 65536, 65536, 65536, 65536, 65536, 65536, 65536],
    // u=1: cos((2x+1)*1*π/16)
    [64277, 54491, 36410, 12785, -12785, -36410, -54491, -64277],
    // u=2: cos((2x+1)*2*π/16)
    [60547, 25080, -25080, -60547, -60547, -25080, 25080, 60547],
    // u=3: cos((2x+1)*3*π/16)
    [54491, -12785, -64277, -36410, 36410, 64277, 12785, -54491],
    // u=4: cos((2x+1)*4*π/16)
    [46341, -46341, -46341, 46341, 46341, -46341, -46341, 46341],
    // u=5: cos((2x+1)*5*π/16)
    [36410, -64277, 12785, 54491, -54491, -12785, 64277, -36410],
    // u=6: cos((2x+1)*6*π/16)
    [25080, -60547, 60547, -25080, -25080, 60547, -60547, 25080],
    // u=7: cos((2x+1)*7*π/16)
    [12785, -36410, 54491, -64277, 64277, -54491, 36410, -12785],
];

/// Scaled reciprocal of √2: 2^16 / √2 ≈ 46341.
/// Used for C(0) = 1/√2 normalization in the DCT formula.
const SQRT2_RECIPROCAL: u64 = 46341;

/// Scale factor for cosines: 2^16 = 65536.
const COSINE_SCALE: u64 = 65536;

/// Divisor for the (0,0) DC term: `8 * 2^32` (see `forward_dct`'s doc comment
/// for the derivation). Computed once at compile time -- no runtime
/// arithmetic. Built via `saturating_add` on `NonZeroU64::MIN` rather than
/// a fallible `NonZeroU64` constructor, so the type's nonzero guarantee
/// holds without any construction path that could ever panic, even at
/// compile time.
const DCT_DIVISOR_DC: NonZeroU64 =
    NonZeroU64::MIN.saturating_add(8 * COSINE_SCALE * COSINE_SCALE - 1);

/// Divisor for edge terms (`u==0 xor v==0`): `4 * sqrt(2) * 2^32`. `C(0) =
/// 1/sqrt(2)` appears in the *numerator* of `F(u,v) = (1/4) * C(u) * C(v) *
/// sum`, so dividing it out of the sum means multiplying the divisor by
/// `sqrt(2)`, not by `1/sqrt(2)` again -- `sqrt(2) = 2 * (1/sqrt(2))`, so
/// `sqrt(2) * 2^16 = 2 * SQRT2_RECIPROCAL`. An earlier version of this
/// divisor used `SQRT2_RECIPROCAL` directly here (i.e. `4 *
/// SQRT2_RECIPROCAL * COSINE_SCALE`), which is exactly half this value: it
/// silently doubled the magnitude of every edge coefficient (`u==0 xor
/// v==0`) this encoder ever produced. Solid-color and 4x4-checkerboard test
/// inputs never exposed this, since they only ever excite the DC term or
/// interior (`u>0 and v>0`) terms -- never an edge term on its own.
const DCT_DIVISOR_EDGE: NonZeroU64 =
    NonZeroU64::MIN.saturating_add(4 * (2 * SQRT2_RECIPROCAL) * COSINE_SCALE - 1);

/// Divisor for interior terms (`u>0 and v>0`): `4 * 2^32`.
const DCT_DIVISOR_INTERIOR: NonZeroU64 =
    NonZeroU64::MIN.saturating_add(4 * COSINE_SCALE * COSINE_SCALE - 1);

/// A `COSINES` factor for the horizontal (x) axis, scaled by `2^16`.
/// Distinct from [`CosV`] so [`RowSum::add_term`]'s cosine argument
/// cannot be confused with a vertical one -- unlike two bare `i32`s,
/// which the compiler accepts in either position, even though swapping the
/// horizontal and vertical factors would silently transpose the DCT.
#[derive(Debug, Clone, Copy)]
struct CosU(i32);

/// A `COSINES` factor for the vertical (y) axis; see [`CosU`].
#[derive(Debug, Clone, Copy)]
struct CosV(i32);

/// Running sum of `row_sum * cos_v` products across an 8x8 block's column
/// pass (8 terms per output cell), accumulated in `i64`. Together with
/// [`RowSum`]'s row pass, this computes the same 64-term
/// `sample * cos_u * cos_v` sum a flat loop would accumulate in one pass,
/// just re-associated into two passes; see [`DctSum::add_row_term`].
///
/// Bound: each `sample` is a `CenteredSample` (`|value| <= 128`), each
/// cosine factor is drawn from `COSINES` (`|value| <= 65536` by inspection
/// of the table), so each of the original 64 `sample * cos_u * cos_v`
/// products is at most `128 * 65536 * 65536 = 2^39` in magnitude, and
/// summing 64 such products is at most `64 * 2^39 = 2^45` in magnitude --
/// far inside `i64` (`2^63 - 1`).
#[derive(Debug, Clone, Copy)]
struct DctSum(i64);

impl DctSum {
    /// Starting value for accumulating a DCT sum (zero).
    const ZERO: Self = Self(0);

    /// Gets the underlying accumulated sum value.
    const fn get(self) -> i64 {
        self.0
    }

    /// Adds one `row_sum * cos_v_y` term -- the column-pass counterpart of
    /// [`RowSum::add_term`], combining an already-row-summed value (see
    /// [`RowSum`]) with the vertical cosine factor. `forward_dct` uses this
    /// to compute the exact same 64-term sum a flat double loop would, but
    /// re-associated into a row pass followed by a column pass -- exact
    /// integer re-association, not an approximation, so it changes nothing
    /// about the result, only how many multiplications reach it (from 64
    /// per output cell down to 8, since the row sums are shared across all
    /// 8 values of `v`).
    ///
    /// Bound: `row_sum` is at most `2^26` in magnitude (see [`RowSum`]'s own
    /// doc comment) and `cos_v_y` is at most `2^16`, so each product is at
    /// most `2^42`, and summing 8 such products is at most `2^45` -- the
    /// same bound the flat 64-term sum has, since both compute the
    /// identical total.
    fn add_row_term(self, row_sum: RowSum, cos_v_y: CosV) -> Self {
        let a = row_sum.get();
        let b = i64::from(cos_v_y.0);
        let product = match a.checked_mul(b) {
            Some(p) => p,
            None if (a < 0) == (b < 0) => i64::MAX,
            None => i64::MIN,
        };
        let sum = match self.0.checked_add(product) {
            Some(s) => s,
            None if product < 0 => i64::MIN,
            None => i64::MAX,
        };
        Self(sum)
    }
}

/// Sum of `sample * cos_u_x` products across one row of 8 samples, for one
/// fixed `u`. This is the row-pass intermediate `forward_dct` computes once
/// per `(u, y)` pair and reuses across all 8 values of `v`, rather than
/// recomputing the same 8-term sum from scratch for each `v` as a flat
/// double loop over all 64 `(x, y)` pairs would.
///
/// Bound: each `sample * cos_u_x` product is at most `128 * 65536 = 2^23`
/// in magnitude, and summing 8 such products is at most `8 * 2^23 = 2^26`
/// -- far inside `i64`.
#[derive(Debug, Clone, Copy)]
struct RowSum(i64);

impl RowSum {
    /// Starting value for accumulating a row sum (zero).
    const ZERO: Self = Self(0);

    /// Adds one `sample * cos_u_x` term. Total via `checked_*` with a
    /// saturating fallback: the in-range bound above means saturation never
    /// actually triggers, but the type no longer depends on that being true
    /// to stay total.
    fn add_term(self, sample: CenteredSample, cos_u_x: CosU) -> Self {
        let a = i64::from(sample.get());
        let b = i64::from(cos_u_x.0);
        let product = match a.checked_mul(b) {
            Some(p) => p,
            None if (a < 0) == (b < 0) => i64::MAX,
            None => i64::MIN,
        };
        let sum = match self.0.checked_add(product) {
            Some(s) => s,
            None if product < 0 => i64::MIN,
            None => i64::MAX,
        };
        Self(sum)
    }

    /// Gets the underlying accumulated sum value.
    const fn get(self) -> i64 {
        self.0
    }
}

/// Forward DCT of an 8x8 block using integer fixed-point arithmetic.
///
/// Implements the DCT-II formula using pre-computed integer cosine values.
/// All intermediate arithmetic uses i64 to prevent overflow. The cosines are
/// scaled by 2^16, so the sum of products is scaled by 2^32. The C(u), C(v)
/// factors and 1/4 scaling are applied to produce the final coefficients.
///
/// Computed as a row pass followed by a column pass (the DCT's separable
/// property) rather than one flat sum over all 64 `(x, y)` pairs per output
/// cell: `row_sums[u][y]` (independent of `v`) is computed once and shared
/// across all 8 values of `v`, cutting the multiply-accumulate count from
/// 64 to 8 per output cell. This is an exact re-association of the same
/// integer terms in the same (never-saturating, per each type's own bound)
/// arithmetic, not a numerical approximation, so it produces bit-identical
/// output to the flat double sum for every input.
pub(super) fn forward_dct(block: &[CenteredSample; 64]) -> [i32; 64] {
    let mut output = [0_i32; 64];
    let (block_rows, _) = block.as_chunks::<8>();

    // Row pass: row_sums[u][y] = sum over x of f(x,y) * cos((2x+1)*u*π/16).
    let mut row_sums = [[RowSum::ZERO; 8]; 8];
    for (cos_u, sums_row) in COSINES.iter().zip(row_sums.iter_mut()) {
        for (block_row, slot) in block_rows.iter().zip(sums_row.iter_mut()) {
            let mut sum = RowSum::ZERO;
            for (&cos_u_x, &sample) in cos_u.iter().zip(block_row.iter()) {
                sum = sum.add_term(sample, CosU(cos_u_x));
            }
            *slot = sum;
        }
    }

    // Zip COSINES rather than index it. The loops already run in lockstep with
    // the table, so pairing them removes the index instead of asserting that
    // the index is in range.
    for ((v, cos_v), out_row) in COSINES
        .iter()
        .enumerate()
        .zip(output.as_chunks_mut::<8>().0.iter_mut())
    {
        for (u, (out_cell, sums_row)) in out_row.iter_mut().zip(row_sums.iter()).enumerate() {
            // Column pass: sum of f(x,y) * cos_u(x) * cos_v(y), grouped as
            // sum over y of row_sums[u][y] * cos_v(y). All cosines are
            // scaled by 2^16, so this sum is scaled by 2^32, same as the
            // flat double sum it replaces.
            let mut sum = DctSum::ZERO;

            for (&cos_v_y, &row_sum) in cos_v.iter().zip(sums_row.iter()) {
                sum = sum.add_row_term(row_sum, CosV(cos_v_y));
            }

            // Apply DCT normalization:
            // F(u,v) = (1/4) * C(u) * C(v) * sum
            // where C(0) = 1/√2 and C(k) = 1 for k > 0
            //
            // With cosines scaled by 2^16, sum is scaled by 2^32.
            // After dividing by 2^32 to remove cosine scaling, apply C(u)*C(v)/4:
            //
            // For (0,0): divide by 4 * √2 * √2 = 8
            // For u=0, v>0: divide by 4 * √2 = 4√2 ≈ 5.66
            // For u>0, v=0: divide by 4 * √2 = 4√2 ≈ 5.66
            // For u>0, v>0: divide by 4
            //
            // In fixed point: use integer approximations
            let divisor = if u == 0 && v == 0 {
                DCT_DIVISOR_DC
            } else if u == 0 || v == 0 {
                DCT_DIVISOR_EDGE
            } else {
                DCT_DIVISOR_INTERIOR
            };

            let dct_value = div_signed(sum.get(), divisor);
            *out_cell = saturate_to_i32(dct_value);
        }
    }

    output
}
