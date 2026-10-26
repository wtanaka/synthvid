use super::*;
use core::num::NonZeroU16;
use synthvid_scene::frame::PixelBuffer;
use synthvid_scene::{Height, Width};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// A frame size, named by its columns and rows so the two counts cannot be swapped.
#[derive(Copy, Clone, Debug)]
struct Size {
    /// Number of sensels across the frame.
    columns: Width,
    /// Number of sensels down the frame.
    rows: Height,
}

/// A 16 by 16 frame, whose center lies on the corner shared by sensels 7 and 8.
fn square_sixteen() -> Result<Size, Fail> {
    Ok(Size {
        columns: Width::new(16).or_fail()?,
        rows: Height::new(16).or_fail()?,
    })
}

/// A 17 by 31 frame, whose center lies on the middle sensel of the middle row.
fn tall_seventeen_by_thirty_one() -> Result<Size, Fail> {
    Ok(Size {
        columns: Width::new(17).or_fail()?,
        rows: Height::new(31).or_fail()?,
    })
}

/// Dimensions of a frame of the given size.
fn dims(size: Size) -> Dimensions {
    Dimensions::new(size.columns, size.rows)
}

/// A frame of the given size with every channel set to `value`.
fn flat_frame(size: Size, value: Channel) -> Result<Frame, Fail> {
    let color = Rgb8 {
        red: value,
        green: value,
        blue: value,
    };
    Frame::from_color(dims(size), color).or_fail()
}

/// A radial vignette of the given raw strength, in which 256 is the whole brightness.
fn radial(raw: NonZeroU16) -> Result<Vignette, Fail> {
    Ok(Vignette::Radial(VignetteStrength::new(raw).or_fail()?))
}

/// The full strength of a radial vignette, a raw value of 256.
fn full_strength() -> Result<NonZeroU16, Fail> {
    NonZeroU16::new(256).or_fail()
}

/// The coordinate of the sensel at the given column and row.
fn sensel(column: PixelColumn, row: PixelRow) -> PixelCoord {
    PixelCoord::new(column, row)
}

/// The brightness factor of the sensel at the given column and row of a frame of the given size,
/// under a radial vignette of the given raw strength.
fn factor_at(
    raw: NonZeroU16,
    size: Size,
    column: PixelColumn,
    row: PixelRow,
) -> Result<VignetteFactor, Fail> {
    let offsets = FrameCenterOffsets::of_sensel(column, row, dims(size)).or_fail()?;
    let extent = DoubledExtent::from_dimensions(dims(size)).or_fail()?;
    let strength = VignetteStrength::new(raw).or_fail()?;
    vignette_factor(strength, offsets, extent).or_fail()
}

/// The red channel of the sensel at the given coordinate, after a radial vignette of the given
/// raw strength is applied to a flat frame of `value`.
fn vignetted_red(
    size: Size,
    value: Channel,
    raw: NonZeroU16,
    at: PixelCoord,
) -> Result<Channel, Fail> {
    let mut frame = flat_frame(size, value)?;
    apply_vignette(&mut frame, radial(raw)?).or_fail()?;
    Ok(frame.get_pixel(at).or_fail()?.red)
}

/// Asserts that a sensel of a frame has the given factor under a radial vignette of raw strength.
fn check_factor(
    size: Size,
    raw: NonZeroU16,
    column: PixelColumn,
    row: PixelRow,
    expected: VignetteFactor,
) -> Result<(), Fail> {
    ensure_eq!(factor_at(raw, size, column, row)?, expected);
    Ok(())
}

/// Asserts that a sensel of a flat frame records the given value under a radial vignette.
fn check_channel(
    size: Size,
    value: Channel,
    raw: NonZeroU16,
    at: PixelCoord,
    expected: Channel,
) -> Result<(), Fail> {
    ensure_eq!(vignetted_red(size, value, raw, at)?, expected);
    Ok(())
}

fn check_factor_at_corner_of_sixteen_square() -> Result<(), Fail> {
    // Sensel (0, 0): dx2 = dy2 = -15, sum 450, W^2 + H^2 = 512, loss = 64 * 256 * 450 / 512
    // = 14400, factor = 65536 - 14400 = 51136.
    let size = square_sixteen()?;
    let strength = NonZeroU16::new(64).or_fail()?;
    check_factor(
        size,
        strength,
        PixelColumn(0),
        PixelRow(0),
        VignetteFactor(51136),
    )?;
    // The other three corners are mirror images of (0, 0) and share its factor.
    check_factor(
        size,
        strength,
        PixelColumn(15),
        PixelRow(15),
        VignetteFactor(51136),
    )?;
    check_factor(
        size,
        strength,
        PixelColumn(0),
        PixelRow(15),
        VignetteFactor(51136),
    )?;
    check_factor(
        size,
        strength,
        PixelColumn(15),
        PixelRow(0),
        VignetteFactor(51136),
    )?;
    Ok(())
}

/// The factor at a corner of a 16 by 16 frame, and at its mirror-image corners.
#[test]
fn factor_at_corner_of_sixteen_square() {
    assert_eq!(check_factor_at_corner_of_sixteen_square(), Ok(()));
}

fn check_factor_near_center_and_mid_edge() -> Result<(), Fail> {
    let size = square_sixteen()?;
    let strength = NonZeroU16::new(64).or_fail()?;
    // Sensel (8, 8): dx2 = dy2 = 1, sum 2, loss = 64 * 256 * 2 / 512 = 64, factor 65472.
    check_factor(
        size,
        strength,
        PixelColumn(8),
        PixelRow(8),
        VignetteFactor(65472),
    )?;
    // Sensel (3, 7): dx2 = -9, dy2 = -1, sum 82, loss = 64 * 256 * 82 / 512 = 2624, factor 62912.
    check_factor(
        size,
        strength,
        PixelColumn(3),
        PixelRow(7),
        VignetteFactor(62912),
    )?;
    Ok(())
}

/// The factor near the center and at an off-axis sensel of a 16 by 16 frame.
#[test]
fn factor_near_center_and_mid_edge() {
    assert_eq!(check_factor_near_center_and_mid_edge(), Ok(()));
}

fn check_factor_at_full_strength() -> Result<(), Fail> {
    // Strength 256 at sensel (0, 0): loss = 65536 * 450 / 512 = 57600, factor 7936.
    let size = square_sixteen()?;
    check_factor(
        size,
        full_strength()?,
        PixelColumn(0),
        PixelRow(0),
        VignetteFactor(7936),
    )
}

/// The factor at full strength, where the loss at the corner is most of the brightness.
#[test]
fn factor_at_full_strength() {
    assert_eq!(check_factor_at_full_strength(), Ok(()));
}

fn check_factor_in_non_square_frame() -> Result<(), Fail> {
    // In a 17 by 31 frame, sensel (16, 30) has dx2 = 2 * 16 + 1 - 17 = 16 and
    // dy2 = 2 * 30 + 1 - 31 = 30, so dx2^2 + dy2^2 = 256 + 900 = 1156 and W^2 + H^2 = 1250.
    // With strength 100, loss = 100 * 256 * 1156 / 1250 = 29593600 / 1250 = 23674 (floor of
    // 23674.88), so the factor is 65536 - 23674 = 41862.
    let size = tall_seventeen_by_thirty_one()?;
    check_factor(
        size,
        NonZeroU16::new(100).or_fail()?,
        PixelColumn(16),
        PixelRow(30),
        VignetteFactor(41862),
    )
}

/// The factor at the far corner of a 17 by 31 frame, whose sides differ in length.
#[test]
fn factor_in_non_square_frame() {
    assert_eq!(check_factor_in_non_square_frame(), Ok(()));
}

fn check_channel_values_at_sixteen_square() -> Result<(), Fail> {
    let size = square_sixteen()?;
    let strength = NonZeroU16::new(64).or_fail()?;
    // 200 at (0, 0) with factor 51136: (200 * 51136 + 32768) / 65536 = 10259968 / 65536 = 156.
    check_channel(
        size,
        Channel(200),
        strength,
        sensel(PixelColumn(0), PixelRow(0)),
        Channel(156),
    )?;
    check_channel(
        size,
        Channel(200),
        strength,
        sensel(PixelColumn(15), PixelRow(15)),
        Channel(156),
    )?;
    check_channel(
        size,
        Channel(200),
        strength,
        sensel(PixelColumn(0), PixelRow(15)),
        Channel(156),
    )?;
    check_channel(
        size,
        Channel(200),
        strength,
        sensel(PixelColumn(15), PixelRow(0)),
        Channel(156),
    )?;
    // 200 at (8, 8) with factor 65472: (200 * 65472 + 32768) / 65536 = 13127168 / 65536 = 200.
    check_channel(
        size,
        Channel(200),
        strength,
        sensel(PixelColumn(8), PixelRow(8)),
        Channel(200),
    )?;
    // 255 at (3, 7) with factor 62912: (255 * 62912 + 32768) / 65536 = 16075328 / 65536 = 245.
    check_channel(
        size,
        Channel(255),
        strength,
        sensel(PixelColumn(3), PixelRow(7)),
        Channel(245),
    )?;
    // 255 at (0, 0) with strength 256 and factor 7936: (255 * 7936 + 32768) / 65536 = 2056448 /
    // 65536 = 31.
    check_channel(
        size,
        Channel(255),
        full_strength()?,
        sensel(PixelColumn(0), PixelRow(0)),
        Channel(31),
    )?;
    Ok(())
}

/// Channel values at known sensels of a 16 by 16 frame, with the rounding worked out by hand.
#[test]
fn channel_values_at_sixteen_square() {
    assert_eq!(check_channel_values_at_sixteen_square(), Ok(()));
}

fn check_channel_value_in_non_square_frame() -> Result<(), Fail> {
    // 255 at (16, 30) with factor 41862: (255 * 41862 + 32768) / 65536 = 10707578 / 65536 = 163.
    check_channel(
        tall_seventeen_by_thirty_one()?,
        Channel(255),
        NonZeroU16::new(100).or_fail()?,
        sensel(PixelColumn(16), PixelRow(30)),
        Channel(163),
    )
}

/// A channel value at the far corner of a 17 by 31 frame, with the rounding worked out by hand.
#[test]
fn channel_value_in_non_square_frame() {
    assert_eq!(check_channel_value_in_non_square_frame(), Ok(()));
}

fn check_none_leaves_frame_unchanged() -> Result<(), Fail> {
    let size = Size {
        columns: Width::new(3).or_fail()?,
        rows: Height::new(2).or_fail()?,
    };
    let bytes = vec![
        7, 201, 99, 254, 0, 128, 1, 2, 3, 250, 251, 252, 17, 34, 51, 68, 85, 102,
    ];
    let original = Frame::new(dims(size), PixelBuffer(bytes)).or_fail()?;
    let mut frame = original.clone();
    apply_vignette(&mut frame, Vignette::None).or_fail()?;
    ensure_eq!(frame, original);
    Ok(())
}

/// A vignette of no falloff leaves every byte of an arbitrary frame as it was.
#[test]
fn none_leaves_frame_unchanged() {
    assert_eq!(check_none_leaves_frame_unchanged(), Ok(()));
}

fn check_flat_gray_falls_off_toward_corners() -> Result<(), Fail> {
    let size = square_sixteen()?;
    let strength = full_strength()?;
    let center = vignetted_red(
        size,
        Channel(200),
        strength,
        sensel(PixelColumn(8), PixelRow(8)),
    )?;
    let edge = vignetted_red(
        size,
        Channel(200),
        strength,
        sensel(PixelColumn(0), PixelRow(8)),
    )?;
    let corner = vignetted_red(
        size,
        Channel(200),
        strength,
        sensel(PixelColumn(0), PixelRow(0)),
    )?;
    ensure!(
        center > edge,
        "the center {center:?} must be brighter than the edge middle {edge:?}"
    );
    ensure!(
        edge > corner,
        "the edge middle {edge:?} must be brighter than the corner {corner:?}"
    );
    Ok(())
}

/// A flat gray frame is brightest at its center and darkest at its corner.
#[test]
fn flat_gray_falls_off_toward_corners() {
    assert_eq!(check_flat_gray_falls_off_toward_corners(), Ok(()));
}

fn check_flat_frame_is_symmetric() -> Result<(), Fail> {
    let size = square_sixteen()?;
    let mut frame = flat_frame(size, Channel(200))?;
    apply_vignette(&mut frame, radial(NonZeroU16::new(100).or_fail()?)?).or_fail()?;
    for row in 0..16_u16 {
        for column in 0..16_u16 {
            let mirror_column = 15_u16.checked_sub(column).or_fail()?;
            let mirror_row = 15_u16.checked_sub(row).or_fail()?;
            let here = frame.get_pixel(sensel(PixelColumn(column), PixelRow(row)));
            let across = frame.get_pixel(sensel(PixelColumn(mirror_column), PixelRow(row)));
            let down = frame.get_pixel(sensel(PixelColumn(column), PixelRow(mirror_row)));
            ensure_eq!(here, across);
            ensure_eq!(here, down);
        }
    }
    Ok(())
}

/// A flat frame is unchanged by flipping it horizontally or vertically.
#[test]
fn flat_frame_is_symmetric() {
    assert_eq!(check_flat_frame_is_symmetric(), Ok(()));
}

fn check_zero_stays_zero() -> Result<(), Fail> {
    let size = square_sixteen()?;
    let mut frame = flat_frame(size, Channel(0))?;
    apply_vignette(&mut frame, radial(full_strength()?)?).or_fail()?;
    for row in 0..16_u16 {
        for column in 0..16_u16 {
            let color = frame
                .get_pixel(sensel(PixelColumn(column), PixelRow(row)))
                .or_fail()?;
            ensure_eq!(color.red, Channel(0));
            ensure_eq!(color.green, Channel(0));
            ensure_eq!(color.blue, Channel(0));
        }
    }
    Ok(())
}

/// A black frame stays black at every sensel, whatever the strength.
#[test]
fn zero_stays_zero() {
    assert_eq!(check_zero_stays_zero(), Ok(()));
}

fn check_single_sensel_frame_is_unchanged() -> Result<(), Fail> {
    let size = Size {
        columns: Width::new(1).or_fail()?,
        rows: Height::new(1).or_fail()?,
    };
    let center = sensel(PixelColumn(0), PixelRow(0));
    // The only sensel is the center, so its offsets are zero and its factor is 65536.
    for raw in [1_u16, 100, 256] {
        let strength = NonZeroU16::new(raw).or_fail()?;
        check_factor(
            size,
            strength,
            PixelColumn(0),
            PixelRow(0),
            VignetteFactor(0x1_0000),
        )?;
        check_channel(size, Channel(200), strength, center, Channel(200))?;
        check_channel(size, Channel(255), strength, center, Channel(255))?;
    }
    Ok(())
}

/// A one-sensel frame is its own center, so no strength changes it.
#[test]
fn single_sensel_frame_is_unchanged() {
    assert_eq!(check_single_sensel_frame_is_unchanged(), Ok(()));
}

fn check_same_inputs_give_identical_frames() -> Result<(), Fail> {
    let size = tall_seventeen_by_thirty_one()?;
    let vignette = radial(NonZeroU16::new(77).or_fail()?)?;
    let mut first = flat_frame(size, Channel(213))?;
    let mut second = flat_frame(size, Channel(213))?;
    apply_vignette(&mut first, vignette).or_fail()?;
    apply_vignette(&mut second, vignette).or_fail()?;
    ensure_eq!(first, second);
    Ok(())
}

/// Applying the same vignette to the same frame twice gives identical frames.
#[test]
fn same_inputs_give_identical_frames() {
    assert_eq!(check_same_inputs_give_identical_frames(), Ok(()));
}

/// A factor is accepted from zero to 65536 and refused on either side of that range.
#[test]
fn factor_constructor_accepts_only_zero_to_unity() {
    assert_eq!(
        check_factor_constructor_accepts_only_zero_to_unity(),
        Ok(())
    );
}

fn check_factor_constructor_accepts_only_zero_to_unity() -> Result<(), Fail> {
    ensure_eq!(
        VignetteFactor::from_remaining(RemainingBrightness(0)),
        Ok(VignetteFactor(0)),
        "a factor of zero is accepted"
    );
    ensure_eq!(
        VignetteFactor::from_remaining(RemainingBrightness(0x1_0000)),
        Ok(VignetteFactor(0x1_0000)),
        "a factor of unity is accepted"
    );
    ensure_eq!(
        VignetteFactor::from_remaining(RemainingBrightness(0x1_0001)),
        Err(VignetteError::Overflow),
        "a factor above unity is refused"
    );
    ensure_eq!(
        VignetteFactor::from_remaining(RemainingBrightness(-1)),
        Err(VignetteError::Overflow),
        "a negative factor is refused"
    );
    Ok(())
}
