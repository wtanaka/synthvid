//! JPEG marker writing functions.

use crate::byte_views::ByteSink;
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

/// Total APP0 segment length. The length field includes the 2-byte length
/// field itself, but excludes the 2-byte marker code:
///
/// | Field                               | Bytes |
/// | ----------------------------------- | ----- |
/// | the length field itself             | 2     |
/// | identifier (`"JFIF\0"`)             | 5     |
/// | version                             | 2     |
/// | aspect ratio units                  | 1     |
/// | X density                           | 2     |
/// | Y density                           | 2     |
/// | thumbnail dimensions                | 2     |
///
/// `2 + 5 + 2 + 1 + 2 + 2 + 2 = 16`.
const APP0_SEGMENT_LEN: u16 = 16;

/// Writes the APP0 JFIF marker.
pub(super) fn write_app0(sink: ByteSink<'_>) {
    let ByteSink(output) = sink;
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

/// Total DQT segment length, including the 2-byte length field itself:
/// the length field (2), the table class and destination (1), and one
/// 64-entry quantization table (64). `2 + 1 + 64 = 67`.
const DQT_SEGMENT_LEN: u16 = 67;

/// Writes the DQT (Define Quantization Table) markers.
pub(super) fn write_dqt(sink: ByteSink<'_>, quality: Quality) {
    let ByteSink(output) = sink;
    // Luminance table
    output.extend_from_slice(&[0xFF, 0xDB]);
    output.extend_from_slice(&DQT_SEGMENT_LEN.to_be_bytes());
    JpegComponent::Luma.write_dqt_entry(ByteSink(&mut *output)); // Table class and destination (8-bit, table 0)

    let luma_table = quant::scale_table(quality, JpegComponent::Luma.huffman_class());
    output.extend_from_slice(luma_table.as_zigzag_bytes().as_marker_bytes());

    // Chrominance table
    output.extend_from_slice(&[0xFF, 0xDB]);
    output.extend_from_slice(&DQT_SEGMENT_LEN.to_be_bytes());
    JpegComponent::Cb.write_dqt_entry(ByteSink(&mut *output)); // Table class and destination (8-bit, table 1)

    let chroma_table = quant::scale_table(quality, JpegComponent::Cb.huffman_class());
    output.extend_from_slice(chroma_table.as_zigzag_bytes().as_marker_bytes());
}

/// Total SOF0 segment length, including the 2-byte length field itself:
///
/// | Field                                         | Bytes     |
/// | --------------------------------------------- | --------- |
/// | the length field itself                       | 2         |
/// | precision                                     | 1         |
/// | image height                                  | 2         |
/// | image width                                   | 2         |
/// | number of components                          | 1         |
/// | component info (3 components, 3 bytes each)   | 3 * 3 = 9 |
///
/// `2 + 1 + 2 + 2 + 1 + 9 = 17`.
const SOF0_SEGMENT_LEN: u16 = 17;

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
fn write_sof0_dimensions(sink: ByteSink<'_>, height: Height, width: Width) {
    let ByteSink(output) = sink;
    let [h_hi, h_lo] = height.get().get().to_be_bytes();
    let [w_hi, w_lo] = width.get().get().to_be_bytes();
    output.extend_from_slice(&[h_hi, h_lo, w_hi, w_lo]);
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
pub(super) fn write_sof0(sink: ByteSink<'_>, frame: &Frame, chroma_sampling: ChromaSampling) {
    let ByteSink(output) = sink;
    output.extend_from_slice(&[0xFF, 0xC0]);
    // Length: 2 (length) + 1 (precision) + 2 (height) + 2 (width) + 1 (components) + 3*3 (component info)
    output.extend_from_slice(&SOF0_SEGMENT_LEN.to_be_bytes());
    output.push(0x08); // 8-bit precision

    // Height and width (big-endian), in that wire order.
    write_sof0_dimensions(ByteSink(&mut *output), frame.height(), frame.width());

    // Number of components
    output.push(0x03); // Y, Cb, Cr -- see JpegComponent::ALL

    for component in JpegComponent::ALL {
        component.write_sof_component(ByteSink(&mut *output), chroma_sampling);
    }
}

/// Total length of the DC DHT segment: the 2-byte length field, then for
/// each of the two DC tables (luma, then chroma) one class-and-destination
/// byte, one 16-entry bits array, and its 12-entry values array.
/// `2 + (1 + 16 + 12) + (1 + 16 + 12) = 60`. The tests below pin both
/// values tables at 12 entries.
const DC_DHT_SEGMENT_LEN: u16 = 60;

/// Total length of the AC DHT segment, laid out as [`DC_DHT_SEGMENT_LEN`]
/// is but with the two 162-entry AC values tables.
/// `2 + (1 + 16 + 162) + (1 + 16 + 162) = 360`. The tests below pin both
/// values tables at 162 entries.
const AC_DHT_SEGMENT_LEN: u16 = 360;

/// Writes the DHT (Define Huffman Table) markers.
pub(super) fn write_dht(sink: ByteSink<'_>) {
    let ByteSink(output) = sink;
    // DC tables (both luma and chroma in one DHT segment)
    let dc_bits_luma = get_dc_luma_bits();
    let dc_values_luma = get_dc_luma_values();
    let dc_bits_chroma = get_dc_chroma_bits();
    let dc_values_chroma = get_dc_chroma_values();

    output.extend_from_slice(&[0xFF, 0xC4]); // DHT marker
    output.extend_from_slice(&DC_DHT_SEGMENT_LEN.to_be_bytes());

    // DC luma table (class 0, destination 0)
    JpegComponent::Luma.write_dht_dc_entry(ByteSink(&mut *output));
    output.extend_from_slice(&dc_bits_luma.0);
    output.extend_from_slice(dc_values_luma.0);

    // DC chroma table (class 0, destination 1)
    JpegComponent::Cb.write_dht_dc_entry(ByteSink(&mut *output));
    output.extend_from_slice(&dc_bits_chroma.0);
    output.extend_from_slice(dc_values_chroma.0);

    // AC tables
    let ac_bits_luma = get_ac_luma_bits();
    let ac_values_luma = get_ac_luma_values();
    let ac_bits_chroma = get_ac_chroma_bits();
    let ac_values_chroma = get_ac_chroma_values();

    output.extend_from_slice(&[0xFF, 0xC4]); // DHT marker
    output.extend_from_slice(&AC_DHT_SEGMENT_LEN.to_be_bytes());

    // AC luma table (class 1, destination 0)
    JpegComponent::Luma.write_dht_ac_entry(ByteSink(&mut *output));
    output.extend_from_slice(&ac_bits_luma.0);
    output.extend_from_slice(ac_values_luma.0);

    // AC chroma table (class 1, destination 1)
    JpegComponent::Cb.write_dht_ac_entry(ByteSink(&mut *output));
    output.extend_from_slice(&ac_bits_chroma.0);
    output.extend_from_slice(ac_values_chroma.0);
}

/// Total SOS segment length, including the 2-byte length field itself:
///
/// | Field                                         | Bytes     |
/// | --------------------------------------------- | --------- |
/// | the length field itself                       | 2         |
/// | number of components                          | 1         |
/// | component info (3 components, 2 bytes each)   | 3 * 2 = 6 |
/// | spectral selection                            | 3         |
///
/// `2 + 1 + 6 + 3 = 12`.
const SOS_SEGMENT_LEN: u16 = 12;

/// Writes the SOS (Start of Scan) marker.
pub(super) fn write_sos(sink: ByteSink<'_>) {
    let ByteSink(output) = sink;
    output.extend_from_slice(&[0xFF, 0xDA]);
    output.extend_from_slice(&SOS_SEGMENT_LEN.to_be_bytes());
    output.push(0x03); // Number of components -- see JpegComponent::ALL

    for component in JpegComponent::ALL {
        component.write_sos_component(ByteSink(&mut *output));
    }

    output.extend_from_slice(&[0x00, 0x3F, 0x00]); // Start/end of spectral selection
}

#[cfg(test)]
mod tests;
