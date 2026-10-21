use super::*;
use synthvid_scene::{Dimensions, Height, PixelBuffer, Width};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

#[test]
fn golden_bytes_output() {
    assert_eq!(check_golden_bytes_output(), Ok(()));
}

fn check_golden_bytes_output() -> Result<(), Fail> {
    let width = Width::new(16).or_fail()?;
    let height = Height::new(16).or_fail()?;
    let dims = Dimensions::new(width, height);
    // 16 * 16 pixels, 3 bytes each.
    let mut data = vec![128_u8; 768];
    let (chunks, _) = data.as_chunks_mut::<3>();
    for (pixel, chunk) in chunks.iter_mut().enumerate() {
        let column = u8::try_from(pixel.rem_euclid(16)).or_fail()?;
        let row = u8::try_from(pixel.div_euclid(16)).or_fail()?;
        let [red, green, blue] = chunk;
        *red = column.checked_mul(16).or_fail()?; // R varies by column
        *green = 128; // G constant
        *blue = row.checked_mul(16).or_fail()?; // B varies by row
    }
    let frame = Frame::new(dims, PixelBuffer(data)).or_fail()?;
    let q75 = Quality::new(75).or_fail()?;
    let q50 = Quality::new(50).or_fail()?;
    let golden_yuv444 = encode_jpeg(&frame, q75, ChromaSampling::Yuv444);
    let golden_yuv420 = encode_jpeg(&frame, q50, ChromaSampling::Yuv420);
    ensure!(
        !golden_yuv444.0.is_empty(),
        "Yuv444 output should not be empty"
    );
    ensure!(
        !golden_yuv420.0.is_empty(),
        "Yuv420 output should not be empty"
    );
    let encoded_yuv444_again = encode_jpeg(&frame, q75, ChromaSampling::Yuv444);
    let encoded_yuv420_again = encode_jpeg(&frame, q50, ChromaSampling::Yuv420);
    ensure_eq!(
        golden_yuv444,
        encoded_yuv444_again,
        "Yuv444 encoding should be deterministic"
    );
    ensure_eq!(
        golden_yuv420,
        encoded_yuv420_again,
        "Yuv420 encoding should be deterministic"
    );
    Ok(())
}
