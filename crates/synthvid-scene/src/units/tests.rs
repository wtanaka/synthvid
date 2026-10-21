use super::*;

use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

use crate::color::{Channel, Rgb8};

#[test]
fn newtypes_and_framerate() {
    assert_eq!(check_newtypes_and_framerate(), Ok(()));
}

fn check_newtypes_and_framerate() -> Result<(), Fail> {
    // FrameIndex
    let fi0 = FrameIndex::new(0);
    ensure_eq!(fi0.get(), 0, "frame index 0");
    let fi42 = FrameIndex::new(42);
    ensure_eq!(fi42.get(), 42, "frame index new / get");

    // FrameCount
    ensure!(FrameCount::new(0).is_err(), "frame count 0 rejected");
    let fc = FrameCount::new(100).or_fail()?;
    ensure_eq!(fc.get().get(), 100, "frame count get");

    // Width & Height & Dimensions
    ensure!(Width::new(0).is_err(), "width 0 rejected");
    ensure!(Height::new(0).is_err(), "height 0 rejected");
    let frame_width = Width::new(1920).or_fail()?;
    let frame_height = Height::new(1080).or_fail()?;
    let dims = Dimensions::new(frame_width, frame_height);
    ensure_eq!(dims.width.get().get(), 1920, "dims width");
    ensure_eq!(dims.height.get().get(), 1080, "dims height");

    // Seed
    let seed = Seed::new(12345);
    ensure_eq!(seed.get(), 12345, "seed value");

    // Rgb8
    let col = Rgb8 {
        red: Channel(255),
        green: Channel(128),
        blue: Channel(64),
    };
    ensure_eq!(col.red, Channel(255), "red channel");
    ensure_eq!(col.green, Channel(128), "green channel");
    ensure_eq!(col.blue, Channel(64), "blue channel");

    // FrameRate
    let d1 = NonZeroI64::new(1).or_fail()?;
    let zero_ratio = Ratio::new(Numerator(0), d1).or_fail()?;
    let neg_ratio = Ratio::new(Numerator(-24), d1).or_fail()?;

    ensure!(FrameRate::new(zero_ratio).is_err(), "rate 0 rejected");
    ensure!(FrameRate::new(neg_ratio).is_err(), "negative rate rejected");
    ensure!(FrameRate::from_fps(0).is_err(), "fps 0 rejected");

    // Integer rate 24 fps
    let fps24 = FrameRate::from_fps(24).or_fail()?;
    ensure_eq!(fps24.get().numer().0, 24, "fps 24 numerator");
    ensure_eq!(fps24.get().denom().get(), 1, "fps 24 denominator");

    // Fractional rate 30000/1001 (NTSC)
    let nz1001 = NonZeroU32::new(1001).or_fail()?;
    let ntsc = FrameRate::from_fraction(30000, nz1001).or_fail()?;
    ensure_eq!(ntsc.ratio().numer().0, 30000, "NTSC numerator");
    ensure_eq!(ntsc.ratio().denom().get(), 1001, "NTSC denominator");

    // Check TryFrom
    let d2 = NonZeroI64::new(2).or_fail()?;
    let pos_ratio = Ratio::new(Numerator(60), d2).or_fail()?;
    let try_res = FrameRate::try_from(pos_ratio);
    ensure!(try_res.is_ok(), "TryFrom positive ratio succeeds");
    let err_res = FrameRate::try_from(neg_ratio);
    ensure_eq!(
        err_res,
        Err(FrameRateError::NonPositive),
        "TryFrom negative ratio fails"
    );
    Ok(())
}
