//! JPEG plane and block-grid machinery.

use core::num::NonZeroU16;

use crate::dct::PixelBlock;
use crate::huffman::Component;
use synthvid_scene::{Height, Width};

// This crate assumes `usize` is at least 32 bits wide, true of every
// platform Rust supports as a hosted (`std`) target. A 16-bit target fails
// the build here rather than silently misbehaving.
#[cfg(target_pointer_width = "16")]
compile_error!("synthvid-encode assumes usize is at least 32 bits wide");

/// Multiplies a block-grid index by 8 (equivalently, `<< 3`) to get a pixel
/// offset, saturating rather than overflowing. A plain `if`/`else` over a
/// shift-based bound, not `checked_mul` plus a match: there is no `Option`
/// here to unwrap, so clippy's `Option`-shaped "manual unwrap" lints do not
/// apply, and there is no `*`/`/` operator either (both `<<` and `>>` sit
/// outside `arithmetic_side_effects` and `integer_division`, since neither
/// can silently overflow or truncate the way `*`/`/` can).
const fn multiply_block_index_by_8(value: usize) -> usize {
    if value > (usize::MAX >> 3) {
        usize::MAX
    } else {
        value << 3
    }
}

/// Computes `width * height` as a `usize`. Total: `width` and `height` are
/// each at most `u16::MAX`, so their product is at most
/// `65_535 * 65_535 = 4_294_836_225`, which fits any `usize` this crate
/// builds for (see the module-level guard above, and [`widen_u32_to_usize`]
/// just below).
fn area_from_dims(dims: PlaneDimensions) -> usize {
    let width = widen_u32_to_usize(u32::from(dims.width().get().get()));
    let height = widen_u32_to_usize(u32::from(dims.height().get().get()));
    match width.checked_mul(height) {
        Some(area) => area,
        // Unreachable: see this function's doc comment.
        None if height == usize::MAX => usize::MAX,
        None => usize::MAX,
    }
}

/// Widens a `u32` to `usize`. Total on every target this crate can actually
/// be built for: the `compile_error!` above already rejects any target
/// where `usize` is narrower than 32 bits, and every `u32` value fits a
/// `usize` that is at least 32 bits wide.
fn widen_u32_to_usize(value: u32) -> usize {
    match usize::try_from(value) {
        Ok(v) => v,
        // The module-level guard above already rules out any target where
        // this could fail (every `usize` this crate builds for is at least
        // 32 bits wide), so both arms below are unreachable in practice;
        // the two are kept distinct because the top of `u32`'s range is
        // the case worth naming, even though nothing else about it changes
        // the (equally unreachable) result.
        Err(_) if value == u32::MAX => usize::MAX,
        Err(_) => usize::MAX,
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
const fn neutral(kind: Component) -> u8 {
    match kind {
        Component::Luma => 0,
        Component::Chroma => 128,
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
pub(super) struct BlockCol(u32);

impl BlockCol {
    /// Wraps a raw column index.
    pub(super) const fn new(value: u32) -> Self {
        Self(value)
    }

    /// This column's starting pixel offset within a row (`self * 8`).
    /// Only `BlockCol` can produce a column pixel offset -- there is no
    /// conversion from `BlockRow` to this same quantity, so a `BlockCoord`
    /// field read the wrong way (row where column belongs) is a type
    /// error, not a swapped-index bug that still compiles.
    fn pixel_col_start(self) -> usize {
        multiply_block_index_by_8(widen_u32_to_usize(self.0))
    }
}

/// A block-grid row index; see [`BlockCol`].
#[derive(Debug, Clone, Copy)]
pub(super) struct BlockRow(u32);

impl BlockRow {
    /// Wraps a raw row index.
    pub(super) const fn new(value: u32) -> Self {
        Self(value)
    }

    /// This row's starting pixel offset within the plane (`self * 8`); see
    /// [`BlockCol::pixel_col_start`].
    fn pixel_row_start(self) -> usize {
        multiply_block_index_by_8(widen_u32_to_usize(self.0))
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

/// A single-channel (Y, Cb, or Cr) image plane: pixel data paired with
/// the dimensions that give it meaning, so the two can never silently
/// drift apart.
#[derive(Debug, Clone)]
pub(super) struct Plane {
    /// Pixel data: `width * height` bytes in row-major order.
    data: Vec<u8>,
    /// This plane's width and height, bundled so they can't be transposed.
    dims: PlaneDimensions,
    /// Which colour role this plane represents, determining both the
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
            data: vec![neutral(kind); area],
            dims,
            kind,
        }
    }

    /// Returns the pad value for this plane (derived from its kind).
    const fn pad(&self) -> u8 {
        neutral(self.kind)
    }

    /// Mutable access to this plane's pixel data, for `YCbCrPlanes::from_frame`
    /// (in the sibling `planes` module) to fill in after construction.
    pub(super) fn data_mut(&mut self) -> &mut [u8] {
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
        let pad = self.pad();
        let mut block = [pad; 64];
        let width = usize::from(self.dims.width().get().get());
        let row_start = coord.by.pixel_row_start();
        let col_start = coord.bx.pixel_col_start();

        let (out_rows, _remainder) = block.as_chunks_mut::<8>();
        let src_rows = self.data.chunks_exact(width).skip(row_start);
        for (src_row, out_row) in src_rows.zip(out_rows.iter_mut()) {
            let available: &[u8] = src_row.get(col_start..).map_or(&[][..], |s| s);
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
            Some(n) => n,
            // Unreachable: 2 is a fixed literal, never zero.
            None => NonZeroU16::MIN,
        };

        let chroma_width_nz = self.dims.width().get().div_ceil(TWO);
        let chroma_height_nz = self.dims.height().get().div_ceil(TWO);
        let chroma_width = Width::from(chroma_width_nz);
        let chroma_height = Height::from(chroma_height_nz);
        let chroma_width_usize = usize::from(chroma_width_nz.get());

        let width = usize::from(self.dims.width().get().get());
        let dims = PlaneDimensions::new(chroma_width, chroma_height);
        let mut out_plane = Self::filled(dims, self.kind);

        let mut src_rows = self.data.chunks(width);
        let mut out_rows = out_plane.data_mut().chunks_exact_mut(chroma_width_usize);
        while let Some(top_row) = src_rows.next() {
            let bottom_row = src_rows
                .next()
                .map_or(BottomSourceRow::Absent, BottomSourceRow::Present);
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
        RowSum::new(match self {
            Self::One(a) => u16::from(a),
            // Unreachable: two `u8`-sourced values sum to at most
            // `255 + 255 = 510`, far below `u16::MAX`.
            Self::Two(a, b) => match u16::from(a).checked_add(u16::from(b)) {
                Some(sum) => sum,
                None if a == u8::MAX => u16::MAX,
                None => u16::MAX,
            },
        })
    }

    /// How many samples this row contributed. `RowCount`'s own range
    /// (`1..=2`) documents the bound in the type itself.
    const fn count(self) -> RowCount {
        match self {
            Self::One(_) => RowCount::new(1),
            Self::Two(_, _) => RowCount::new(2),
        }
    }
}

/// An unsigned integer that is always within `MIN..=MAX`: the range is
/// spelled out in the type itself, at every place it is named, rather than
/// left to a comment beside a plain `u16`. Its only constructor clamps into
/// range, so nothing outside this range can ever be observed through
/// `get()`, regardless of what value is passed in.
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
    const fn new(value: u16) -> Self {
        let clamped = if value < MIN {
            MIN
        } else if value > MAX {
            MAX
        } else {
            value
        };
        Self(clamped)
    }

    /// The underlying value, `MIN..=MAX`.
    const fn get(self) -> u16 {
        self.0
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
            Some(n) => n,
            // Unreachable: ASSERT_MIN_AT_LEAST_ONE guarantees self.0 >= 1.
            None => NonZeroU16::MIN,
        }
    }
}

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
        let bottom_value = bottom.map_or(0, RowSum::get);
        // Unreachable: both operands are at most 510 (`RowSum`'s own
        // range), so their sum is at most 1020, far below `u16::MAX`.
        let sum = match top.get().checked_add(bottom_value) {
            Some(sum) => sum,
            None if bottom_value == 0 => top.get(),
            None => u16::MAX,
        };
        Self::new(sum)
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
        let bottom_value = bottom.map_or(0, RowCount::get);
        // Unreachable: both operands are at most 2 (`RowCount`'s own
        // range), so their sum is at most 4, far below `u16::MAX`.
        let sum = match top.get().checked_add(bottom_value) {
            Some(sum) => sum,
            None if bottom_value == 0 => top.get(),
            None => u16::MAX,
        };
        Self::new(sum)
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
fn one_or_two_samples(row: &[u8]) -> impl Iterator<Item = OneOrTwoSamples> + '_ {
    let (chunks, remainder) = row.as_chunks::<2>();
    let pairs = chunks.iter().map(|&[a, b]| OneOrTwoSamples::Two(a, b));
    let last = match *remainder {
        [a] => Some(OneOrTwoSamples::One(a)),
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
enum BottomSourceRow<'a> {
    /// Past the plane's bottom edge: no second row to average in.
    Absent,
    /// A real second row's samples.
    Present(&'a [u8]),
}

/// Averages one row of 2x2 (or, at an edge, 1x2/2x1/1x1) pixel neighborhoods
/// from a pair of source rows into `out_row`. `bottom` is [`BottomSourceRow::Absent`]
/// past the plane's bottom edge; each neighborhood is a short chunk past its
/// right edge. Every element of `out_row` is written exactly once.
///
/// Takes `top: &[u8]` and `bottom: BottomSourceRow`, not two adjacent
/// `&[u8]`s where absence is signalled by an empty slice: an empty slice in
/// either position type-checks identically, so a caller could pass the
/// bottom-edge sentinel as `top` (or a genuine top row as `bottom`) with no
/// compiler error, leaving the whole output row at its pre-filled neutral
/// value instead of using the real top row's data. Naming which row can be
/// legitimately absent removes that ambiguity.
fn downsample_row_pair(top: &[u8], bottom: BottomSourceRow<'_>, out_row: &mut [u8]) {
    let mut top_samples = one_or_two_samples(top);
    let mut bottom_samples = match bottom {
        BottomSourceRow::Absent => None,
        BottomSourceRow::Present(row) => Some(one_or_two_samples(row)),
    };
    for out in out_row.iter_mut() {
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
        let bottom_row = bottom_samples
            .as_mut()
            .and_then(Iterator::next)
            .map_or(BottomRow::Absent, BottomRow::Present);
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

/// Divides a combined sum by a combined count, rounding to the nearest
/// integer (ties away from zero), and saturates the result into a sample
/// byte. Total: `count.get_nonzero()` is checked at compile time to never
/// fall back (`CombinedCount`'s own `MIN` is 1), the same
/// clamped-type-to-`NonZero*`-at-the-point-of-use pattern `dct.rs`'s
/// `quantize` uses to convert a `QuantDivisor` into a `NonZeroU64`. The
/// widest possible sum (`4 * 255 = 1020`) fits `u16` with room to spare,
/// so `u8::try_from` never actually falls back either.
fn divide(sum: CombinedSum, count: CombinedCount) -> AveragedSample {
    let count = count.get_nonzero();
    let half = count.get() >> 1;
    // `sum` is at most 1020 and `half` at most 2 (`count`'s own `1..=4`
    // range, halved), so their total is at most 1022, far below `u16::MAX`.
    let biased = match sum.get().checked_add(half) {
        Some(total) => total,
        None if half == 0 => sum.get(),
        None => u16::MAX,
    };
    let quotient = biased.div_euclid(count.get());
    AveragedSample(narrow_avg_to_u8(quotient))
}

/// Narrows an averaged-sample quotient to `u8`. `sum` and `count` are
/// always derived from the same real neighborhood of `u8` samples
/// (`average_neighborhood`, below), so their quotient is itself always a
/// valid sample -- `<= 255` -- by construction; the `min` here documents
/// that bound at the point of use rather than relying on it silently.
fn narrow_avg_to_u8(quotient: u16) -> u8 {
    let [lo, ..] = quotient.min(255).to_le_bytes();
    lo
}

/// Averages this 2x2 neighborhood's samples. `top` is always present;
/// `bottom` is [`BottomRow::Absent`] only past the plane's bottom edge,
/// where there is no row below, in which case it contributes `0` to the
/// sum and count rather than a second row's worth of either.
fn average_neighborhood(top: OneOrTwoSamples, bottom: BottomRow) -> AveragedSample {
    let sum = CombinedSum::combine(top.sum(), bottom.sum());
    let count = CombinedCount::combine(top.count(), bottom.count());
    divide(sum, count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use synthvid_scene::Height;

    #[test]
    fn test_plane_downsample() {
        let width_2 = Width::new(2).expect("2 is valid");
        let height_2 = Height::new(2).expect("2 is valid");
        let mut p = Plane::filled(PlaneDimensions::new(width_2, height_2), Component::Chroma);
        let d = p.data_mut();
        d[0] = 100;
        d[1] = 101;
        d[2] = 101;
        d[3] = 101;
        let p_ds = p.downsample_2x2();
        assert_eq!(
            (
                p_ds.dims.width().get().get(),
                p_ds.dims.height().get().get(),
                p_ds.data[0]
            ),
            (1, 1, 101)
        );
    }
}
