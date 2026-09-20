//! JPEG marker writing functions.

use crate::config::{ChromaSampling, Quality};
use crate::huffman::{
    get_ac_chroma_bits, get_ac_chroma_values, get_ac_luma_bits, get_ac_luma_values,
    get_dc_chroma_bits, get_dc_chroma_values, get_dc_luma_bits, get_dc_luma_values,
};
use crate::jpeg::JpegComponent;
use crate::quant;
use synthvid_scene::{Frame, Height, Width};

/// The `0x03`/`3 *` component-count literals below (the SOF0/SOS "number of
/// components" bytes and the `*_COMPONENT_INFO_LEN` segment-length
/// constants) all assume exactly the 3 components `JpegComponent::ALL`
/// lists. This assertion ties them together at compile time: if `ALL` ever
/// grew or shrank, this fails to compile instead of silently leaving those
/// literals out of sync with the per-component loops that actually use
/// `ALL`.
const _: [(); 3] = [(); JpegComponent::ALL.len()];

/// APP0 segment length calculations.
/// The length field includes the 2-byte length field itself, but excludes the 2-byte marker code.
/// Length of the APP0 identifier field ("JFIF\0").
const APP0_IDENTIFIER_LEN: u16 = 5;
/// Length of the APP0 version field.
const APP0_VERSION_LEN: u16 = 2;
/// Length of the APP0 aspect ratio units field.
const APP0_UNITS_LEN: u16 = 1;
/// Length of the APP0 X density field.
const APP0_DENSITY_X_LEN: u16 = 2;
/// Length of the APP0 Y density field.
const APP0_DENSITY_Y_LEN: u16 = 2;
/// Length of the APP0 thumbnail dimensions field.
const APP0_THUMBNAIL_DIMS_LEN: u16 = 2;
/// Total APP0 segment length including the 2-byte length field itself.
const APP0_SEGMENT_LEN: u16 = 2 // the length field itself
    + APP0_IDENTIFIER_LEN
    + APP0_VERSION_LEN
    + APP0_UNITS_LEN
    + APP0_DENSITY_X_LEN
    + APP0_DENSITY_Y_LEN
    + APP0_THUMBNAIL_DIMS_LEN;

/// Writes the APP0 JFIF marker.
pub(super) fn write_app0(output: &mut Vec<u8>) {
    output.extend_from_slice(&[0xFF, 0xE0]);
    // Length
    output.extend_from_slice(&APP0_SEGMENT_LEN.to_be_bytes());
    // Identifier "JFIF\0"
    output.extend_from_slice(b"JFIF\0");
    // Version 1.1
    output.extend_from_slice(&[0x01, 0x01]);
    // Aspect ratio units (0 = no units)
    output.push(0x00);
    // X density
    output.extend_from_slice(&[0x00, 0x01]);
    // Y density
    output.extend_from_slice(&[0x00, 0x01]);
    // Thumbnail dimensions
    output.extend_from_slice(&[0x00, 0x00]);
}

/// DQT segment length calculations (length field includes the 2-byte length field itself).
/// Length of the DQT class and destination field.
const DQT_CLASS_DEST_LEN: u16 = 1;
/// Length of a single DQT quantization table.
const DQT_QUANT_TABLE_LEN: u16 = 64;
/// Total DQT segment length including the 2-byte length field itself.
const DQT_SEGMENT_LEN: u16 = 2 // the length field itself
    + DQT_CLASS_DEST_LEN
    + DQT_QUANT_TABLE_LEN;

/// Writes the DQT (Define Quantization Table) markers.
pub(super) fn write_dqt(output: &mut Vec<u8>, quality: Quality) {
    // Luminance table
    output.extend_from_slice(&[0xFF, 0xDB]);
    output.extend_from_slice(&DQT_SEGMENT_LEN.to_be_bytes());
    JpegComponent::Y.write_dqt_entry(output); // Table class and destination (8-bit, table 0)

    let luma_table = quant::scale_table(quality, JpegComponent::Y.huffman_class());
    output.extend_from_slice(luma_table.as_zigzag_bytes().as_marker_bytes());

    // Chrominance table
    output.extend_from_slice(&[0xFF, 0xDB]);
    output.extend_from_slice(&DQT_SEGMENT_LEN.to_be_bytes());
    JpegComponent::Cb.write_dqt_entry(output); // Table class and destination (8-bit, table 1)

    let chroma_table = quant::scale_table(quality, JpegComponent::Cb.huffman_class());
    output.extend_from_slice(chroma_table.as_zigzag_bytes().as_marker_bytes());
}

/// SOF0 segment length calculations (length field includes the 2-byte length field itself).
/// Length of the SOF0 precision field.
const SOF0_PRECISION_LEN: u16 = 1;
/// Length of the SOF0 image height field.
const SOF0_HEIGHT_LEN: u16 = 2;
/// Length of the SOF0 image width field.
const SOF0_WIDTH_LEN: u16 = 2;
/// Length of the SOF0 number of components field.
const SOF0_COMPONENTS_LEN: u16 = 1;
/// Length of all SOF0 component info fields (3 components, 3 bytes each).
const SOF0_COMPONENT_INFO_LEN: u16 = 3 * 3;
/// Total SOF0 segment length including the 2-byte length field itself.
const SOF0_SEGMENT_LEN: u16 = 2 // the length field itself
    + SOF0_PRECISION_LEN
    + SOF0_HEIGHT_LEN
    + SOF0_WIDTH_LEN
    + SOF0_COMPONENTS_LEN
    + SOF0_COMPONENT_INFO_LEN;

/// The SOF0 marker's 4-byte height-then-width field, in that wire order.
///
/// Takes `Height` before `Width` -- not two bare `u16`s -- so that this
/// order is stated once, by the parameter types and their position, rather
/// than reconstructed at every call site from two separately-named byte
/// pairs a caller has to remember to interleave correctly. `Height` and
/// `Width` are themselves distinct types, so a caller cannot pass them in
/// the wrong argument position without a compiler error -- unlike calling
/// `.get().get()` on each first, which discards that distinction and
/// leaves nothing but variable names (`h`/`w`) to tell them apart when
/// building the output array.
const fn sof0_dimension_bytes(height: Height, width: Width) -> [u8; 4] {
    let [h_hi, h_lo] = height.get().get().to_be_bytes();
    let [w_hi, w_lo] = width.get().get().to_be_bytes();
    [h_hi, h_lo, w_hi, w_lo]
}

/// Writes the SOF0 (Start of Frame) marker.
///
/// Takes `&Frame`, not a separate `Width`/`Height` the caller derives ahead
/// of time: `encode_scan` (the only other place this frame's dimensions
/// matter) also derives them from `&Frame` itself, via `build_ycbcr_planes`.
/// Deriving the SOF0 header's dimensions from the same `frame` value here,
/// rather than from a `width`/`height` pair `encode_jpeg` would otherwise
/// compute once and pass to both, means the header and the scan cannot end
/// up with two different ideas of the frame's size.
pub(super) fn write_sof0(output: &mut Vec<u8>, frame: &Frame, chroma_sampling: ChromaSampling) {
    output.extend_from_slice(&[0xFF, 0xC0]);
    // Length: 2 (length) + 1 (precision) + 2 (height) + 2 (width) + 1 (components) + 3*3 (component info)
    output.extend_from_slice(&SOF0_SEGMENT_LEN.to_be_bytes());
    output.push(0x08); // 8-bit precision

    // Height and width (big-endian), in that wire order.
    output.extend_from_slice(&sof0_dimension_bytes(frame.height(), frame.width()));

    // Number of components
    output.push(0x03); // Y, Cb, Cr -- see JpegComponent::ALL

    for component in JpegComponent::ALL {
        component.write_sof_component(output, chroma_sampling);
    }
}

/// Narrows a Huffman values table's length to `u16`. Total in practice:
/// this crate's four value tables (`get_{dc,ac}_{luma,chroma}_values`) are
/// fixed-size arrays of at most 162 entries, far below `u16::MAX`.
fn narrow_table_len(len: usize) -> u16 {
    match u16::try_from(len) {
        Ok(n) => n,
        // Unreachable: see this function's doc comment.
        Err(_) if len == 0 => 0,
        Err(_) => u16::MAX,
    }
}

/// Computes a DHT segment's total length: the 2-byte length field, one
/// class-and-destination byte, one 16-entry bits array, and one values
/// array, twice over (once for luma, once for chroma). Total: the widest
/// case (`luma_len`/`chroma_len` both 162, the AC tables' size) is
/// `2 + 1 + 16 + 162 + 1 + 16 + 162 = 360`, far below `u16::MAX`.
fn dht_segment_len(table_lens: [u16; 2]) -> u16 {
    /// The class-and-destination byte plus the 16-entry bits array, summed
    /// once as a `const` item: entirely compile-time arithmetic between two
    /// fixed literals, not a runtime operation that could overflow.
    const PER_TABLE_OVERHEAD: u16 = DHT_CLASS_DEST_LEN + DHT_BITS_LEN;

    let mut total = 2_u16;
    for table_len in table_lens {
        let table_total = match PER_TABLE_OVERHEAD.checked_add(table_len) {
            Some(v) => v,
            None if table_len == 0 => PER_TABLE_OVERHEAD,
            None => u16::MAX,
        };
        total = match total.checked_add(table_total) {
            Some(v) => v,
            None if table_total == 0 => total,
            None => u16::MAX,
        };
    }
    total
}

/// Writes the DHT (Define Huffman Table) markers.
pub(super) fn write_dht(output: &mut Vec<u8>) {
    // DC tables (both luma and chroma in one DHT segment)
    let dc_bits_luma = get_dc_luma_bits();
    let dc_values_luma = get_dc_luma_values();
    let dc_bits_chroma = get_dc_chroma_bits();
    let dc_values_chroma = get_dc_chroma_values();

    // Calculate lengths for DC table segment
    let dc_luma_len = narrow_table_len(dc_values_luma.len());
    let dc_chroma_len = narrow_table_len(dc_values_chroma.len());
    let dc_segment_len: u16 = dht_segment_len([dc_luma_len, dc_chroma_len]);

    output.extend_from_slice(&[0xFF, 0xC4]); // DHT marker
    output.extend_from_slice(&dc_segment_len.to_be_bytes());

    // DC luma table (class 0, destination 0)
    JpegComponent::Y.write_dht_dc_entry(output);
    output.extend_from_slice(dc_bits_luma);
    output.extend_from_slice(dc_values_luma);

    // DC chroma table (class 0, destination 1)
    JpegComponent::Cb.write_dht_dc_entry(output);
    output.extend_from_slice(dc_bits_chroma);
    output.extend_from_slice(dc_values_chroma);

    // AC tables
    let ac_bits_luma = get_ac_luma_bits();
    let ac_values_luma = get_ac_luma_values();
    let ac_bits_chroma = get_ac_chroma_bits();
    let ac_values_chroma = get_ac_chroma_values();

    // Calculate lengths for AC table segment
    let ac_luma_len = narrow_table_len(ac_values_luma.len());
    let ac_chroma_len = narrow_table_len(ac_values_chroma.len());
    let ac_segment_len: u16 = dht_segment_len([ac_luma_len, ac_chroma_len]);

    output.extend_from_slice(&[0xFF, 0xC4]); // DHT marker
    output.extend_from_slice(&ac_segment_len.to_be_bytes());

    // AC luma table (class 1, destination 0)
    JpegComponent::Y.write_dht_ac_entry(output);
    output.extend_from_slice(ac_bits_luma);
    output.extend_from_slice(ac_values_luma);

    // AC chroma table (class 1, destination 1)
    JpegComponent::Cb.write_dht_ac_entry(output);
    output.extend_from_slice(ac_bits_chroma);
    output.extend_from_slice(ac_values_chroma);
}

/// SOS segment length calculations (length field includes the 2-byte length field itself).
/// Length of the SOS number of components field.
const SOS_COMPONENTS_COUNT_LEN: u16 = 1;
/// Length of all SOS component info fields (3 components, 2 bytes each).
const SOS_COMPONENT_INFO_LEN: u16 = 3 * 2;
/// Length of the SOS spectral selection fields.
const SOS_SPECTRAL_SELECTION_LEN: u16 = 3;
/// Total SOS segment length including the 2-byte length field itself.
const SOS_SEGMENT_LEN: u16 = 2 // the length field itself
    + SOS_COMPONENTS_COUNT_LEN
    + SOS_COMPONENT_INFO_LEN
    + SOS_SPECTRAL_SELECTION_LEN;

/// Writes the SOS (Start of Scan) marker.
pub(super) fn write_sos(output: &mut Vec<u8>) {
    output.extend_from_slice(&[0xFF, 0xDA]);
    output.extend_from_slice(&SOS_SEGMENT_LEN.to_be_bytes());
    output.push(0x03); // Number of components -- see JpegComponent::ALL

    for component in JpegComponent::ALL {
        component.write_sos_component(output);
    }

    output.extend_from_slice(&[0x00, 0x3F, 0x00]); // Start/end of spectral selection
}

/// DHT segment length calculations (length field includes the 2-byte length field itself).
/// Length of a DHT table's class-and-destination byte.
const DHT_CLASS_DEST_LEN: u16 = 1;
/// Length of a DHT table's 16-entry code-length-count array.
const DHT_BITS_LEN: u16 = 16;
