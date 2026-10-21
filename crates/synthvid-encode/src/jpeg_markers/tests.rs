use super::*;

/// `DC_DHT_SEGMENT_LEN` and `AC_DHT_SEGMENT_LEN` are derived from these
/// table lengths.
#[test]
fn dht_segment_lengths_match_the_values_tables() {
    assert_eq!(
        [
            get_dc_luma_values().0.len(),
            get_dc_chroma_values().0.len(),
            get_ac_luma_values().0.len(),
            get_ac_chroma_values().0.len(),
        ],
        [12, 12, 162, 162]
    );
}
