use super::*;
use crate::camera::{Camera, Magnification, Rotation, Turns, Zoom};
use crate::scene::{Motion, Scene};
use crate::units::{Dimensions, FrameCount, FrameRate, Height, Width};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// Builds the identity camera: still at the origin with unit magnification.
fn identity_camera() -> Result<Camera, Fail> {
    let zero = Ratio::from_integer(Numerator(0));
    let angle = Turns::new(zero);
    let unit = Zoom::Fixed(Magnification::new(Ratio::from_integer(Numerator(1))).or_fail()?);
    Ok(Camera::new(
        Motion::Fixed(Point {
            across: zero,
            down: zero,
        }),
        Rotation::Fixed(angle),
        unit,
    ))
}

/// Builds an 8x8 scene with the given backdrop and no objects.
fn bare_scene(background: Background) -> Result<Scene, Fail> {
    let width = Width::new(8).or_fail()?;
    let height = Height::new(8).or_fail()?;
    let rate = FrameRate::from_fps(30).or_fail()?;
    let count = FrameCount::new(4).or_fail()?;
    let camera = identity_camera()?;
    Ok(Scene {
        dimensions: Dimensions::new(width, height),
        frame_rate: rate,
        frame_count: count,
        scale: None,
        background,
        camera,
        objects: Vec::new(),
    })
}

#[test]
fn checker_pixels() {
    assert_eq!(check_checker_pixels(), Ok(()));
}

fn check_checker_pixels() -> Result<(), Fail> {
    let cell = NonZeroU16::new(2).or_fail()?;
    let red = Rgb8 {
        red: Channel(255),
        green: Channel(0),
        blue: Channel(0),
    };
    let blue = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(255),
    };
    let scene = bare_scene(Background::Checker {
        cell,
        even: red,
        odd: blue,
    })?;
    let mut frame = Frame::zeroed(scene.dimensions).or_fail()?;
    paint_background(&mut frame, scene.background).or_fail()?;
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(0), PixelRow(0))),
        Some(red),
        "origin square must be a"
    );
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(2), PixelRow(0))),
        Some(blue),
        "next square across must be b"
    );
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(0), PixelRow(2))),
        Some(blue),
        "next square down must be b"
    );
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(2), PixelRow(2))),
        Some(red),
        "diagonal square must be a again"
    );
    Ok(())
}

#[test]
fn grid_pixels() {
    assert_eq!(check_grid_pixels(), Ok(()));
}

fn check_grid_pixels() -> Result<(), Fail> {
    let spacing = NonZeroU16::new(4).or_fail()?;
    let white = Rgb8 {
        red: Channel(255),
        green: Channel(255),
        blue: Channel(255),
    };
    let black = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    };
    let scene = bare_scene(Background::Grid {
        spacing,
        line: white,
        ground: black,
    })?;
    let mut frame = Frame::zeroed(scene.dimensions).or_fail()?;
    paint_background(&mut frame, scene.background).or_fail()?;
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(0), PixelRow(3))),
        Some(white),
        "column zero must be a line"
    );
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(3), PixelRow(0))),
        Some(white),
        "row zero must be a line"
    );
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(1), PixelRow(1))),
        Some(black),
        "off-line pixels must be ground"
    );
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(4), PixelRow(4))),
        Some(white),
        "multiples of spacing must be lines"
    );
    Ok(())
}

#[test]
fn gradient_edges() {
    assert_eq!(check_gradient_edges(), Ok(()));
}

fn check_gradient_edges() -> Result<(), Fail> {
    let from = Rgb8 {
        red: Channel(10),
        green: Channel(20),
        blue: Channel(30),
    };
    let to = Rgb8 {
        red: Channel(200),
        green: Channel(180),
        blue: Channel(160),
    };
    let scene = bare_scene(Background::Gradient {
        from,
        to,
        direction: Direction::Horizontal,
    })?;
    let mut frame = Frame::zeroed(scene.dimensions).or_fail()?;
    paint_background(&mut frame, scene.background).or_fail()?;
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(0), PixelRow(0))),
        Some(from),
        "gradient start edge must be from"
    );
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(7), PixelRow(0))),
        Some(to),
        "gradient far edge must be to"
    );
    let middle = frame.pixel(PixelCoord::new(PixelColumn(3), PixelRow(0)));
    ensure!(
        middle.is_some() && middle != Some(from) && middle != Some(to),
        "gradient interior must lie strictly between its edges"
    );
    Ok(())
}

#[test]
fn blobs_deterministic() {
    assert_eq!(check_blobs_deterministic(), Ok(()));
}

fn check_blobs_deterministic() -> Result<(), Fail> {
    let blobs = Background::Blobs {
        seed: Seed::new(1234),
        count: BlobCount(9),
        radii: BlobRadii::spanning([BlobRadius(1), BlobRadius(4)]),
    };
    let scene = bare_scene(blobs)?;
    let mut once = Frame::zeroed(scene.dimensions).or_fail()?;
    let mut twice = Frame::zeroed(scene.dimensions).or_fail()?;
    paint_background(&mut once, scene.background).or_fail()?;
    paint_background(&mut twice, scene.background).or_fail()?;
    ensure_eq!(
        once.data(),
        twice.data(),
        "blob layout from one seed must paint identically twice"
    );
    let mut ground_only = Frame::zeroed(scene.dimensions).or_fail()?;
    paint_background(
        &mut ground_only,
        Background::Blobs {
            seed: Seed::new(1234),
            count: BlobCount(0),
            radii: BlobRadii::spanning([BlobRadius(1), BlobRadius(4)]),
        },
    )
    .or_fail()?;
    ensure!(
        ground_only.data() != once.data(),
        "a zero-count blob field must differ from a populated one"
    );
    Ok(())
}
