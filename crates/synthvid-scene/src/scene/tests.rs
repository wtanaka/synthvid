use super::*;
use crate::camera::{Magnification, Rotation, Turns, Zoom};
use crate::color::Channel;
use crate::ratio::Numerator;
use crate::testutil::Frac;
use crate::units::{Height, Width};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// Builds the fixed point at the origin for motion tests.
#[must_use]
fn origin_point() -> Point {
    let zero = Ratio::from_integer(Numerator(0));
    Point {
        across: zero,
        down: zero,
    }
}

/// Builds the identity camera: still at the origin with unit magnification.
fn fixed_camera() -> Result<Camera, Fail> {
    let origin = origin_point();
    let angle = Turns::new(Ratio::from_integer(Numerator(0)));
    let unit = Zoom::Fixed(Magnification::new(Ratio::from_integer(Numerator(1))).or_fail()?);
    Ok(Camera::new(
        Motion::Fixed(origin),
        Rotation::Fixed(angle),
        unit,
    ))
}

#[test]
fn scale_rejects_non_positive() {
    assert_eq!(check_scale_rejects_non_positive(), Ok(()));
}

fn check_scale_rejects_non_positive() -> Result<(), Fail> {
    let zero = Ratio::from_integer(Numerator(0));
    let neg = Frac(-3, 2).ratio()?;
    let pos = Frac(3, 2).ratio()?;
    ensure!(Scale::new(zero).is_err(), "zero scale must be rejected");
    ensure!(Scale::new(neg).is_err(), "negative scale must be rejected");
    let scale = Scale::new(pos).or_fail()?;
    ensure_eq!(scale.get(), pos, "positive scale must round-trip");
    ensure_eq!(scale.ratio(), pos, "scale ratio must round-trip");
    ensure_eq!(
        Scale::try_from(pos),
        Ok(scale),
        "TryFrom a positive ratio must succeed"
    );
    ensure_eq!(
        Scale::try_from(neg),
        Err(ScaleError::NonPositive),
        "TryFrom a negative ratio must fail"
    );
    ensure_eq!(
        Scale::from_ratio(pos),
        Ok(scale),
        "from_ratio must match new"
    );
    Ok(())
}

#[test]
fn frame_span_reports_the_bounds_it_was_built_with() {
    assert_eq!(
        check_frame_span_reports_the_bounds_it_was_built_with(),
        Ok(())
    );
}

fn check_frame_span_reports_the_bounds_it_was_built_with() -> Result<(), Fail> {
    let span = FrameSpan::new(FrameIndex::new(3)..FrameIndex::new(11)).or_fail()?;
    ensure_eq!(span.start(), FrameIndex::new(3));
    ensure_eq!(span.end(), FrameIndex::new(11));
    Ok(())
}

#[test]
fn frame_span_contains_and_rejects_inverted() {
    assert_eq!(check_frame_span_contains_and_rejects_inverted(), Ok(()));
}

fn check_frame_span_contains_and_rejects_inverted() -> Result<(), Fail> {
    let span = FrameSpan::new(FrameIndex::new(2)..FrameIndex::new(5)).or_fail()?;
    ensure_eq!(
        span.contains(FrameIndex::new(2)),
        Membership::Inside,
        "span must contain start"
    );
    ensure_eq!(
        span.contains(FrameIndex::new(4)),
        Membership::Inside,
        "span must contain end - 1"
    );
    ensure_eq!(
        span.contains(FrameIndex::new(5)),
        Membership::Outside,
        "span must exclude end"
    );
    ensure_eq!(
        span.contains(FrameIndex::new(1)),
        Membership::Outside,
        "span must exclude before start"
    );
    ensure!(
        FrameSpan::new(FrameIndex::new(5)..FrameIndex::new(2)).is_err(),
        "inverted span must be rejected"
    );
    ensure!(
        FrameSpan::new(FrameIndex::new(5)..FrameIndex::new(4)).is_err(),
        "inverted span must be rejected"
    );
    ensure!(
        FrameSpan::new(FrameIndex::new(5)..FrameIndex::new(5)).is_err(),
        "empty span must be rejected"
    );
    let unit = FrameSpan::new(FrameIndex::new(5)..FrameIndex::new(6)).or_fail()?;
    ensure_eq!(
        unit.contains(FrameIndex::new(5)),
        Membership::Inside,
        "unit span must contain start"
    );
    ensure_eq!(
        unit.contains(FrameIndex::new(6)),
        Membership::Outside,
        "unit span must exclude end"
    );
    Ok(())
}

#[test]
fn scene_clone_compares_equal() {
    assert_eq!(check_scene_clone_compares_equal(), Ok(()));
}

fn check_scene_clone_compares_equal() -> Result<(), Fail> {
    let width = Width::new(64).or_fail()?;
    let height = Height::new(48).or_fail()?;
    let dimensions = Dimensions::new(width, height);
    let rate = FrameRate::from_fps(30).or_fail()?;
    let count = FrameCount::new(4).or_fail()?;
    let unit = Ratio::from_integer(Numerator(2));
    let scale = Scale::new(unit).or_fail()?;
    let camera = fixed_camera()?;
    let origin = origin_point();
    let radius = Ratio::from_integer(Numerator(5));
    let span = FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(4)).or_fail()?;
    let red = Rgb8 {
        red: Channel(200),
        green: Channel(30),
        blue: Channel(30),
    };
    let disc = Object::new(Shape::Disc { radius }, red, Motion::Fixed(origin), span);
    let scene = Scene {
        dimensions,
        frame_rate: rate,
        frame_count: count,
        scale: Some(scale),
        background: Background::Solid(red),
        camera,
        objects: vec![disc],
    };
    let cloned = scene.clone();
    ensure_eq!(cloned, scene, "a scene must equal its clone");
    Ok(())
}

/// Builds the shared object list covering every shape variant.
fn every_shape_objects(green: Rgb8, span: FrameSpan) -> Vec<Object> {
    let origin = origin_point();
    let one = Ratio::from_integer(Numerator(1));
    let two = Ratio::from_integer(Numerator(2));
    let drift = Motion::Linear {
        origin,
        velocity: Vector {
            across: one,
            down: one,
        },
    };
    let shapes = [
        Shape::Disc { radius: two },
        Shape::Rect {
            half_width: two,
            half_height: one,
        },
        Shape::Polygon {
            vertices: vec![
                origin,
                Point {
                    across: one,
                    down: origin.down,
                },
                Point {
                    across: one,
                    down: one,
                },
            ],
        },
        Shape::Cross {
            arm: two,
            thickness: one,
        },
    ];
    let mut objects = Vec::new();
    for shape in shapes {
        objects.push(Object::new(shape, green, drift.clone(), span));
    }
    objects
}

/// Builds every background variant in declaration order.
fn every_background() -> Result<[Background; 6], Fail> {
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
    let cell = NonZeroU16::new(4).or_fail()?;
    let spacing = NonZeroU16::new(8).or_fail()?;
    Ok([
        Background::Solid(red),
        Background::Checker {
            cell,
            even: red,
            odd: blue,
        },
        Background::Gradient {
            from: red,
            to: blue,
            direction: Direction::Horizontal,
        },
        Background::Gradient {
            from: black,
            to: white,
            direction: Direction::Vertical,
        },
        Background::Blobs {
            seed: Seed::new(7),
            count: BlobCount(5),
            radii: BlobRadii::spanning([BlobRadius(1), BlobRadius(4)]),
        },
        Background::Grid {
            spacing,
            line: white,
            ground: black,
        },
    ])
}

/// Builds a small scene with the given backdrop and objects.
fn small_scene(background: Background, objects: Vec<Object>) -> Result<Scene, Fail> {
    let width = Width::new(32).or_fail()?;
    let height = Height::new(32).or_fail()?;
    let dimensions = Dimensions::new(width, height);
    let rate = FrameRate::from_fps(60).or_fail()?;
    let count = FrameCount::new(8).or_fail()?;
    let camera = fixed_camera()?;
    Ok(Scene {
        dimensions,
        frame_rate: rate,
        frame_count: count,
        scale: None,
        background,
        camera,
        objects,
    })
}

#[test]
fn scene_backgrounds_round_trip() {
    assert_eq!(check_scene_backgrounds_round_trip(), Ok(()));
}

fn check_scene_backgrounds_round_trip() -> Result<(), Fail> {
    let backgrounds = every_background()?;
    let span = FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(8)).or_fail()?;
    let green = Rgb8 {
        red: Channel(0),
        green: Channel(255),
        blue: Channel(0),
    };
    let objects = every_shape_objects(green, span);
    for background in backgrounds {
        let scene = small_scene(background, objects.clone())?;
        let cloned = scene.clone();
        ensure_eq!(
            cloned,
            scene,
            "every background variant must round-trip through clone"
        );
    }
    Ok(())
}

#[test]
fn scene_shapes_round_trip() {
    assert_eq!(check_scene_shapes_round_trip(), Ok(()));
}

fn check_scene_shapes_round_trip() -> Result<(), Fail> {
    let span = FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(8)).or_fail()?;
    let green = Rgb8 {
        red: Channel(0),
        green: Channel(255),
        blue: Channel(0),
    };
    let objects = every_shape_objects(green, span);
    let slice = objects.get(0..4).or_fail()?;
    ensure_eq!(slice.len(), 4, "one object per shape variant must exist");
    let backgrounds = every_background()?;
    let first = backgrounds[0];
    let scene = small_scene(first, objects)?;
    let cloned = scene.clone();
    ensure_eq!(
        cloned,
        scene,
        "every shape variant must round-trip through clone"
    );
    Ok(())
}

#[test]
fn scene_inequality() {
    assert_eq!(check_scene_inequality(), Ok(()));
}

fn check_scene_inequality() -> Result<(), Fail> {
    let backgrounds = every_background()?;
    let first = backgrounds[0];
    let span = FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(8)).or_fail()?;
    let green = Rgb8 {
        red: Channel(0),
        green: Channel(255),
        blue: Channel(0),
    };
    let red = Rgb8 {
        red: Channel(255),
        green: Channel(0),
        blue: Channel(0),
    };
    let objects = every_shape_objects(green, span);
    let scene = small_scene(first, objects)?;
    let mut altered = scene.clone();
    altered.background = Background::Solid(green);
    ensure!(
        altered != scene,
        "changing the background must compare unequal"
    );
    let one = Ratio::from_integer(Numerator(1));
    let two = Ratio::from_integer(Numerator(2));
    let origin = origin_point();
    let still = Motion::Fixed(origin);
    let still_object = Object::new(Shape::Disc { radius: one }, red, still, span);
    ensure!(
        still_object.shape != Shape::Disc { radius: two },
        "distinct shapes must compare unequal"
    );
    Ok(())
}

#[test]
fn motion_variants_clone_equal() {
    assert_eq!(check_motion_variants_clone_equal(), Ok(()));
}

fn check_motion_variants_clone_equal() -> Result<(), Fail> {
    let origin = origin_point();
    let one = Ratio::from_integer(Numerator(1));
    let quarter = Frac(1, 4).ratio()?;
    let count = FrameCount::new(24).or_fail()?;
    let vector = Vector {
        across: one,
        down: one,
    };
    let motions = vec![
        Motion::Fixed(origin),
        Motion::Linear {
            origin,
            velocity: vector,
        },
        Motion::Ballistic {
            origin,
            velocity: vector,
            acceleration: vector,
        },
        Motion::Circular {
            centre: origin,
            radius: one,
            turns_per_frame: quarter,
            phase: one,
        },
        Motion::Oscillating {
            origin,
            amplitude: vector,
            period: count,
            phase: quarter,
        },
        Motion::Walk {
            origin,
            step: one,
            seed: Seed::new(11),
        },
        Motion::Piecewise(vec![(FrameIndex::new(0), Motion::Fixed(origin))]),
    ];
    for motion in motions {
        let cloned = motion.clone();
        ensure_eq!(cloned, motion, "every motion variant must equal its clone");
    }
    Ok(())
}
