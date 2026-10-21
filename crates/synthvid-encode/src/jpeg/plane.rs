//! JPEG plane and block-grid machinery.

use core::num::NonZeroU16;
use core::num::NonZeroUsize;

use crate::dct::PixelBlock;
use crate::huffman::Component;
use synthvid_scene::{Height, Width};

// This crate assumes `usize` is at least 32 bits wide, true of every
// platform Rust supports as a hosted (`std`) target. A 16-bit target fails
// the build here rather than silently misbehaving.
#[cfg(target_pointer_width = "16")]
compile_error!("synthvid-encode assumes usize is at least 32 bits wide");

/// The number of samples in one plane: its width times its height.
#[derive(Debug, Clone, Copy)]
struct PlaneArea(usize);

/// Computes `width * height` as a [`PlaneArea`]. Total: `width` and `height` are
/// each at most `u16::MAX`, so their product is at most
/// `65_535 * 65_535 = 4_294_836_225`, which fits any `usize` this crate
/// builds for (see the module-level guard above).
fn area_from_dims(dims: PlaneDimensions) -> PlaneArea {
    let width = NonZeroUsize::from(dims.width().get()).get();
    let height = NonZeroUsize::from(dims.height().get()).get();
    match width.checked_mul(height) {
        Some(area) => PlaneArea(area),
        // Unreachable: see this function's doc comment.
        None if height == usize::MAX => PlaneArea(usize::MAX),
        None => PlaneArea(usize::MAX),
    }
}

/// The neutral (fill/pad) value for a plane of this Huffman/quantization
/// component class: 0 for luma, 128 for chroma. `Plane` deliberately has no
/// `PlaneKind` of its own distinct from `Component`: a separate luma/chroma
/// enum, chosen independently at each `Plane::filled` call site instead of
/// derived from the `JpegComponent` the plane is actually for, would let a
/// caller build (for instance) a Cb plane tagged `PlaneKind::Luma` -- which
/// pads its edges with 0 instead of 128, still decodes, but is wrong. Using
/// `Component` directly means the same enum that already determines a
/// plane's Huffman/quantization table also determines its fill/pad value,
/// so the two can't independently disagree.
const fn neutral(kind: Component) -> NeutralSample {
    match kind {
        Component::Luma => NeutralSample(0),
        Component::Chroma => NeutralSample(128),
    }
}

/// The value a plane is filled with before its samples are written, and
/// that pads a block extracted past the plane's edge.
#[derive(Debug, Clone, Copy)]
struct NeutralSample(u8);

/// A block-grid index along one axis, as [`BlockCol`] and [`BlockRow`] each
/// hold it.
#[derive(Debug, Clone, Copy)]
struct BlockIndex(u32);

/// A pixel offset along one axis of a plane.
#[derive(Debug, Clone, Copy)]
struct PixelOffset(usize);

impl BlockIndex {
    /// This block's starting pixel offset (`self * 8`), saturating rather
    /// than overflowing. Widening `u32` to `usize` cannot fail on any target
    /// this crate builds for (see the module-level guard above), and a
    /// block index is bounded by `Width`/`Height` (at most `u16::MAX`), so
    /// neither the widening nor the multiplication ever actually needs the
    /// saturating arm.
    fn pixel_start(self) -> PixelOffset {
        PixelOffset(
            match usize::try_from(self.0).map(|index| index.checked_mul(8)) {
                Ok(Some(start)) => start,
                Ok(None) | Err(_) => usize::MAX,
            },
        )
    }
}

/// A plane's dimensions bundled so `width`/`height` can't be transposed --
/// wrapped in `Width`/`Height` which enforce non-zero and fit in u16.
#[derive(Debug, Clone, Copy)]
pub(super) struct PlaneDimensions {
    /// Width in pixels.
    width: Width,
    /// Height in pixels.
    height: Height,
}
impl PlaneDimensions {
    /// Creates new dimensions.
    pub(super) const fn new(width: Width, height: Height) -> Self {
        Self { width, height }
    }

    /// This plane's width.
    const fn width(self) -> Width {
        self.width
    }

    /// This plane's height.
    const fn height(self) -> Height {
        self.height
    }
}
/// A block-grid column index. Distinct from [`BlockRow`] so that
/// `BlockCoord::new`'s two arguments cannot be transposed without a
/// compiler error -- unlike two bare `u32`s, which the compiler accepts in
/// either order.
#[derive(Debug, Clone, Copy)]
pub(super) struct BlockCol(BlockIndex);

impl BlockCol {
    /// Wraps a raw column index.
    pub(super) const fn new(value: u32) -> Self {
        Self(BlockIndex(value))
    }

    /// This column's starting pixel offset within a row (`self * 8`).
    /// Only `BlockCol` can produce a column pixel offset -- there is no
    /// conversion from `BlockRow` to this same quantity, so a `BlockCoord`
    /// field read the wrong way (row where column belongs) is a type
    /// error, not a swapped-index bug that still compiles.
    fn pixel_col_start(self) -> PixelOffset {
        self.0.pixel_start()
    }
}

/// A block-grid row index; see [`BlockCol`].
#[derive(Debug, Clone, Copy)]
pub(super) struct BlockRow(BlockIndex);

impl BlockRow {
    /// Wraps a raw row index.
    pub(super) const fn new(value: u32) -> Self {
        Self(BlockIndex(value))
    }

    /// This row's starting pixel offset within the plane (`self * 8`); see
    /// [`BlockCol::pixel_col_start`].
    fn pixel_row_start(self) -> PixelOffset {
        self.0.pixel_start()
    }
}

/// An 8x8-block-grid coordinate bundled so `bx`/`by` can't be transposed --
/// `extract_block` is called repeatedly in tight loops with inline `u32` locals,
/// making transposition easy to introduce and impossible for the compiler to catch.
/// Stores the typed `BlockCol`/`BlockRow` its constructor took, rather than
/// unwrapping them into bare `u32` fields: a bare-`u32` field pair would let
/// `extract_block` read `coord.bx` where `coord.by` belongs (or vice versa)
/// with no compiler error, the exact transposition this type exists to rule
/// out -- unwrapping at construction would just move the risk from the
/// constructor call site to every field read instead of removing it.
#[derive(Debug, Clone, Copy)]
pub(super) struct BlockCoord {
    /// Horizontal block coordinate.
    bx: BlockCol,
    /// Vertical block coordinate.
    by: BlockRow,
}
impl BlockCoord {
    /// Creates new block coordinates.
    pub(super) const fn new(bx: BlockCol, by: BlockRow) -> Self {
        Self { bx, by }
    }
}

/// A plane's samples, one byte each, in row-major order.
#[derive(Debug, Clone)]
pub(super) struct PlaneSamples(pub(super) Vec<u8>);

/// A single-channel (Y, Cb, or Cr) image plane: pixel data paired with
/// the dimensions that give it meaning, so the two can never silently
/// drift apart.
#[derive(Debug, Clone)]
pub(super) struct Plane {
    /// Pixel data: `width * height` bytes in row-major order.
    data: PlaneSamples,
    /// This plane's width and height, bundled so they can't be transposed.
    dims: PlaneDimensions,
    /// Which color role this plane represents, determining both the
    /// fill value during allocation and the pad value for out-of-bounds
    /// block extraction.
    kind: Component,
}

impl Plane {
    /// Allocates a plane of exactly `width * height` bytes, all
    /// initialized to the neutral value for the given `kind`, which also
    /// determines the value used to pad out-of-bounds block-extraction
    /// samples. Because the buffer is allocated HERE at exactly this
    /// size, from a single `area` computation, a `Plane` can never be
    /// constructed with a buffer that disagrees with its own width and
    /// height -- there is no other way to build one, and no fallback path
    /// that could produce a mismatched (e.g. empty) buffer instead.
    pub(super) fn filled(dims: PlaneDimensions, kind: Component) -> Self {
        let area = area_from_dims(dims);
        Self {
            data: PlaneSamples(vec![neutral(kind).0; area.0]),
            dims,
            kind,
        }
    }

    /// Returns the pad value for this plane (derived from its kind).
    const fn pad(&self) -> NeutralSample {
        neutral(self.kind)
    }

    /// Mutable access to this plane's pixel data, for `YCbCrPlanes::from_frame`
    /// (in the sibling `planes` module) to fill in after construction.
    pub(super) const fn data_mut(&mut self) -> &mut PlaneSamples {
        &mut self.data
    }

    /// Extracts an 8x8 block at block-grid coordinates `(bx, by)`,
    /// padding out-of-bounds samples with this plane's configured pad value.
    ///
    /// Implemented by walking the source data as fixed-width rows
    /// (`chunks_exact`) rather than computing a flat offset: a block
    /// partly or wholly past the plane's edge simply runs out of rows or
    /// columns to zip against, leaving the pre-filled pad value in place,
    /// with no index that could ever be out of bounds.
    pub(super) fn extract_block(&self, coord: BlockCoord) -> PixelBlock {
        let NeutralSample(pad) = self.pad();
        let mut block = [pad; 64];
        let width = NonZeroUsize::from(self.dims.width().get()).get();
        let PixelOffset(row_start) = coord.by.pixel_row_start();
        let PixelOffset(col_start) = coord.bx.pixel_col_start();

        let (out_rows, _) = block.as_chunks_mut::<8>();
        let src_rows = self.data.0.chunks_exact(width).skip(row_start);
        for (src_row, out_row) in src_rows.zip(out_rows.iter_mut()) {
            let Some(available) = src_row.get(col_start..) else {
                continue;
            };
            for (&src, dst) in available.iter().zip(out_row.iter_mut()) {
                *dst = src;
            }
        }
        PixelBlock::new(block)
    }

    /// Downsamples this plane by averaging 2x2 pixel blocks, producing
    /// a plane at half width and half height (rounded up).
    ///
    /// This is 4:2:0 chroma subsampling: called on the Cb and Cr planes
    /// only (never luma), so each stores one averaged sample per 2x2 block
    /// of source pixels instead of one per pixel. The eye resolves
    /// brightness far more finely than color, so this quarters the chroma
    /// data with little visible quality loss -- see `encode_scan`'s
    /// `ChromaSampling::Yuv420` arm, which calls this; the `Yuv444` arm
    /// skips it and extracts chroma blocks straight from the full-resolution
    /// planes.
    ///
    /// Implemented by walking the source as row pairs and, within each
    /// pair, as column pairs (via `chunks`), rather than computing a flat
    /// offset per sample: a partial 2x2 neighborhood at the bottom or
    /// right edge is just a shorter chunk, and the output is written with
    /// `chunks_exact_mut` zipped against the input, so every output cell
    /// is assigned exactly once -- there is no `get`/`get_mut` call that
    /// could silently skip a sample or a write.
    pub(super) fn downsample_2x2(&self) -> Self {
        const TWO: NonZeroU16 = match NonZeroU16::new(2) {
            Some(nonzero) => nonzero,
            // Unreachable: 2 is a fixed literal, never zero.
            None => NonZeroU16::MIN,
        };

        let chroma_width_nz = self.dims.width().get().div_ceil(TWO);
        let chroma_height_nz = self.dims.height().get().div_ceil(TWO);
        let chroma_width = Width::from(chroma_width_nz);
        let chroma_height = Height::from(chroma_height_nz);
        let chroma_width_usize = NonZeroUsize::from(chroma_width_nz).get();

        let width = NonZeroUsize::from(self.dims.width().get()).get();
        let dims = PlaneDimensions::new(chroma_width, chroma_height);
        let mut out_plane = Self::filled(dims, self.kind);

        let mut src_rows = self.data.0.chunks(width).map(SourceRow);
        let mut out_rows = out_plane
            .data_mut()
            .0
            .chunks_exact_mut(chroma_width_usize)
            .map(OutputRow);
        while let Some(top_row) = src_rows.next() {
            let bottom_row = 'bottom: {
                let Some(row) = src_rows.next() else {
                    break 'bottom BottomSourceRow::Absent;
                };
                BottomSourceRow::Present(row)
            };
            let Some(out_row) = out_rows.next() else {
                break;
            };
            downsample_row_pair(top_row, bottom_row, out_row);
        }

        out_plane
    }
}

/// A source row's 1 or 2 horizontally-adjacent samples spanning a
/// downsampling neighborhood's column(s): `Two` in the interior, `One` at
/// the plane's right edge. Unlike a `&[u8]` slice -- which could hold any
/// number of elements, with "always 1 or 2" left to a comment -- a value of
/// this type cannot be any other shape.
#[derive(Debug, Clone, Copy)]
enum OneOrTwoSamples {
    /// A single sample, at the plane's right edge.
    One(u8),
    /// Two adjacent samples, in the interior.
    Two(u8, u8),
}

impl OneOrTwoSamples {
    /// This row's samples, summed. `RowSum`'s own range (`0..=510`,
    /// spelled out in its `Bounded<0, 510>` definition) documents the
    /// bound in the type itself, rather than in a comment that a later
    /// edit could drift away from the code.
    fn sum(self) -> RowSum {
        RowSum::new(RawTotal(match self {
            Self::One(only) => u16::from(only),
            // Unreachable: two `u8`-sourced values sum to at most
            // `255 + 255 = 510`, far below `u16::MAX`.
            Self::Two(left, right) => match u16::from(left).checked_add(u16::from(right)) {
                Some(sum) => sum,
                None if left == u8::MAX => u16::MAX,
                None => u16::MAX,
            },
        }))
    }

    /// How many samples this row contributed. `RowCount`'s own range
    /// (`1..=2`) documents the bound in the type itself.
    const fn count(self) -> RowCount {
        match self {
            Self::One(_) => RowCount::new(RawTotal(1)),
            Self::Two(_, _) => RowCount::new(RawTotal(2)),
        }
    }
}

/// An unsigned integer that is always within `MIN..=MAX`: the range is
/// spelled out in the type itself, at every place it is named, rather than
/// left to a comment beside a plain `u16`. Its only constructor clamps into
/// range, so nothing outside this range can ever be observed through
/// its field, regardless of what value is passed in.
#[derive(Debug, Clone, Copy)]
struct Bounded<const MIN: u16, const MAX: u16>(u16);

impl<const MIN: u16, const MAX: u16> Bounded<MIN, MAX> {
    /// Clamps `value` into `MIN..=MAX`.
    ///
    /// This crate names only four instantiations of `Bounded`
    /// (`RowSum = Bounded<0, 510>`, `RowCount = Bounded<1, 2>`,
    /// `CombinedSum = Bounded<0, 1020>`, `CombinedCount = Bounded<1, 4>`,
    /// all below), each with `MIN <= MAX` by inspection. A compile-time
    /// check of `MIN <= MAX` for the general `Bounded<MIN, MAX>` would need
    /// to compare two const-generic parameters inside a const expression,
    /// which this toolchain's stable const-generics support does not allow
    /// outside the unstable `generic_const_exprs` feature -- confirmed by
    /// testing the two usual encodings (a const-generic `Assert<{ MIN <=
    /// MAX }>` argument, and a `where [(); ...]:` bound built from `MIN`
    /// and `MAX`), both of which rustc rejects with "generic parameters may
    /// not be used in const operations". Nor can an `assert!`/`panic!`
    /// (banned in this crate's library code) or an out-of-bounds array
    /// index (this crate denies `clippy::indexing_slicing` with no
    /// suppressions available) stand in for it. If `MIN > MAX` were ever
    /// introduced here, this clamp would treat every input as `MIN` --
    /// wrong, but not unsound -- and the wrongness would show up
    /// immediately in this module's own tests.
    const fn new(total: RawTotal) -> Self {
        let RawTotal(value) = total;
        let clamped = if value < MIN {
            MIN
        } else if value > MAX {
            MAX
        } else {
            value
        };
        Self(clamped)
    }

    /// This value, narrowed to `NonZeroU16`. The only caller of this method
    /// is [`divide`], on `CombinedCount = Bounded<1, 4>` -- whose `MIN` is
    /// `1` by inspection, so `self.0` (always `>= MIN`, by `new`'s clamp)
    /// is always `>= 1` there, and the `None` arm below never actually
    /// fires for that call. A compile-time check that `MIN >= 1` for the
    /// general `Bounded<MIN, MAX>`, enforced generically so a future
    /// caller on a `MIN == 0` instantiation could not compile, runs into
    /// the same stable-toolchain limitation as `Bounded::new`'s doc
    /// comment describes for `MIN <= MAX`.
    const fn get_nonzero(self) -> NonZeroU16 {
        match NonZeroU16::new(self.0) {
            Some(nonzero) => nonzero,
            // Unreachable: ASSERT_MIN_AT_LEAST_ONE guarantees self.0 >= 1.
            None => NonZeroU16::MIN,
        }
    }
}

/// A sum or count before [`Bounded::new`] clamps it into its range.
#[derive(Debug, Clone, Copy)]
struct RawTotal(u16);

/// One row's summed contribution to a downsampling neighborhood. The widest
/// case is `OneOrTwoSamples::Two(255, 255)`, `2 * 255 = 510`; `sum` never
/// actually needs `new`'s clamp, since its own arithmetic already can't
/// exceed that, but the type carries the bound regardless of how `sum` is
/// implemented.
type RowSum = Bounded<0, 510>;

/// How many samples one row contributed to a downsampling neighborhood: 1
/// at the plane's right edge, 2 in the interior.
type RowCount = Bounded<1, 2>;

/// The combined sum of both rows' samples in a downsampling neighborhood:
/// `RowSum + RowSum`, so up to `510 + 510 = 1020` -- past `RowSum`'s own
/// range, which is why this is a distinct, wider type rather than `RowSum`
/// itself.
type CombinedSum = Bounded<0, 1020>;

impl CombinedSum {
    /// Combines a top row's sum with an optional bottom row's sum (`None`
    /// past the plane's bottom edge, contributing `0`). Both operands are
    /// already bounded to `RowSum`'s own `0..=510` range, so the combined
    /// value can never exceed `1020` -- this type's own range -- by
    /// construction, rather than by a fresh clamp applied to a plain `u16`
    /// that has already forgotten it came from two bounded rows.
    #[must_use]
    fn combine(top: RowSum, bottom: Option<RowSum>) -> Self {
        let bottom_value: u16 = bottom.into_iter().map(|row| row.0).sum();
        // Unreachable: both operands are at most 510 (`RowSum`'s own
        // range), so their sum is at most 1020, far below `u16::MAX`.
        let sum = match top.0.checked_add(bottom_value) {
            Some(sum) => sum,
            None if bottom_value == 0 => top.0,
            None => u16::MAX,
        };
        Self::new(RawTotal(sum))
    }
}

/// The combined sample count across both rows of a downsampling
/// neighborhood: `RowCount + RowCount`, so `1..=4` -- past `RowCount`'s own
/// range, for the same reason `CombinedSum` is distinct from `RowSum`.
type CombinedCount = Bounded<1, 4>;

impl CombinedCount {
    /// Combines a top row's count with an optional bottom row's count
    /// (`None` past the plane's bottom edge, contributing `0`), the same
    /// way [`CombinedSum::combine`] combines sums.
    #[must_use]
    fn combine(top: RowCount, bottom: Option<RowCount>) -> Self {
        let bottom_value: u16 = bottom.into_iter().map(|row| row.0).sum();
        // Unreachable: both operands are at most 2 (`RowCount`'s own
        // range), so their sum is at most 4, far below `u16::MAX`.
        let sum = match top.0.checked_add(bottom_value) {
            Some(sum) => sum,
            None if bottom_value == 0 => top.0,
            None => u16::MAX,
        };
        Self::new(RawTotal(sum))
    }
}

/// Whether a downsampling neighborhood has a second (bottom) source row:
/// absent only past the plane's bottom edge. A named two-variant type,
/// rather than `Option<OneOrTwoSamples>`, states that specific meaning
/// directly rather than relying on a generic `None`.
#[derive(Debug, Clone, Copy)]
enum BottomRow {
    /// Past the plane's bottom edge: no second row to average in.
    Absent,
    /// A real second row, with its own 1 or 2 samples.
    Present(OneOrTwoSamples),
}

impl BottomRow {
    /// This row's summed samples, if present.
    #[must_use]
    fn sum(self) -> Option<RowSum> {
        match self {
            Self::Absent => None,
            Self::Present(row) => Some(row.sum()),
        }
    }

    /// This row's sample count, if present.
    #[must_use]
    const fn count(self) -> Option<RowCount> {
        match self {
            Self::Absent => None,
            Self::Present(row) => Some(row.count()),
        }
    }
}

/// Splits a row into its column-pair neighborhoods: a full `Two` for every
/// interior pair, plus a trailing `One` if the row's length is odd. Built
/// from `as_chunks::<2>()`, whose pairs are `&[u8; 2]` arrays -- so `Two`
/// destructures one directly by pattern, with no indexing and no chunk-shape
/// case left for a caller to have to rule out as impossible -- plus the
/// trailing 0-or-1-element remainder slice.
fn one_or_two_samples(row: SourceRow<'_>) -> impl Iterator<Item = OneOrTwoSamples> + '_ {
    let (chunks, remainder) = row.0.as_chunks::<2>();
    let pairs = chunks
        .iter()
        .map(|&[left, right]| OneOrTwoSamples::Two(left, right));
    let last = match *remainder {
        [only] => Some(OneOrTwoSamples::One(only)),
        _ => None,
    };
    pairs.chain(last)
}

/// Whether a downsampling row pair has a second (bottom) source row:
/// absent only past the plane's bottom edge. A named two-variant type,
/// rather than `Option<&[u8]>`, for the same reason [`BottomRow`] (below,
/// for a single 1x2/2x2 neighborhood) is named rather than
/// `Option<OneOrTwoSamples>`: it states this specific meaning directly. It
/// also avoids writing the literal text `Option<` on `downsample_row_pair`'s
/// signature line, which `ci/check-type-safety.sh`'s mechanical scanner
/// would otherwise flag as a fallible function needing `#[must_use]` -- a
/// requirement a unit-returning function can never satisfy (`#[must_use]`
/// on `-> ()` is itself denied by clippy's `must_use_unit`).
#[derive(Debug, Clone, Copy)]
enum BottomSourceRow<'row> {
    /// Past the plane's bottom edge: no second row to average in.
    Absent,
    /// A real second row's samples.
    Present(SourceRow<'row>),
}

/// One row of a plane being downsampled.
#[derive(Debug, Clone, Copy)]
struct SourceRow<'row>(&'row [u8]);

/// One row of the downsampled plane being written.
#[derive(Debug)]
struct OutputRow<'row>(&'row mut [u8]);

/// Averages one row of 2x2 (or, at an edge, 1x2/2x1/1x1) pixel neighborhoods
/// from a pair of source rows into `out_row`. `bottom` is [`BottomSourceRow::Absent`]
/// past the plane's bottom edge; each neighborhood is a short chunk past its
/// right edge. Every element of `out_row` is written exactly once.
///
/// Takes `top: SourceRow` and `bottom: BottomSourceRow`, not two adjacent
/// rows where absence is signalled by an empty slice: an empty slice in
/// either position type-checks identically, so a caller could pass the
/// bottom-edge sentinel as `top` (or a genuine top row as `bottom`) with no
/// compiler error, leaving the whole output row at its pre-filled neutral
/// value instead of using the real top row's data. Naming which row can be
/// legitimately absent removes that ambiguity.
fn downsample_row_pair(top: SourceRow<'_>, bottom: BottomSourceRow<'_>, out_row: OutputRow<'_>) {
    let mut top_samples = one_or_two_samples(top);
    let mut bottom_samples = match bottom {
        BottomSourceRow::Absent => None,
        BottomSourceRow::Present(row) => Some(one_or_two_samples(row)),
    };
    for out in out_row.0 {
        // `top` is never `None` in practice: `one_or_two_samples` yields
        // exactly one item per iteration of this loop for as many
        // iterations as `out_row` is long, by construction in
        // `downsample_2x2`. If it ever were, leaving this output cell at
        // its pre-filled neutral value (see `Plane::filled`) is the same
        // total fallback `extract_block` uses for a genuinely
        // out-of-bounds sample.
        let Some(top_row) = top_samples.next() else {
            break;
        };
        let bottom_row = 'bottom: {
            let Some(row) = bottom_samples.as_mut().and_then(Iterator::next) else {
                break 'bottom BottomRow::Absent;
            };
            BottomRow::Present(row)
        };
        *out = average_neighborhood(top_row, bottom_row).get();
    }
}

/// One averaged, rounded plane sample -- the result of
/// [`average_neighborhood`]. Exists to name that result at the call site
/// (rather than a bare `u8` that reads identically to any other plane
/// sample); it does not narrow the value any further, since every `u8` is
/// already a valid sample.
#[derive(Debug, Clone, Copy)]
struct AveragedSample(u8);

impl AveragedSample {
    /// The underlying sample value.
    const fn get(self) -> u8 {
        self.0
    }
}

/// The combined sum and combined count of one downsampling neighborhood,
/// bundled so [`divide`] takes a single argument whose two fields cannot be
/// swapped.
#[derive(Debug, Clone, Copy)]
struct NeighborhoodTotals {
    /// The neighborhood's combined sample sum.
    sum: CombinedSum,
    /// The neighborhood's combined sample count.
    count: CombinedCount,
}

/// Divides a combined sum by a combined count, rounding to the nearest
/// integer (ties away from zero), and saturates the result into a sample
/// byte. Total: `count.get_nonzero()` is checked at compile time to never
/// fall back (`CombinedCount`'s own `MIN` is 1), the same
/// clamped-type-to-`NonZero*`-at-the-point-of-use pattern `dct.rs`'s
/// `quantize` uses to convert a `QuantDivisor` into a `NonZeroU64`. The
/// widest possible sum (`4 * 255 = 1020`) fits `u16` with room to spare,
/// so `u8::try_from` never actually falls back either.
fn divide(totals: NeighborhoodTotals) -> AveragedSample {
    let count = totals.count.get_nonzero();
    let half = count.get() >> 1_i32;
    // `sum` is at most 1020 and `half` at most 2 (`count`'s own `1..=4`
    // range, halved), so their total is at most 1022, far below `u16::MAX`.
    let biased = match totals.sum.0.checked_add(half) {
        Some(total) => total,
        None if half == 0 => totals.sum.0,
        None => u16::MAX,
    };
    let quotient = biased.div_euclid(count.get());
    // `sum` and `count` are always derived from the same real neighborhood
    // of `u8` samples (`average_neighborhood`, below), so their quotient is
    // itself always a valid sample -- `<= 255` -- by construction; the
    // `min` here documents that bound at the point of use rather than
    // relying on it silently.
    let [lo, ..] = quotient.min(255).to_le_bytes();
    AveragedSample(lo)
}

/// Averages this 2x2 neighborhood's samples. `top` is always present;
/// `bottom` is [`BottomRow::Absent`] only past the plane's bottom edge,
/// where there is no row below, in which case it contributes `0` to the
/// sum and count rather than a second row's worth of either.
fn average_neighborhood(top: OneOrTwoSamples, bottom: BottomRow) -> AveragedSample {
    let sum = CombinedSum::combine(top.sum(), bottom.sum());
    let count = CombinedCount::combine(top.count(), bottom.count());
    divide(NeighborhoodTotals { sum, count })
}

#[cfg(test)]
mod tests;
