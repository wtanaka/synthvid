//! Oracle tests for `synthvid-encode`'s JPEG output, using `djpeg` (from
//! libjpeg-turbo/mozjpeg) and Python's PIL/Pillow as independent decoders:
//! these tests never trust that "the bytes we wrote look like valid JPEG
//! to our own code" -- they hand those bytes to real, independently
//! implemented decoders and check what comes back.
//!
//! Like the JSON oracle tests in [`crate::tests`], these skip gracefully
//! (rather than fail) when their external tool is not available.

use std::io::Write;
use std::process::{Command, Stdio};
use synthvid_encode::{encode_jpeg, ChromaSampling, Quality};
use synthvid_scene::{Dimensions, Frame, Height, Rgb8, Width};

use crate::support::is_command_available;

/// Builds a checkerboard-pattern frame: 4x4-pixel black/white blocks.
fn make_checkerboard_frame(width: u16, height: u16) -> Frame {
    let dims = frame_dims(width, height);
    let mut data = Vec::with_capacity(
        usize::from(width)
            .saturating_mul(usize::from(height))
            .saturating_mul(3),
    );
    for y in 0..u32::from(height) {
        for x in 0..u32::from(width) {
            let block_sum = x.saturating_div(4).saturating_add(y.saturating_div(4));
            let value = if block_sum.is_multiple_of(2) {
                255_u8
            } else {
                0_u8
            };
            data.push(value);
            data.push(value);
            data.push(value);
        }
    }
    Frame::new(dims, data).expect("buffer matches dimensions")
}

/// Builds a diagonal gradient frame: red varies with `x`, green with `y`,
/// blue with `width - 1 - x` -- a non-uniform pattern, since a solid color
/// would hide many of the bugs this codebase has actually hit (see this
/// crate's own encoder module docs on preferring an awkward concrete input
/// over a convenient one).
fn make_gradient_frame(width: u16, height: u16) -> Frame {
    let dims = frame_dims(width, height);
    let mut data = Vec::with_capacity(
        usize::from(width)
            .saturating_mul(usize::from(height))
            .saturating_mul(3),
    );
    for y in 0..u32::from(height) {
        for x in 0..u32::from(width) {
            let r = u8::try_from(x.saturating_mul(8)).unwrap_or(u8::MAX);
            let g = u8::try_from(y.saturating_mul(8)).unwrap_or(u8::MAX);
            let b = u8::try_from(
                u32::from(width)
                    .saturating_sub(1)
                    .saturating_sub(x)
                    .saturating_mul(8),
            )
            .unwrap_or(u8::MAX);
            data.push(r);
            data.push(g);
            data.push(b);
        }
    }
    Frame::new(dims, data).expect("buffer matches dimensions")
}

/// Builds a solid-color frame.
fn make_solid_frame(width: u16, height: u16, color: Rgb8) -> Frame {
    Frame::from_color(frame_dims(width, height), color).expect("creating frame should succeed")
}

fn frame_dims(width: u16, height: u16) -> Dimensions {
    let w = Width::new(width).expect("test width is nonzero");
    let h = Height::new(height).expect("test height is nonzero");
    Dimensions::new(w, h)
}

/// Tries to decode `jpeg_bytes` with `djpeg`, an independently implemented
/// JPEG decoder (from libjpeg-turbo/mozjpeg), feeding the bytes over
/// stdin and writing decoded output to `/dev/null`.
///
/// Returns `Ok(Some(()))` if `djpeg` was used and decoding succeeded,
/// `Ok(None)` if `djpeg` is not available, `Err(msg)` if decoding failed.
fn decode_jpeg_with_djpeg(jpeg_bytes: &[u8]) -> Result<Option<()>, String> {
    if !is_command_available("djpeg") {
        return Ok(None);
    }

    let mut child = Command::new("djpeg")
        .arg("-outfile")
        .arg("/dev/null")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn djpeg: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(jpeg_bytes)
            .map_err(|e| format!("failed to write to djpeg stdin: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for djpeg: {e}"))?;

    if output.status.success() {
        Ok(Some(()))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("djpeg decoding failed: {stderr}"))
    }
}

/// Tries to decode `jpeg_bytes` with Python's PIL/Pillow, an independently
/// implemented JPEG decoder, feeding the bytes over stdin and reading raw
/// RGB pixel bytes back from stdout.
///
/// Returns `Ok(Some(pixels))` if PIL was used and decoding succeeded,
/// `Ok(None)` if python3/PIL is not available, `Err(msg)` if decoding
/// failed.
fn decode_jpeg_with_pil(jpeg_bytes: &[u8]) -> Result<Option<Vec<u8>>, String> {
    if !is_command_available("python3") {
        return Ok(None);
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
        .map_err(|e| format!("failed to spawn python3: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(jpeg_bytes)
            .map_err(|e| format!("failed to write to python3 stdin: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for python3: {e}"))?;

    if output.status.success() {
        Ok(Some(output.stdout))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("PIL decoding failed: {stderr}"))
    }
}

/// The mean absolute per-byte difference between `decoded` and `expected`,
/// pixel data of identical length assumed.
fn mean_absolute_error(decoded: &[u8], expected: &[u8]) -> u32 {
    let mut error_sum: u32 = 0;
    let mut count: u32 = 0;
    for (&d, &e) in decoded.iter().zip(expected.iter()) {
        error_sum = error_sum.saturating_add(i32::from(d).abs_diff(i32::from(e)));
        count = count.saturating_add(1);
    }
    error_sum.checked_div(count).unwrap_or(0)
}

/// A real, independent JPEG decoder accepts a 4:4:4 baseline JPEG this
/// crate's encoder produces.
#[test]
fn test_encoded_jpeg_yuv444_decodes_with_djpeg() {
    let frame = make_solid_frame(
        24,
        24,
        Rgb8 {
            r: 100,
            g: 150,
            b: 200,
        },
    );
    let quality = Quality::new(90).expect("90 is a valid quality");
    let jpeg_bytes = encode_jpeg(&frame, quality, ChromaSampling::Yuv444);

    match decode_jpeg_with_djpeg(&jpeg_bytes) {
        Ok(Some(()) | None) => {}
        Err(e) => panic!("{e}"),
    }
}

/// A real, independent JPEG decoder accepts a 4:2:0 (chroma-subsampled)
/// baseline JPEG this crate's encoder produces.
#[test]
fn test_encoded_jpeg_yuv420_decodes_with_djpeg() {
    let frame = make_gradient_frame(32, 32);
    let quality = Quality::new(85).expect("85 is a valid quality");
    let jpeg_bytes = encode_jpeg(&frame, quality, ChromaSampling::Yuv420);

    match decode_jpeg_with_djpeg(&jpeg_bytes) {
        Ok(Some(()) | None) => {}
        Err(e) => panic!("{e}"),
    }
}

/// A real, independent JPEG decoder accepts a 4:2:0 JPEG whose frame size
/// is a multiple of neither 8 nor 16, exercising the MCU grid's
/// edge-padding path (`Plane::extract_block`'s out-of-bounds padding, and
/// `encode_scan`'s partial trailing MCU row/column).
#[test]
fn test_encoded_jpeg_yuv420_non_multiple_of_16_decodes_with_djpeg() {
    let frame = make_gradient_frame(50, 37);
    let quality = Quality::new(75).expect("75 is a valid quality");
    let jpeg_bytes = encode_jpeg(&frame, quality, ChromaSampling::Yuv420);

    match decode_jpeg_with_djpeg(&jpeg_bytes) {
        Ok(Some(()) | None) => {}
        Err(e) => panic!("{e}"),
    }
}

/// A 4:4:4 checkerboard round-trips through a real decoder with a small
/// mean absolute pixel error, bounding how much a high-quality encode is
/// allowed to distort a sharp-edged pattern.
#[test]
fn test_encoded_jpeg_yuv444_pixel_values_with_pil() {
    let width = 16;
    let height = 16;
    let frame = make_checkerboard_frame(width, height);
    let quality = Quality::new(95).expect("95 is a valid quality");
    let jpeg_bytes = encode_jpeg(&frame, quality, ChromaSampling::Yuv444);

    let Some(decoded) = (match decode_jpeg_with_pil(&jpeg_bytes) {
        Ok(decoded) => decoded,
        Err(e) => panic!("{e}"),
    }) else {
        return; // python3/PIL not available, skip.
    };

    let expected_len = usize::from(width)
        .saturating_mul(usize::from(height))
        .saturating_mul(3);
    assert_eq!(decoded.len(), expected_len);

    let expected: Vec<u8> = frame.data().to_vec();
    let mae = mean_absolute_error(&decoded, &expected);
    assert!(
        mae < 20,
        "mean absolute error {mae} exceeds threshold for 4:4:4"
    );
}

/// A 4:2:0 solid color round-trips through a real decoder with a small
/// mean absolute pixel error, allowing slightly more slack than 4:4:4
/// since chroma subsampling adds its own averaging error.
#[test]
fn test_encoded_jpeg_yuv420_pixel_values_with_pil() {
    let width = 24;
    let height = 24;
    let frame = make_solid_frame(
        width,
        height,
        Rgb8 {
            r: 128,
            g: 192,
            b: 64,
        },
    );
    let quality = Quality::new(90).expect("90 is a valid quality");
    let jpeg_bytes = encode_jpeg(&frame, quality, ChromaSampling::Yuv420);

    let Some(decoded) = (match decode_jpeg_with_pil(&jpeg_bytes) {
        Ok(decoded) => decoded,
        Err(e) => panic!("{e}"),
    }) else {
        return; // python3/PIL not available, skip.
    };

    let expected_len = usize::from(width)
        .saturating_mul(usize::from(height))
        .saturating_mul(3);
    assert_eq!(decoded.len(), expected_len);

    let expected: Vec<u8> = frame.data().to_vec();
    let mae = mean_absolute_error(&decoded, &expected);
    assert!(
        mae < 20,
        "mean absolute error {mae} exceeds threshold for 4:2:0"
    );
}

/// A high-quality (100) 4:4:4 gradient nearly round-trips exactly, since
/// quality 100 disables almost all quantization.
#[test]
fn test_encoded_jpeg_high_quality_preserves_gradient_with_pil() {
    let width = 32;
    let height = 32;
    let frame = make_gradient_frame(width, height);
    let quality = Quality::new(100).expect("100 is a valid quality");
    let jpeg_bytes = encode_jpeg(&frame, quality, ChromaSampling::Yuv444);

    let Some(decoded) = (match decode_jpeg_with_pil(&jpeg_bytes) {
        Ok(decoded) => decoded,
        Err(e) => panic!("{e}"),
    }) else {
        return; // python3/PIL not available, skip.
    };

    let expected_len = usize::from(width)
        .saturating_mul(usize::from(height))
        .saturating_mul(3);
    assert_eq!(decoded.len(), expected_len);

    let expected: Vec<u8> = frame.data().to_vec();
    let mae = mean_absolute_error(&decoded, &expected);
    assert!(
        mae < 15,
        "mean absolute error {mae} exceeds threshold for high quality 4:4:4"
    );
}

/// The encoded bytes carry a well-formed SOI marker, an APP0/JFIF marker,
/// and an EOI marker at the expected positions -- checked directly against
/// the byte stream, then sanity-checked with PIL if available.
#[test]
fn test_encoded_jpeg_has_jfif_markers() {
    let frame = make_solid_frame(
        24,
        24,
        Rgb8 {
            r: 200,
            g: 100,
            b: 50,
        },
    );
    let quality = Quality::new(80).expect("80 is a valid quality");
    let jpeg_bytes = encode_jpeg(&frame, quality, ChromaSampling::Yuv444);

    assert_eq!(jpeg_bytes.first().copied(), Some(0xFF));
    assert_eq!(jpeg_bytes.get(1).copied(), Some(0xD8));

    let eoi_start = jpeg_bytes.len().saturating_sub(2);
    assert_eq!(jpeg_bytes.get(eoi_start).copied(), Some(0xFF));
    assert_eq!(jpeg_bytes.last().copied(), Some(0xD9));

    let found_app0 = jpeg_bytes.windows(2).any(|w| w == [0xFF, 0xE0]);
    assert!(found_app0, "APP0 (JFIF) marker not found");

    match decode_jpeg_with_pil(&jpeg_bytes) {
        Ok(Some(_) | None) => {}
        Err(e) => panic!("{e}"),
    }
}
