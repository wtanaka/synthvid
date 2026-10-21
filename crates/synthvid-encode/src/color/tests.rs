use super::*;
use synthvid_scene::Channel;

#[test]
fn rgb_to_ycbcr_converts_white() {
    let ycbcr = rgb_to_ycbcr(Rgb8 {
        red: Channel(255),
        green: Channel(255),
        blue: Channel(255),
    });
    // White (255, 255, 255) should give exactly luma 255 with proper rounding
    // and neutral chroma (128, 128) since white has no color bias
    assert_eq!(ycbcr.y().get(), 255);
    assert_eq!(ycbcr.cb().get(), 128);
    assert_eq!(ycbcr.cr().get(), 128);
}

#[test]
fn rgb_to_ycbcr_converts_black() {
    let ycbcr = rgb_to_ycbcr(Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    });
    assert_eq!(ycbcr.y().get(), 0);
    assert_eq!(ycbcr.cb().get(), 128);
    assert_eq!(ycbcr.cr().get(), 128);
}

#[test]
fn rgb_to_ycbcr_small_red_value() {
    // Test proper rounding of chroma with small values
    // rgb_to_ycbcr(Rgb8 { r: Channel(1), g: Channel(0), b: Channel(0) }): blue_sum = -43*1 + -85*0 + 128*0 = -43
    // (-43 + 128) >> 8 = 85 >> 8 = 0, so blue_raw = 0, cb = 0 + 128 = 128
    let ycbcr = rgb_to_ycbcr(Rgb8 {
        red: Channel(1),
        green: Channel(0),
        blue: Channel(0),
    });
    assert_eq!(ycbcr.cb().get(), 128);
    // Similarly for red: red_sum = 128*1 + -107*0 + -21*0 = 128
    // (128 + 128) >> 8 = 256 >> 8 = 1, so red_raw = 1, cr = 1 + 128 = 129
    assert_eq!(ycbcr.cr().get(), 129);
}
