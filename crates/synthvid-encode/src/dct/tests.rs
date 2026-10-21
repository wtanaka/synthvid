use super::*;
use crate::config::Quality;
use crate::huffman::Component;
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

#[test]
fn encode_uniform_block() {
    assert_eq!(check_encode_uniform_block(), Ok(()));
}

fn check_encode_uniform_block() -> Result<(), Fail> {
    let pixel_block = PixelBlock::new([128_u8; 64]);
    let quality = Quality::new(75).or_fail()?;
    let quant_table = quant::scale_table(quality, Component::Luma);
    let result = encode_block(&pixel_block, &quant_table);
    // All zeros in DCT coefficients should remain zeros
    ensure_eq!(result.dc().0, 0);
    Ok(())
}

#[test]
fn forward_dct_edge_term_matches_hand_derivation() {
    // A block whose pixel value depends only on x (identical in every
    // row), so every v>0 output term is exactly zero (no y-variation
    // to excite them, by orthogonality of the cosine basis) and
    // F(1,0) is a pure "edge" term (u>0, v==0) -- exactly the term
    // `DCT_DIVISOR_EDGE` divides. F(1,0) is hand-derived below,
    // independently of this code, specifically to catch a regression
    // of the bug once found in this crate: `DCT_DIVISOR_EDGE` computed
    // as `4 * SQRT2_RECIPROCAL * COSINE_SCALE` (i.e. `4 * (1/sqrt(2))
    // * 2^32`) instead of the correct `4 * sqrt(2) * 2^32` -- exactly
    // half the correct value, silently doubling the magnitude of every
    // edge coefficient this encoder ever produced. Solid-color and
    // 4x4-checkerboard inputs never exposed this, since neither
    // excites an edge term (`u==0 xor v==0`) on its own.
    const ROW: [u8; 8] = [0, 16, 32, 48, 64, 80, 96, 112];
    let mut pixels = [0_u8; 64];
    for chunk in pixels.chunks_mut(8) {
        chunk.copy_from_slice(&ROW);
    }
    let block = PixelBlock::new(pixels);
    let output = forward_dct(&block.centered());

    // Centered column values (pixel - 128) for x=0..7:
    // -128,-112,-96,-80,-64,-48,-32,-16. COSINES[1] (already scaled by
    // 2^16): [64277, 54491, 36410, 12785, -12785, -36410, -54491,
    // -64277]. sum_x centered(x) * COSINES[1][x] = -13,510,544.
    // Summed over all 8 identical rows (COSINES[0] is all 65536, i.e.
    // cos(0) scaled): the raw accumulated sum is
    // 8 * 65536 * -13,510,544 = -7,083,416,092,672. Dividing by the
    // corrected DCT_DIVISOR_EDGE (4 * sqrt(2) * 2^32 =
    // 24,296,030,208) and truncating toward zero gives -291. (The
    // pre-fix, half-sized divisor would have given approximately
    // -583.)
    assert_eq!(output.0[1], -291_i32, "F(1,0), the u=1,v=0 edge term");
}

#[test]
fn scale_quantization() {
    assert_eq!(check_scale_quantization(), Ok(()));
}

fn check_scale_quantization() -> Result<(), Fail> {
    let low = quant::scale_table(Quality::new(50).or_fail()?, Component::Luma);
    let high = quant::scale_table(Quality::new(90).or_fail()?, Component::Luma);
    // Higher quality should have smaller quantization values
    ensure!(low.divisors()[0].get() > high.divisors()[0].get());
    Ok(())
}

#[test]
fn zigzag_matches_known_permutation() {
    const ZIGZAG: [usize; 64] = [
        0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27,
        20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51,
        58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
    ];
    let mut raster = [0_usize; 64];
    for (position, cell) in raster.iter_mut().enumerate() {
        *cell = position;
    }
    let expected = ZIGZAG;
    assert_eq!(reorder_to_zigzag(&raster), expected);
}
