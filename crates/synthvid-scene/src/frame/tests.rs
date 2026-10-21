use super::*;
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

#[test]
fn frame_buffer_length_invariant() {
    assert_eq!(check_frame_buffer_length_invariant(), Ok(()));
}

fn check_frame_buffer_length_invariant() -> Result<(), Fail> {
    let frame_width = Width::new(4).or_fail()?;
    let frame_height = Height::new(3).or_fail()?;
    let dims = Dimensions {
        width: frame_width,
        height: frame_height,
    };
    // Expected buffer size: 4 * 3 * 3 = 36 bytes.
    let expected_len = required_buffer_len(dims).or_fail()?.get();
    ensure_eq!(expected_len, 36, "4x3 RGB frame requires exactly 36 bytes");

    // Too short buffer must be rejected.
    let too_short = vec![0_u8; 35];
    ensure!(
        Frame::new(dims, PixelBuffer(too_short)).is_err(),
        "buffer shorter than required must return Err"
    );

    // Too long buffer must be rejected.
    let too_long = vec![0_u8; 37];
    ensure!(
        Frame::new(dims, PixelBuffer(too_long)).is_err(),
        "buffer longer than required must return Err"
    );

    // Empty buffer must be rejected.
    ensure!(
        Frame::new(dims, PixelBuffer(Vec::new())).is_err(),
        "empty buffer must return Err"
    );

    // Exact length buffer must succeed.
    let exact = vec![0_u8; 36];
    let frame = Frame::new(dims, PixelBuffer(exact)).or_fail()?;
    ensure_eq!(frame.data().0.len(), 36, "frame len must equal 36");
    ensure!(!frame.data().0.is_empty(), "frame must not be empty");
    ensure_eq!(frame.dimensions(), dims, "dimensions must match");
    ensure_eq!(frame.width(), frame_width, "width must match");
    ensure_eq!(frame.height(), frame_height, "height must match");
    Ok(())
}

#[test]
fn frame_zeroed_and_from_color() {
    assert_eq!(check_frame_zeroed_and_from_color(), Ok(()));
}

fn check_frame_zeroed_and_from_color() -> Result<(), Fail> {
    let frame_width = Width::new(2).or_fail()?;
    let frame_height = Height::new(2).or_fail()?;
    let dims = Dimensions {
        width: frame_width,
        height: frame_height,
    };

    let zeroed = Frame::zeroed(dims).or_fail()?;
    ensure_eq!(zeroed.data().0.len(), 12, "2x2 frame must have 12 bytes");
    for byte in &zeroed.data().0 {
        ensure_eq!(*byte, 0, "all bytes in zeroed frame must be 0");
    }

    let red = Rgb8 {
        red: Channel(255),
        green: Channel(0),
        blue: Channel(0),
    };
    let color_frame = Frame::from_color(dims, red).or_fail()?;
    for row in 0..2_u16 {
        for column in 0..2_u16 {
            ensure_eq!(
                color_frame.pixel(PixelCoord::new(PixelColumn(column), PixelRow(row))),
                Some(red),
                "pixel must match filled color"
            );
        }
    }
    Ok(())
}

#[test]
fn frame_bounds_checked_accessors() {
    assert_eq!(check_frame_bounds_checked_accessors(), Ok(()));
}

fn check_frame_bounds_checked_accessors() -> Result<(), Fail> {
    let frame_width = Width::new(3).or_fail()?;
    let frame_height = Height::new(2).or_fail()?;
    let dims = Dimensions {
        width: frame_width,
        height: frame_height,
    };
    let mut frame = Frame::zeroed(dims).or_fail()?;

    // Within bounds
    let col = Rgb8 {
        red: Channel(10),
        green: Channel(20),
        blue: Channel(30),
    };
    let coord_1_1 = PixelCoord::new(PixelColumn(1), PixelRow(1));
    ensure!(
        frame.set_pixel(coord_1_1, col).is_ok(),
        "setting pixel within bounds must succeed"
    );
    ensure_eq!(
        frame.pixel(coord_1_1),
        Some(col),
        "pixel at (1, 1) must return set color"
    );
    ensure_eq!(
        frame.get_pixel(coord_1_1),
        Some(col),
        "get_pixel at (1, 1) must return set color"
    );
    ensure_eq!(
        frame.pixel_bytes(coord_1_1),
        Some(PixelBytes([10, 20, 30])),
        "pixel_bytes at (1, 1) must return [10, 20, 30]"
    );
    check_out_of_bounds_accessors(&mut frame, col)
}

/// Checks that every accessor of the 3x2 `frame` refuses a coordinate on or
/// past its edges, and that its raw bytes end where its pixels do.
fn check_out_of_bounds_accessors(frame: &mut Frame, col: Rgb8) -> Result<(), Fail> {
    // Out of bounds: x == width
    let coord_3_0 = PixelCoord::new(PixelColumn(3), PixelRow(0));
    ensure!(
        frame.pixel(coord_3_0).is_none(),
        "pixel at x == width must return None"
    );
    ensure!(
        frame.pixel_bytes(coord_3_0).is_none(),
        "pixel_bytes at x == width must return None"
    );
    ensure!(
        frame.set_pixel(coord_3_0, col).is_err(),
        "set_pixel at x == width must return None"
    );

    // Out of bounds: y == height
    let coord_0_2 = PixelCoord::new(PixelColumn(0), PixelRow(2));
    ensure!(
        frame.pixel(coord_0_2).is_none(),
        "pixel at y == height must return None"
    );
    ensure!(
        frame.set_pixel(coord_0_2, col).is_err(),
        "set_pixel at y == height must return None"
    );

    // Far out of bounds
    let coord_max = PixelCoord::new(PixelColumn(u16::MAX), PixelRow(u16::MAX));
    ensure!(
        frame.pixel(coord_max).is_none(),
        "pixel at u16::MAX must return None"
    );
    ensure!(
        frame.pixel_bytes(coord_max).is_none(),
        "pixel_bytes at u16::MAX must return None"
    );
    ensure!(
        frame.set_pixel(coord_max, col).is_err(),
        "set_pixel at u16::MAX must return None"
    );

    // Raw byte access
    ensure_eq!(frame.data().0.first(), Some(&0), "byte 0 must be 0");
    ensure!(
        frame.data().0.get(18).is_none(),
        "byte index 18 on 18-byte buffer must return None"
    );
    Ok(())
}

#[test]
fn frame_fill() {
    assert_eq!(check_frame_fill(), Ok(()));
}

fn check_frame_fill() -> Result<(), Fail> {
    let frame_width = Width::new(2).or_fail()?;
    let frame_height = Height::new(2).or_fail()?;
    let dims = Dimensions {
        width: frame_width,
        height: frame_height,
    };
    let mut frame = Frame::zeroed(dims).or_fail()?;

    let green = Rgb8 {
        red: Channel(0),
        green: Channel(255),
        blue: Channel(0),
    };
    frame.fill(green);

    for row in 0..2_u16 {
        for column in 0..2_u16 {
            ensure_eq!(
                frame.pixel(PixelCoord::new(PixelColumn(column), PixelRow(row))),
                Some(green),
                "every pixel after fill must match green"
            );
        }
    }
    Ok(())
}

#[test]
fn frame_blend_pixel() {
    assert_eq!(check_frame_blend_pixel(), Ok(()));
}

fn check_frame_blend_pixel() -> Result<(), Fail> {
    let frame_width = Width::new(2).or_fail()?;
    let frame_height = Height::new(2).or_fail()?;
    let dims = Dimensions {
        width: frame_width,
        height: frame_height,
    };
    let mut frame = Frame::zeroed(dims).or_fail()?;

    // Set pixel (0, 0) to white (255, 255, 255)
    let white = Rgb8 {
        red: Channel(255),
        green: Channel(255),
        blue: Channel(255),
    };
    let coord_0_0 = PixelCoord::new(PixelColumn(0), PixelRow(0));
    ensure!(
        frame.set_pixel(coord_0_0, white).is_ok(),
        "set_pixel(0, 0) must succeed"
    );

    // Blend with black (0, 0, 0) at alpha 0 -> must remain white
    let black = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    };
    ensure!(
        frame.blend_pixel(coord_0_0, black, Alpha(0)).is_ok(),
        "blend at alpha 0 must succeed"
    );
    ensure_eq!(
        frame.pixel(coord_0_0),
        Some(white),
        "pixel with alpha 0 blend must remain unchanged"
    );

    // Blend with red (255, 0, 0) at alpha 255 -> must become pure red
    let red = Rgb8 {
        red: Channel(255),
        green: Channel(0),
        blue: Channel(0),
    };
    ensure!(
        frame.blend_pixel(coord_0_0, red, Alpha(255)).is_ok(),
        "blend at alpha 255 must succeed"
    );
    ensure_eq!(
        frame.pixel(coord_0_0),
        Some(red),
        "pixel with alpha 255 blend must become source color"
    );
    check_half_alpha_blend(&mut frame)?;

    // Out of bounds blend returns None
    let coord_2_0 = PixelCoord::new(PixelColumn(2), PixelRow(0));
    ensure!(
        frame.blend_pixel(coord_2_0, red, Alpha(255)).is_err(),
        "out of bounds blend must return None"
    );
    let coord_max = PixelCoord::new(PixelColumn(u16::MAX), PixelRow(u16::MAX));
    ensure!(
        frame.blend_pixel(coord_max, red, Alpha(255)).is_err(),
        "out of bounds blend at u16::MAX must return None"
    );
    Ok(())
}

/// Checks a half-opacity blend at pixel (1, 1) of `frame` against the
/// hand-computed rounded value.
fn check_half_alpha_blend(frame: &mut Frame) -> Result<(), Fail> {
    // Set pixel (1, 1) to (100, 100, 100) and blend with (200, 200, 200) at alpha 128
    let gray100 = Rgb8 {
        red: Channel(100),
        green: Channel(100),
        blue: Channel(100),
    };
    let coord_1_1 = PixelCoord::new(PixelColumn(1), PixelRow(1));
    ensure!(
        frame.set_pixel(coord_1_1, gray100).is_ok(),
        "set_pixel(1, 1) must succeed"
    );
    let gray200 = Rgb8 {
        red: Channel(200),
        green: Channel(200),
        blue: Channel(200),
    };
    ensure!(
        frame.blend_pixel(coord_1_1, gray200, Alpha(128)).is_ok(),
        "blend at alpha 128 must succeed"
    );
    // Blend formula: (200 * 128 + 100 * 127 + 127) / 255 = (25600 + 12700 + 127) / 255 = 38427 / 255 = 150
    let blended = frame.pixel(coord_1_1).or_fail()?;
    ensure_eq!(
        blended.red,
        Channel(150),
        "blended channel at 50% opacity must equal 150"
    );
    ensure_eq!(
        blended.green,
        Channel(150),
        "blended channel at 50% opacity must equal 150"
    );
    ensure_eq!(
        blended.blue,
        Channel(150),
        "blended channel at 50% opacity must equal 150"
    );
    Ok(())
}
