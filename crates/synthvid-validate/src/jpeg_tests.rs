//! Oracle tests for `synthvid-encode`'s JPEG output, using `djpeg` (from
//! libjpeg-turbo/mozjpeg) and Python's PIL/Pillow as independent decoders:
//! these tests never trust that "the bytes we wrote look like valid JPEG
//! to our own code" -- they hand those bytes to real, independently
//! implemented decoders and check what comes back.
//!
//! Like the JSON oracle tests in [`crate::tests`], these skip gracefully
//! (rather than fail) when their external tool is not available.

use core::fmt;
use core::num::NonZeroU32;
use std::io::Write as _;
use std::process::{Command, Stdio};
use synthvid_encode::{encode_jpeg, ChromaSampling, EncodedFrame, Quality};
use synthvid_scene::{Channel, Dimensions, Frame, Height, PixelBuffer, Rgb8, Width};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

use crate::support::{availability, Availability, OracleFailure, Program};

/// The bytes of an encoded JPEG.
struct Jpeg(EncodedFrame);

/// Raw RGB pixel bytes a decoder returned.
#[derive(Debug)]
struct DecodedPixels(Vec<u8>);

/// The mean absolute per-byte difference between two images.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct MeanAbsoluteError(u32);

impl fmt::Display for MeanAbsoluteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Builds a checkerboard-pattern frame: 4x4-pixel black/white blocks.
fn make_checkerboard_frame(width: Width, height: Height) -> Result<Frame, Fail> {
    let columns = NonZeroU32::from(width.get()).get();
    let rows = NonZeroU32::from(height.get()).get();
    let mut data = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            let block_sum = column
                .checked_div(4)
                .or_fail()?
                .checked_add(row.checked_div(4).or_fail()?)
                .or_fail()?;
            let value = if block_sum.is_multiple_of(2) {
                255_u8
            } else {
                0_u8
            };
            data.extend([value; 3]);
        }
    }
    Frame::new(Dimensions::new(width, height), PixelBuffer(data)).or_fail()
}

/// Builds a diagonal gradient frame: red varies with `x`, green with `y`,
/// blue with `width - 1 - x` -- a non-uniform pattern, since a solid color
/// would hide many of the bugs this codebase has actually hit (see this
/// crate's own encoder module docs on preferring an awkward concrete input
/// over a convenient one). Each channel is eight times its coordinate,
/// clamped to 255.
fn make_gradient_frame(width: Width, height: Height) -> Result<Frame, Fail> {
    let columns = NonZeroU32::from(width.get()).get();
    let rows = NonZeroU32::from(height.get()).get();
    let top = u32::from(u8::MAX);
    let mut data = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            let red = u8::try_from(column.checked_mul(8).or_fail()?.min(top)).or_fail()?;
            let green = u8::try_from(row.checked_mul(8).or_fail()?.min(top)).or_fail()?;
            let mirrored = columns
                .checked_sub(1)
                .or_fail()?
                .checked_sub(column)
                .or_fail()?;
            let blue = u8::try_from(mirrored.checked_mul(8).or_fail()?.min(top)).or_fail()?;
            data.extend([red, green, blue]);
        }
    }
    Frame::new(Dimensions::new(width, height), PixelBuffer(data)).or_fail()
}

/// Tries to decode `jpeg` with `djpeg`, an independently implemented
/// JPEG decoder (from libjpeg-turbo/mozjpeg), feeding the bytes over
/// stdin and writing decoded output to `/dev/null`.
///
/// Returns `Ok(Some(()))` if `djpeg` was used and decoding succeeded,
/// `Ok(None)` if `djpeg` is not available, `Err(failure)` if decoding failed.
fn decode_jpeg_with_djpeg(jpeg: &Jpeg) -> Result<Option<()>, OracleFailure> {
    match availability(Program("djpeg")) {
        Availability::Installed => {}
        Availability::Missing => return Ok(None),
    }

    let mut child = Command::new("djpeg")
        .arg("-outfile")
        .arg("/dev/null")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| OracleFailure(format!("failed to spawn djpeg: {error}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(jpeg.0.as_bytes())
            .map_err(|error| OracleFailure(format!("failed to write to djpeg stdin: {error}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| OracleFailure(format!("failed to wait for djpeg: {error}")))?;

    if output.status.success() {
        Ok(Some(()))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(OracleFailure(format!("djpeg decoding failed: {stderr}")))
    }
}

/// Tries to decode `jpeg` with Python's PIL/Pillow, an independently
/// implemented JPEG decoder, feeding the bytes over stdin and reading raw
/// RGB pixel bytes back from stdout.
///
/// Returns `Ok(Some(pixels))` if PIL was used and decoding succeeded,
/// `Ok(None)` if python3/PIL is not available, `Err(failure)` if decoding
/// failed.
fn decode_jpeg_with_pil(jpeg: &Jpeg) -> Result<Option<DecodedPixels>, OracleFailure> {
    match availability(Program("python3")) {
        Availability::Installed => {}
        Availability::Missing => return Ok(None),
    }

    // Every statement here is a simple statement: a `for` loop cannot
    // follow a `;` in `python3 -c`, so this reads the whole image in one
    // expression rather than looping.
    let code = "import sys, io; \
                from PIL import Image; \
                data = sys.stdin.buffer.read(); \
                img = Image.open(io.BytesIO(data)).convert('RGB'); \
                sys.stdout.buffer.write(img.tobytes())";

    let mut child = Command::new("python3")
        .arg("-c")
        .arg(code)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| OracleFailure(format!("failed to spawn python3: {error}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(jpeg.0.as_bytes())
            .map_err(|error| OracleFailure(format!("failed to write to python3 stdin: {error}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| OracleFailure(format!("failed to wait for python3: {error}")))?;

    if output.status.success() {
        Ok(Some(DecodedPixels(output.stdout)))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(OracleFailure(format!("PIL decoding failed: {stderr}")))
    }
}

/// The mean absolute per-byte difference between `decoded` and the pixel
/// data of `frame`, pixel data of identical length assumed. Comparing no
/// bytes at all is no difference.
fn mean_absolute_error(decoded: &DecodedPixels, frame: &Frame) -> Result<MeanAbsoluteError, Fail> {
    let mut error_sum: u32 = 0;
    let mut count: u32 = 0;
    for (&decoded_byte, &expected_byte) in decoded.0.iter().zip(&frame.data().0) {
        error_sum = error_sum
            .checked_add(u32::from(decoded_byte.abs_diff(expected_byte)))
            .or_fail()?;
        count = count.checked_add(1).or_fail()?;
    }
    if count == 0 {
        return Ok(MeanAbsoluteError(0));
    }
    error_sum
        .checked_div(count)
        .map(MeanAbsoluteError)
        .or_fail()
}

/// Decodes `jpeg` with PIL. `Ok(None)` means python3/PIL is not installed,
/// which is a skip; a rejection is a failure.
fn decoded_by_pil(jpeg: &Jpeg) -> Result<Option<DecodedPixels>, Fail> {
    let decoded_result = decode_jpeg_with_pil(jpeg);
    ensure!(
        decoded_result.is_ok(),
        "PIL rejected the encoded JPEG: {decoded_result:?}"
    );
    decoded_result.or_fail()
}

/// A real, independent JPEG decoder accepts a 4:4:4 baseline JPEG this
/// crate's encoder produces.
#[test]
fn encoded_jpeg_yuv444_decodes_with_djpeg() {
    assert_eq!(check_encoded_jpeg_yuv444_decodes_with_djpeg(), Ok(()));
}

fn check_encoded_jpeg_yuv444_decodes_with_djpeg() -> Result<(), Fail> {
    let frame = Frame::from_color(
        Dimensions::new(Width::new(24).or_fail()?, Height::new(24).or_fail()?),
        Rgb8 {
            red: Channel(100),
            green: Channel(150),
            blue: Channel(200),
        },
    )
    .or_fail()?;
    let quality = Quality::new(90).or_fail()?;
    let jpeg = Jpeg(encode_jpeg(&frame, quality, ChromaSampling::Yuv444));

    // Ok(None) means djpeg is not installed, which is a skip.
    let result = decode_jpeg_with_djpeg(&jpeg);
    ensure!(
        result.is_ok(),
        "djpeg rejected the encoded JPEG: {result:?}"
    );
    Ok(())
}

/// A real, independent JPEG decoder accepts a 4:2:0 (chroma-subsampled)
/// baseline JPEG this crate's encoder produces.
#[test]
fn encoded_jpeg_yuv420_decodes_with_djpeg() {
    assert_eq!(check_encoded_jpeg_yuv420_decodes_with_djpeg(), Ok(()));
}

fn check_encoded_jpeg_yuv420_decodes_with_djpeg() -> Result<(), Fail> {
    let frame = make_gradient_frame(Width::new(32).or_fail()?, Height::new(32).or_fail()?)?;
    let quality = Quality::new(85).or_fail()?;
    let jpeg = Jpeg(encode_jpeg(&frame, quality, ChromaSampling::Yuv420));

    // Ok(None) means djpeg is not installed, which is a skip.
    let result = decode_jpeg_with_djpeg(&jpeg);
    ensure!(
        result.is_ok(),
        "djpeg rejected the encoded JPEG: {result:?}"
    );
    Ok(())
}

/// A real, independent JPEG decoder accepts a 4:2:0 JPEG whose frame size
/// is a multiple of neither 8 nor 16, exercising the MCU grid's
/// edge-padding path (`Plane::extract_block`'s out-of-bounds padding, and
/// `encode_scan`'s partial trailing MCU row/column).
#[test]
fn encoded_jpeg_yuv420_non_multiple_of_16_decodes_with_djpeg() {
    assert_eq!(
        check_encoded_jpeg_yuv420_non_multiple_of_16_decodes_with_djpeg(),
        Ok(())
    );
}

fn check_encoded_jpeg_yuv420_non_multiple_of_16_decodes_with_djpeg() -> Result<(), Fail> {
    let frame = make_gradient_frame(Width::new(50).or_fail()?, Height::new(37).or_fail()?)?;
    let quality = Quality::new(75).or_fail()?;
    let jpeg = Jpeg(encode_jpeg(&frame, quality, ChromaSampling::Yuv420));

    // Ok(None) means djpeg is not installed, which is a skip.
    let result = decode_jpeg_with_djpeg(&jpeg);
    ensure!(
        result.is_ok(),
        "djpeg rejected the encoded JPEG: {result:?}"
    );
    Ok(())
}

/// A 4:4:4 checkerboard round-trips through a real decoder with a small
/// mean absolute pixel error, bounding how much a high-quality encode is
/// allowed to distort a sharp-edged pattern.
#[test]
fn encoded_jpeg_yuv444_pixel_values_with_pil() {
    assert_eq!(check_encoded_jpeg_yuv444_pixel_values_with_pil(), Ok(()));
}

fn check_encoded_jpeg_yuv444_pixel_values_with_pil() -> Result<(), Fail> {
    let frame = make_checkerboard_frame(Width::new(16).or_fail()?, Height::new(16).or_fail()?)?;
    let quality = Quality::new(95).or_fail()?;
    let jpeg = Jpeg(encode_jpeg(&frame, quality, ChromaSampling::Yuv444));

    let Some(decoded) = decoded_by_pil(&jpeg)? else {
        return Ok(());
    };
    // 16 x 16 pixels, three bytes each.
    ensure_eq!(decoded.0.len(), 768);

    let mae = mean_absolute_error(&decoded, &frame)?;
    ensure!(
        mae < MeanAbsoluteError(20),
        "mean absolute error {mae} exceeds threshold for 4:4:4"
    );
    Ok(())
}

/// A 4:2:0 solid color round-trips through a real decoder with a small
/// mean absolute pixel error, allowing slightly more slack than 4:4:4
/// since chroma subsampling adds its own averaging error.
#[test]
fn encoded_jpeg_yuv420_pixel_values_with_pil() {
    assert_eq!(check_encoded_jpeg_yuv420_pixel_values_with_pil(), Ok(()));
}

fn check_encoded_jpeg_yuv420_pixel_values_with_pil() -> Result<(), Fail> {
    let frame = Frame::from_color(
        Dimensions::new(Width::new(24).or_fail()?, Height::new(24).or_fail()?),
        Rgb8 {
            red: Channel(128),
            green: Channel(192),
            blue: Channel(64),
        },
    )
    .or_fail()?;
    let quality = Quality::new(90).or_fail()?;
    let jpeg = Jpeg(encode_jpeg(&frame, quality, ChromaSampling::Yuv420));

    let Some(decoded) = decoded_by_pil(&jpeg)? else {
        return Ok(());
    };
    // 24 x 24 pixels, three bytes each.
    ensure_eq!(decoded.0.len(), 1728);

    let mae = mean_absolute_error(&decoded, &frame)?;
    ensure!(
        mae < MeanAbsoluteError(20),
        "mean absolute error {mae} exceeds threshold for 4:2:0"
    );
    Ok(())
}

/// A high-quality (100) 4:4:4 gradient nearly round-trips exactly, since
/// quality 100 disables almost all quantization.
#[test]
fn encoded_jpeg_high_quality_preserves_gradient_with_pil() {
    assert_eq!(
        check_encoded_jpeg_high_quality_preserves_gradient_with_pil(),
        Ok(())
    );
}

fn check_encoded_jpeg_high_quality_preserves_gradient_with_pil() -> Result<(), Fail> {
    let frame = make_gradient_frame(Width::new(32).or_fail()?, Height::new(32).or_fail()?)?;
    let quality = Quality::new(100).or_fail()?;
    let jpeg = Jpeg(encode_jpeg(&frame, quality, ChromaSampling::Yuv444));

    let Some(decoded) = decoded_by_pil(&jpeg)? else {
        return Ok(());
    };
    // 32 x 32 pixels, three bytes each.
    ensure_eq!(decoded.0.len(), 3072);

    let mae = mean_absolute_error(&decoded, &frame)?;
    ensure!(
        mae < MeanAbsoluteError(15),
        "mean absolute error {mae} exceeds threshold for high quality 4:4:4"
    );
    Ok(())
}

/// The encoded bytes carry a well-formed SOI marker, an APP0/JFIF marker,
/// and an EOI marker at the expected positions -- checked directly against
/// the byte stream, then sanity-checked with PIL if available.
#[test]
fn encoded_jpeg_has_jfif_markers() {
    assert_eq!(check_encoded_jpeg_has_jfif_markers(), Ok(()));
}

fn check_encoded_jpeg_has_jfif_markers() -> Result<(), Fail> {
    let frame = Frame::from_color(
        Dimensions::new(Width::new(24).or_fail()?, Height::new(24).or_fail()?),
        Rgb8 {
            red: Channel(200),
            green: Channel(100),
            blue: Channel(50),
        },
    )
    .or_fail()?;
    let quality = Quality::new(80).or_fail()?;
    let jpeg = Jpeg(encode_jpeg(&frame, quality, ChromaSampling::Yuv444));
    let jpeg_bytes = jpeg.0.as_bytes();

    ensure_eq!(jpeg_bytes.first().copied(), Some(0xFF));
    ensure_eq!(jpeg_bytes.get(1).copied(), Some(0xD8));

    let eoi_start = jpeg_bytes.len().checked_sub(2).or_fail()?;
    ensure_eq!(jpeg_bytes.get(eoi_start).copied(), Some(0xFF));
    ensure_eq!(jpeg_bytes.last().copied(), Some(0xD9));

    let found_app0 = jpeg_bytes.windows(2).any(|window| window == [0xFF, 0xE0]);
    ensure!(found_app0, "APP0 (JFIF) marker not found");

    // Ok(None) means python3/PIL is not installed, which is a skip.
    let result = decode_jpeg_with_pil(&jpeg);
    ensure!(result.is_ok(), "PIL rejected the encoded JPEG: {result:?}");
    Ok(())
}
