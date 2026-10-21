//! JPEG file assembly and encoding.
use crate::jpeg_markers::{write_app0, write_dht, write_dqt, write_sof0, write_sos};

use crate::ac_encoding::encode_ac_coefficients;
use crate::bitstream::BitstreamWriter;
use crate::byte_views::ByteSink;
pub use crate::config::{ChromaSampling, Quality};
use crate::config::{SamplingFactor, SubsampleFactor};
use crate::dct::encode_block;
use crate::huffman::{Component, TableDestination};
use crate::media::EncodedFrame;
use crate::quant::{self, QuantTable};
use core::num::NonZeroU32;
use synthvid_scene::Frame;

mod plane;
use plane::{BlockCol, BlockCoord, BlockRow};

mod planes;
use planes::{ActivePlanes, YCbCrPlanes};

mod predictor;
use predictor::DcPredictors;

mod types;
use types::{ComponentId, McuExtent};

/// A JPEG component's identity: exactly the three this encoder ever
/// produces.
///
/// A closed set held as an enum, not a `u8` id plus a separately-stored
/// `class` -- both the marker id (1/2/3) and the Huffman/quantization
/// table class (luma/chroma) are derived from the single variant below,
/// so they can never independently disagree, and there is no way to
/// build a fourth, meaningless component.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum JpegComponent {
    /// Y (luma) component.
    Luma,
    /// Cb (blue chroma) component.
    Cb,
    /// Cr (red chroma) component.
    Cr,
}

impl JpegComponent {
    /// The two chroma components, in wire order (Cb, Cr). The 4:2:0 scan's
    /// per-MCU chroma loop derives from this instead of a separate
    /// hand-written `[Cb, Cr]` literal, so it cannot drift from `ALL`
    /// (below) -- e.g. by dropping `Cr` or listing `Cb` twice, which a
    /// second independent literal could do with no compiler error.
    pub(crate) const CHROMA: [Self; 2] = [Self::Cb, Self::Cr];

    /// All three components this encoder ever emits, in wire order (Y, Cb,
    /// Cr). SOF0 and SOS each write one marker entry per component, and a
    /// 4:4:4 scan encodes one block per component per MCU; deriving each of
    /// those loops from this single array, instead of writing out `[Y, Cb,
    /// Cr]` (or three individual calls) at every site, means they cannot
    /// drift out of sync with each other -- e.g. a marker that lists Y, Cb,
    /// Cr but a scan loop that only encodes Y, Cb. Built from `Y` followed
    /// by `CHROMA`'s own elements, rather than as its own independent
    /// `[Y, Cb, Cr]` literal, so the two arrays cannot disagree about which
    /// components are chroma.
    pub(crate) const ALL: [Self; 3] = [Self::Luma, Self::CHROMA[0], Self::CHROMA[1]];

    /// This component's marker ID byte (1 = Y, 2 = Cb, 3 = Cr), as SOF0
    /// and SOS both require.
    #[must_use]
    const fn id(self) -> ComponentId {
        match self {
            Self::Luma => ComponentId(1),
            Self::Cb => ComponentId(2),
            Self::Cr => ComponentId(3),
        }
    }

    /// Gets this component's quantization table index. Derived from
    /// `huffman_class`, which is the single source of truth for every
    /// table destination this component uses.
    #[must_use]
    const fn quant_table(self) -> TableDestination {
        self.huffman_class().table_dest()
    }

    /// Gets this component's DC Huffman table index. Derived from
    /// `huffman_class`; see [`Self::quant_table`].
    #[must_use]
    const fn dc_table(self) -> TableDestination {
        self.huffman_class().table_dest()
    }

    /// Writes this component's entry in a DQT (Define Quantization Table)
    /// marker: the byte combining the table-class nibble (0) with the
    /// quantization-table destination index.
    pub(crate) fn write_dqt_entry(self, sink: ByteSink<'_>) {
        let ByteSink(output) = sink;
        output.push(self.quant_table().0);
    }

    /// The sampling factor this component uses under a given chroma
    /// sampling mode: `Y` follows the mode's own factor (`(2, 2)` under
    /// 4:2:0), while `Cb`/`Cr` are always `ONE_TO_ONE` -- they are the
    /// reference grid the luma factor is expressed relative to, not
    /// something the caller separately decides. Deriving both from one
    /// `(component, chroma_sampling)` pair, instead of a caller picking a
    /// `SubsampleFactor` to pass alongside the component, closes off the
    /// mismatch this type made representable: nothing used to stop a call
    /// site from pairing `Y` with `ONE_TO_ONE` or `Cb` with the mode's real
    /// factor.
    const fn sampling(self, chroma_sampling: ChromaSampling) -> SubsampleFactor {
        match self {
            Self::Luma => chroma_sampling.subsample_factor(),
            Self::Cb | Self::Cr => SubsampleFactor::ONE_TO_ONE,
        }
    }

    /// Writes this component's entry in a SOF0 (Start of Frame) marker:
    /// the component ID, sampling factors (high nibble = H, low nibble = V),
    /// and quantization table destination index. A `SubsampleFactor` holds
    /// one factor for both axes, so the two nibbles are always equal and
    /// the byte is named per factor rather than packed from two lookups.
    pub(crate) fn write_sof_component(self, sink: ByteSink<'_>, chroma_sampling: ChromaSampling) {
        let ByteSink(output) = sink;
        let factor = self.sampling(chroma_sampling);
        output.push(self.id().0);
        output.push(match factor.horizontal() {
            SamplingFactor::One => 0x11,
            SamplingFactor::Two => 0x22,
        });
        output.push(self.quant_table().0);
    }

    /// Writes this component's DC Huffman table class-and-destination byte
    /// for a DHT (Define Huffman Table) marker: the class nibble (0) with
    /// the DC table destination index.
    pub(crate) fn write_dht_dc_entry(self, sink: ByteSink<'_>) {
        let ByteSink(output) = sink;
        output.push(self.dc_table().0);
    }

    /// Writes this component's AC Huffman table class-and-destination byte
    /// for a DHT (Define Huffman Table) marker: the class nibble (1) with
    /// the AC table destination index (`0` for luma, `1` for chroma, as
    /// [`Component::table_dest`] assigns them). The class nibble is written
    /// here, ensuring it cannot be omitted or misstated.
    pub(crate) fn write_dht_ac_entry(self, sink: ByteSink<'_>) {
        let ByteSink(output) = sink;
        output.push(match self.huffman_class() {
            Component::Luma => 0x10,
            Component::Chroma => 0x11,
        });
    }

    /// Writes this component's entry in a SOS (Start of Scan) marker:
    /// the component ID and the byte combining DC and AC Huffman table
    /// indices (DC in the high nibble, AC in the low nibble). Both
    /// indices are [`Component::table_dest`]'s, so they always agree.
    pub(crate) fn write_sos_component(self, sink: ByteSink<'_>) {
        let ByteSink(output) = sink;
        output.push(self.id().0);
        output.push(match self.huffman_class() {
            Component::Luma => 0x00,
            Component::Chroma => 0x11,
        });
    }

    /// The `huffman::Component`/table-content selector this JPEG
    /// component uses: `Luma` for `Y`, `Chroma` for `Cb`/`Cr`.
    #[must_use]
    pub const fn huffman_class(self) -> Component {
        match self {
            Self::Luma => Component::Luma,
            Self::Cb | Self::Cr => Component::Chroma,
        }
    }
}

/// Encodes a frame as a JPEG file.
///
/// # Arguments
///
/// * `frame` - The input RGB frame
/// * `quality` - Quality parameter (1-100)
/// * `chroma_sampling` - Chroma sampling mode
#[must_use]
pub fn encode_jpeg(
    frame: &Frame,
    quality: Quality,
    chroma_sampling: ChromaSampling,
) -> EncodedFrame {
    let mut output = Vec::new();

    // Write SOI marker
    output.extend_from_slice(&[0xFF, 0xD8]);

    // Write APP0 (JFIF) marker
    write_app0(ByteSink(&mut output));

    // Write DQT (Define Quantization Table) markers
    write_dqt(ByteSink(&mut output), quality);

    // Write SOF0 (Start of Frame) marker
    write_sof0(ByteSink(&mut output), frame, chroma_sampling);

    // Write DHT (Define Huffman Table) markers
    write_dht(ByteSink(&mut output));

    // Write SOS (Start of Scan) marker
    write_sos(ByteSink(&mut output));

    // Encode frame data
    encode_scan(frame, quality, chroma_sampling, ByteSink(&mut output));

    // Write EOI marker
    output.extend_from_slice(&[0xFF, 0xD9]);

    EncodedFrame(output)
}

/// Both quantization tables a scan needs, built once per [`encode_scan`]
/// call rather than once per block.
///
/// `quant::scale_table` builds a fresh 64-entry table from scratch, and
/// quality is constant across an entire image, so calling it once per
/// block -- of which a single frame can have tens of thousands -- rebuilt
/// the same two tables that many times over. This holds the two tables
/// [`encode_scan`] actually needs (one per [`Component`] variant) so every
/// block reuses them instead.
struct QuantTablesByComponent {
    /// The luma quantization table.
    luma: QuantTable,
    /// The chroma quantization table.
    chroma: QuantTable,
}

impl QuantTablesByComponent {
    /// Builds both tables for `quality`.
    fn new(quality: Quality) -> Self {
        Self {
            luma: quant::scale_table(quality, Component::Luma),
            chroma: quant::scale_table(quality, Component::Chroma),
        }
    }

    /// Returns the table for `component`.
    const fn for_component(&self, component: Component) -> &QuantTable {
        match component {
            Component::Luma => &self.luma,
            Component::Chroma => &self.chroma,
        }
    }
}

/// The state every block of one scan advances: a DC predictor per
/// component, the two quantization tables, and the entropy-coded output.
struct ScanState {
    /// One DC predictor per component.
    predictors: DcPredictors,
    /// The quantization tables, one per table class.
    quant_tables: QuantTablesByComponent,
    /// The entropy-coded scan data written so far.
    writer: BitstreamWriter,
}

impl ScanState {
    /// A scan with no blocks written yet, quantizing at `quality`.
    fn new(quality: Quality) -> Self {
        Self {
            predictors: DcPredictors::new(),
            quant_tables: QuantTablesByComponent::new(quality),
            writer: BitstreamWriter::new(),
        }
    }

    /// Extracts and encodes one block for `component`, at block-grid
    /// coordinates `coord`: DCT, delta-DC encoding, and AC coefficient
    /// run-length encoding.
    ///
    /// Takes `component` once and derives the source plane
    /// ([`ActivePlanes::plane`]), the DC predictor ([`DcPredictors::slot`]),
    /// and the Huffman/quantization table class from it, rather than a caller
    /// picking a pixel block and a `ComponentSlot` as two separate arguments
    /// that could name different components with no compiler error -- which a
    /// previous version of this function, split into two, allowed (a
    /// `ComponentSlot` for one component passed alongside a block extracted
    /// from a different component's plane).
    fn encode_plane_block(
        &mut self,
        planes: &ActivePlanes<'_>,
        component: JpegComponent,
        coord: BlockCoord,
    ) {
        let block = planes.plane(component).extract_block(coord);
        let mut slot = self.predictors.slot(component);
        let table_class = component.huffman_class();

        let dct_coeffs = encode_block(&block, self.quant_tables.for_component(table_class));
        slot.encode_dc(dct_coeffs.dc(), &mut self.writer);

        // Reorder coefficients into zigzag scan order
        let zigzagged = dct_coeffs.into_zigzag();

        // Encode AC coefficients
        encode_ac_coefficients(zigzagged.ac_coefficients(), &mut self.writer, table_class);
    }
}

/// The pixel width or height of one MCU in a given dimension: `8` for no
/// subsampling, `16` for 2x. A pure lookup over
/// [`SamplingFactor`]'s two variants, not `8 * factor.get()`:
/// there is no multiplication here to overflow, since the two possible
/// results are just named directly.
const fn mcu_pixel_size(factor: SamplingFactor) -> McuExtent {
    match factor {
        SamplingFactor::One => McuExtent(8),
        SamplingFactor::Two => McuExtent(16),
    }
}

/// An MCU's index along one axis of the MCU grid.
#[derive(Debug, Clone, Copy)]
struct McuIndex(u32);

impl McuIndex {
    /// The block-grid indices, along this axis, of the luma blocks this MCU
    /// covers: `index * factor + offset` for each `offset` in `0..factor`.
    /// The `One` arm needs no arithmetic at all; the `Two` arm's
    /// `index * 2 + 1` is bounded by this crate's own `Width`/`Height`
    /// types (at most `u16::MAX`, so at most `131_071`) -- far below
    /// `u32::MAX`, so neither checked step ever actually comes back empty.
    fn luma_blocks(self, factor: SamplingFactor) -> impl Iterator<Item = LumaBlockIndex> {
        let doubled = self.0.checked_mul(2);
        let (first, second) = match factor {
            SamplingFactor::One => (Some(self.0), None),
            SamplingFactor::Two => (doubled, doubled.and_then(|even| even.checked_add(1))),
        };
        first.into_iter().chain(second).map(LumaBlockIndex)
    }
}

/// A luma block's index along one axis of the block grid.
#[derive(Debug, Clone, Copy)]
struct LumaBlockIndex(u32);

/// Encodes the scan data.
fn encode_scan(
    frame: &Frame,
    quality: Quality,
    chroma_sampling: ChromaSampling,
    sink: ByteSink<'_>,
) {
    let ByteSink(output) = sink;

    // Convert frame to YCbCr
    let planes = YCbCrPlanes::from_frame(frame);

    let mut state = ScanState::new(quality);

    match chroma_sampling {
        ChromaSampling::Yuv444 => {
            encode_full_resolution_blocks(&mut state, &planes.full_resolution(), frame);
        }
        ChromaSampling::Yuv420 => {
            // Downsample chroma planes to half resolution
            let downsampled_chroma = planes.downsample_chroma();
            encode_subsampled_mcus(
                &mut state,
                &downsampled_chroma.active(),
                frame,
                chroma_sampling.subsample_factor(),
            );
        }
    }

    let scan_data = state.writer.into_bytes();
    output.extend_from_slice(&scan_data.0);
}

/// 4:4:4 encoding: one luma block, one Cb and one Cr block per block
/// position `(bx, by)`.
fn encode_full_resolution_blocks(state: &mut ScanState, active: &ActivePlanes<'_>, frame: &Frame) {
    let block_width = NonZeroU32::from(frame.width().get()).get().div_ceil(8);
    let block_height = NonZeroU32::from(frame.height().get()).get().div_ceil(8);

    for by in 0..block_height {
        for bx in 0..block_width {
            let coord = BlockCoord::new(BlockCol::new(bx), BlockRow::new(by));
            for component in JpegComponent::ALL {
                state.encode_plane_block(active, component, coord);
            }
        }
    }
}

/// 4:2:0 encoding: the MCU grid covers 16x16 luma pixels, with chroma
/// read from the downsampled planes.
fn encode_subsampled_mcus(
    state: &mut ScanState,
    active: &ActivePlanes<'_>,
    frame: &Frame,
    factor: SubsampleFactor,
) {
    let mcu_pixel_width = mcu_pixel_size(factor.horizontal());
    let mcu_pixel_height = mcu_pixel_size(factor.vertical());

    let mcu_width = NonZeroU32::from(frame.width().get())
        .get()
        .div_ceil(mcu_pixel_width.0);
    let mcu_height = NonZeroU32::from(frame.height().get())
        .get()
        .div_ceil(mcu_pixel_height.0);

    for mby in (0..mcu_height).map(McuIndex) {
        for mbx in (0..mcu_width).map(McuIndex) {
            encode_mcu(state, active, factor, McuCoord { mbx, mby });
        }
    }
}

/// An MCU's position in the MCU grid.
#[derive(Debug, Clone, Copy)]
struct McuCoord {
    /// The MCU's column.
    mbx: McuIndex,
    /// The MCU's row.
    mby: McuIndex,
}

/// Encodes one 4:2:0 MCU: its luma blocks, then one Cb and one Cr block.
fn encode_mcu(
    state: &mut ScanState,
    active: &ActivePlanes<'_>,
    factor: SubsampleFactor,
    McuCoord { mbx, mby }: McuCoord,
) {
    // Four luma blocks in MCU order: top-left, top-right,
    // bottom-left, bottom-right
    for LumaBlockIndex(by) in mby.luma_blocks(factor.vertical()) {
        for LumaBlockIndex(bx) in mbx.luma_blocks(factor.horizontal()) {
            let coord = BlockCoord::new(BlockCol::new(bx), BlockRow::new(by));
            state.encode_plane_block(active, JpegComponent::Luma, coord);
        }
    }

    // One Cb block and one Cr block per MCU from downsampled planes
    let mcu_coord = BlockCoord::new(BlockCol::new(mbx.0), BlockRow::new(mby.0));
    for component in JpegComponent::CHROMA {
        state.encode_plane_block(active, component, mcu_coord);
    }
}

#[cfg(test)]
mod tests;
