//! JPEG file assembly and encoding.

use crate::ac_encoding::encode_ac_coefficients;
use crate::bitstream::BitstreamWriter;
use crate::config::SubsampleFactor;
pub use crate::config::{ChromaSampling, Quality};
use crate::dct::encode_block;
use crate::huffman::Component;
use synthvid_scene::Frame;

mod plane;
use plane::{BlockCol, BlockCoord, BlockRow};

mod planes;
use planes::{ActivePlanes, YCbCrPlanes};

mod predictor;
use predictor::DcPredictors;

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
    Y,
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
    pub(crate) const ALL: [Self; 3] = [Self::Y, Self::CHROMA[0], Self::CHROMA[1]];

    /// This component's marker ID byte (1 = Y, 2 = Cb, 3 = Cr), as SOF0
    /// and SOS both require.
    #[must_use]
    const fn id(self) -> u8 {
        match self {
            Self::Y => 1,
            Self::Cb => 2,
            Self::Cr => 3,
        }
    }

    /// Gets this component's quantization table index. Derived from
    /// `huffman_class`, which is the single source of truth for every
    /// table destination this component uses.
    #[must_use]
    const fn quant_table(self) -> u8 {
        self.huffman_class().table_dest()
    }

    /// Gets this component's DC Huffman table index. Derived from
    /// `huffman_class`; see [`Self::quant_table`].
    #[must_use]
    const fn dc_table(self) -> u8 {
        self.huffman_class().table_dest()
    }

    /// Gets this component's AC Huffman table index. Derived from
    /// `huffman_class`; see [`Self::quant_table`].
    #[must_use]
    const fn ac_table(self) -> u8 {
        self.huffman_class().table_dest()
    }

    /// Writes this component's entry in a DQT (Define Quantization Table)
    /// marker: the byte combining the table-class nibble (0) with the
    /// quantization-table destination index.
    pub(crate) fn write_dqt_entry(self, out: &mut Vec<u8>) {
        out.push(self.quant_table());
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
            Self::Y => chroma_sampling.subsample_factor(),
            Self::Cb | Self::Cr => SubsampleFactor::ONE_TO_ONE,
        }
    }

    /// Writes this component's entry in a SOF0 (Start of Frame) marker:
    /// the component ID, sampling factors (high nibble = H, low nibble = V),
    /// and quantization table destination index.
    pub(crate) fn write_sof_component(self, out: &mut Vec<u8>, chroma_sampling: ChromaSampling) {
        let factor = self.sampling(chroma_sampling);
        out.push(self.id());
        out.push((factor.horizontal().get() << 4) | factor.vertical().get());
        out.push(self.quant_table());
    }

    /// Writes this component's DC Huffman table class-and-destination byte
    /// for a DHT (Define Huffman Table) marker: the class nibble (0) with
    /// the DC table destination index.
    pub(crate) fn write_dht_dc_entry(self, out: &mut Vec<u8>) {
        out.push(self.dc_table());
    }

    /// Writes this component's AC Huffman table class-and-destination byte
    /// for a DHT (Define Huffman Table) marker: the class nibble (1) with
    /// the AC table destination index. The `1 << 4` class nibble is
    /// constructed here, ensuring it cannot be omitted or misstated.
    pub(crate) fn write_dht_ac_entry(self, out: &mut Vec<u8>) {
        out.push((1_u8 << 4) | self.ac_table());
    }

    /// Writes this component's entry in a SOS (Start of Scan) marker:
    /// the component ID and the byte combining DC and AC Huffman table
    /// indices (DC in the high nibble, AC in the low nibble).
    pub(crate) fn write_sos_component(self, out: &mut Vec<u8>) {
        out.push(self.id());
        out.push((self.dc_table() << 4) | self.ac_table());
    }

    /// The `huffman::Component`/table-content selector this JPEG
    /// component uses: `Luma` for `Y`, `Chroma` for `Cb`/`Cr`.
    #[must_use]
    pub const fn huffman_class(self) -> Component {
        match self {
            Self::Y => Component::Luma,
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
pub fn encode_jpeg(frame: &Frame, quality: Quality, chroma_sampling: ChromaSampling) -> Vec<u8> {
    let mut output = Vec::new();

    // Write SOI marker
    output.extend_from_slice(&[0xFF, 0xD8]);

    // Write APP0 (JFIF) marker
    crate::jpeg_markers::write_app0(&mut output);

    // Write DQT (Define Quantization Table) markers
    crate::jpeg_markers::write_dqt(&mut output, quality);

    // Write SOF0 (Start of Frame) marker
    crate::jpeg_markers::write_sof0(&mut output, frame, chroma_sampling);

    // Write DHT (Define Huffman Table) markers
    crate::jpeg_markers::write_dht(&mut output);

    // Write SOS (Start of Scan) marker
    crate::jpeg_markers::write_sos(&mut output);

    // Encode frame data
    encode_scan(frame, quality, chroma_sampling, &mut output);

    // Write EOI marker
    output.extend_from_slice(&[0xFF, 0xD9]);

    output
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
    planes: &ActivePlanes<'_>,
    predictors: &mut DcPredictors,
    component: JpegComponent,
    coord: BlockCoord,
    quality: Quality,
    writer: &mut BitstreamWriter,
) {
    let block = planes.plane(component).extract_block(coord);
    let mut slot = predictors.slot(component);
    let table_class = component.huffman_class();

    let dct_coeffs = encode_block(&block, quality, table_class);
    slot.encode_dc(dct_coeffs.dc(), writer);

    // Reorder coefficients into zigzag scan order
    let zigzagged = dct_coeffs.into_zigzag();

    // Encode AC coefficients
    encode_ac_coefficients(zigzagged.ac_coefficients(), writer, table_class);
}

/// The pixel width or height of one MCU in a given dimension: `8` for no
/// subsampling, `16` for 2x. A pure lookup over
/// [`crate::config::SamplingFactor`]'s two variants, not `8 * factor.get()`:
/// there is no multiplication here to overflow, since the two possible
/// results are just named directly.
const fn mcu_pixel_size(factor: crate::config::SamplingFactor) -> u32 {
    match factor {
        crate::config::SamplingFactor::One => 8,
        crate::config::SamplingFactor::Two => 16,
    }
}

/// Computes a block-grid coordinate from its MCU index, subsampling
/// factor, and in-MCU offset: `mcu_index * factor.get() + offset`. Total:
/// `factor` is always `One` or `Two` (see
/// [`crate::config::SamplingFactor`]), and `offset` is always `0` when
/// `factor` is `One` (the encoding loop above only iterates
/// `0..factor.get()`), so the `One` arm needs no arithmetic operator at
/// all, and the `Two` arm's `mcu_index * 2` is bounded by this crate's own
/// `Width`/`Height` types (at most `u16::MAX`, so at most `131_070` when
/// doubled) -- both far below `u32::MAX`.
const fn scaled_block_index(
    mcu_index: u32,
    factor: crate::config::SamplingFactor,
    offset: u32,
) -> u32 {
    match factor {
        crate::config::SamplingFactor::One => mcu_index,
        crate::config::SamplingFactor::Two => {
            let doubled = match mcu_index.checked_add(mcu_index) {
                Some(d) => d,
                None if mcu_index == 0 => 0,
                None => u32::MAX,
            };
            match doubled.checked_add(offset) {
                Some(v) => v,
                None if offset == 0 => doubled,
                None => u32::MAX,
            }
        }
    }
}

/// Encodes the scan data.
fn encode_scan(
    frame: &Frame,
    quality: Quality,
    chroma_sampling: ChromaSampling,
    output: &mut Vec<u8>,
) {
    let width = u32::from(frame.width().get().get());
    let height = u32::from(frame.height().get().get());

    // Convert frame to YCbCr
    let planes = YCbCrPlanes::from_frame(frame);

    let mut writer = BitstreamWriter::new();

    let mut predictors = DcPredictors::new();

    match chroma_sampling {
        ChromaSampling::Yuv444 => {
            let active = planes.full_resolution();

            // 4:4:4 encoding: one luma block + one Cb + one Cr block per (bx, by)
            let block_width = width.div_ceil(8);
            let block_height = height.div_ceil(8);

            for by in 0..block_height {
                for bx in 0..block_width {
                    let coord = BlockCoord::new(BlockCol::new(bx), BlockRow::new(by));
                    for component in JpegComponent::ALL {
                        encode_plane_block(
                            &active,
                            &mut predictors,
                            component,
                            coord,
                            quality,
                            &mut writer,
                        );
                    }
                }
            }
        }
        ChromaSampling::Yuv420 => {
            // 4:2:0 encoding: MCU grid covers 16x16 luma pixels
            // Downsample chroma planes to half resolution
            let downsampled_chroma = planes.downsample_chroma();
            let active = downsampled_chroma.active();

            let factor = chroma_sampling.subsample_factor();
            let mcu_pixel_width = mcu_pixel_size(factor.horizontal());
            let mcu_pixel_height = mcu_pixel_size(factor.vertical());

            let mcu_width = width.div_ceil(mcu_pixel_width);
            let mcu_height = height.div_ceil(mcu_pixel_height);

            for mby in 0..mcu_height {
                for mbx in 0..mcu_width {
                    // Four luma blocks in MCU order: top-left, top-right,
                    // bottom-left, bottom-right
                    for dy in 0_u32..u32::from(factor.vertical().get()) {
                        for dx in 0_u32..u32::from(factor.horizontal().get()) {
                            let bx = scaled_block_index(mbx, factor.horizontal(), dx);
                            let by = scaled_block_index(mby, factor.vertical(), dy);
                            encode_plane_block(
                                &active,
                                &mut predictors,
                                JpegComponent::Y,
                                BlockCoord::new(BlockCol::new(bx), BlockRow::new(by)),
                                quality,
                                &mut writer,
                            );
                        }
                    }

                    // One Cb block and one Cr block per MCU from downsampled planes
                    let mcu_coord = BlockCoord::new(BlockCol::new(mbx), BlockRow::new(mby));
                    for component in JpegComponent::CHROMA {
                        encode_plane_block(
                            &active,
                            &mut predictors,
                            component,
                            mcu_coord,
                            quality,
                            &mut writer,
                        );
                    }
                }
            }
        }
    }

    let scan_data = writer.into_vec();
    output.extend_from_slice(&scan_data);
}

#[cfg(test)]
mod tests {
    use super::*;
    use synthvid_scene::{Dimensions, Height, Width};

    #[test]
    fn test_golden_bytes_output() {
        let width = Width::new(16).expect("valid width");
        let height = Height::new(16).expect("valid height");
        let dims = Dimensions::new(width, height);
        let mut data = vec![128u8; 16 * 16 * 3];
        let (chunks, _remainder) = data.as_chunks_mut::<3>();
        for (i, chunk) in chunks.iter_mut().enumerate() {
            let column = u8::try_from(i.rem_euclid(16)).expect("rem_euclid(16) fits in u8");
            let row = u8::try_from(i.div_euclid(16)).expect("i < 256, so div_euclid(16) < 16");
            let [red, green, blue] = chunk;
            *red = column.saturating_mul(16); // R varies by column
            *green = 128; // G constant
            *blue = row.saturating_mul(16); // B varies by row
        }
        let frame = Frame::new(dims, data).expect("valid frame");
        let golden_yuv444 = encode_jpeg(&frame, Quality::new(75).unwrap(), ChromaSampling::Yuv444);
        let golden_yuv420 = encode_jpeg(&frame, Quality::new(50).unwrap(), ChromaSampling::Yuv420);
        assert!(
            !golden_yuv444.is_empty(),
            "Yuv444 output should not be empty"
        );
        assert!(
            !golden_yuv420.is_empty(),
            "Yuv420 output should not be empty"
        );
        let encoded_yuv444_again =
            encode_jpeg(&frame, Quality::new(75).unwrap(), ChromaSampling::Yuv444);
        let encoded_yuv420_again =
            encode_jpeg(&frame, Quality::new(50).unwrap(), ChromaSampling::Yuv420);
        assert_eq!(
            golden_yuv444, encoded_yuv444_again,
            "Yuv444 encoding should be deterministic"
        );
        assert_eq!(
            golden_yuv420, encoded_yuv420_again,
            "Yuv420 encoding should be deterministic"
        );
    }
}
