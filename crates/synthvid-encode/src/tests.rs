use synthvid_scene::{Dimensions, Frame, Height, PixelBuffer, Width};
use synthvid_testkit::{ensure, Fail, OrFail as _};

/// Pins the cross-crate invariant `encode_jpeg`'s pixel-plane
/// construction relies on but does not itself check: `Frame::new`
/// only ever hands out a `Frame` whose `data` length exactly matches
/// `width * height * 3`. If that invariant were ever loosened,
/// `build_ycbcr_planes` would silently pad a short buffer's tail
/// with each plane's neutral fill value (or silently drop a long
/// buffer's excess), producing a self-consistent, fully decodable,
/// but silently corrupted JPEG -- the exact failure signature of
/// several real bugs already found and fixed in this crate. This
/// test does not exercise `synthvid-encode` code at all; it exists
/// solely to fail loudly if `synthvid_scene::Frame` ever stops
/// enforcing the length it currently enforces.
#[test]
fn frame_rejects_mismatched_data_length() {
    assert_eq!(check_frame_rejects_mismatched_data_length(), Ok(()));
}

fn check_frame_rejects_mismatched_data_length() -> Result<(), Fail> {
    let width = Width::new(4).or_fail()?;
    let height = Height::new(4).or_fail()?;
    let dims = Dimensions::new(width, height);

    // 4 * 4 * 3 bytes is exact; one fewer and one more are not.
    let too_short = vec![0_u8; 47];
    let too_long = vec![0_u8; 49];
    let correct = vec![0_u8; 48];

    ensure!(
        Frame::new(dims, PixelBuffer(too_short)).is_err(),
        "Frame::new must reject a buffer shorter than width*height*3"
    );
    ensure!(
        Frame::new(dims, PixelBuffer(too_long)).is_err(),
        "Frame::new must reject a buffer longer than width*height*3"
    );
    ensure!(
        Frame::new(dims, PixelBuffer(correct)).is_ok(),
        "Frame::new must accept a buffer of exactly width*height*3 bytes"
    );
    Ok(())
}
