use super::*;
use crate::color::{Channel, Rgb8};
use crate::frame::{PixelColumn, PixelCoord, PixelRow};
use crate::ratio::Numerator;
use crate::scene::{
    Background, BlobCount, BlobRadii, BlobRadius, FrameSpan, Motion, Object, Shape,
};
use crate::testutil::Frac;
use crate::units::{Height, Seed, Width};
use crate::view::{Rotation, Zoom};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// Builds a view that follows `origin` with no rotation and unit magnification.
fn still_view_at(origin: Point) -> Result<View, Fail> {
    let angle = Turns::new(Ratio::from_integer(Numerator(0)));
    let unit = Zoom::Fixed(Magnification::new(Ratio::from_integer(Numerator(1))).or_fail()?);
    Ok(View::new(
        Motion::Fixed(origin),
        Rotation::Fixed(angle),
        unit,
    ))
}

/// Builds a view fixed at the scene origin with unit magnification, which is the identity placement.
fn identity_view() -> Result<View, Fail> {
    let zero = Ratio::from_integer(Numerator(0));
    still_view_at(Point {
        across: zero,
        down: zero,
    })
}

/// Builds the 8x8 frame dimensions that the small scenes are rendered at.
fn small_dimensions() -> Result<Dimensions, Fail> {
    let width = Width::new(8).or_fail()?;
    let height = Height::new(8).or_fail()?;
    Ok(Dimensions::new(width, height))
}

/// Builds a scene with the given backdrop and objects.
fn small_scene(background: Background, objects: Vec<Object>) -> Scene {
    Scene {
        scale: None,
        background,
        objects,
    }
}

/// Builds the visibility span covering all four frames of [`small_scene`].
fn whole_span() -> Result<FrameSpan, Fail> {
    FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(4)).or_fail()
}

/// Builds a disc object fixed at `centre` with the given radius.
fn fixed_disc(centre: Point, radius: Ratio, fill: Rgb8, span: FrameSpan) -> Object {
    Object::new(Shape::Disc { radius }, fill, Motion::Fixed(centre), span)
}

#[test]
fn render_frame_twice_byte_identical() {
    assert_eq!(check_render_frame_twice_byte_identical(), Ok(()));
}

fn check_render_frame_twice_byte_identical() -> Result<(), Fail> {
    let red = Rgb8 {
        red: Channel(200),
        green: Channel(40),
        blue: Channel(40),
    };
    let green = Rgb8 {
        red: Channel(20),
        green: Channel(180),
        blue: Channel(60),
    };
    let span = whole_span()?;
    let three = Ratio::from_integer(Numerator(3));
    let six = Ratio::from_integer(Numerator(6));
    let first = fixed_disc(
        Point {
            across: three,
            down: three,
        },
        Ratio::from_integer(Numerator(2)),
        red,
        span,
    );
    let second = fixed_disc(
        Point {
            across: six,
            down: six,
        },
        three,
        green,
        span,
    );
    let seed_bg = Background::Blobs {
        seed: Seed::new(9),
        count: BlobCount(6),
        radii: BlobRadii::spanning([BlobRadius(1), BlobRadius(3)]),
    };
    let scene = small_scene(seed_bg, vec![first, second]);
    for frame_number in 0..4_u32 {
        let frame = FrameIndex::new(frame_number);
        let once = render_frame(&scene, &identity_view()?, small_dimensions()?, frame).or_fail()?;
        let twice =
            render_frame(&scene, &identity_view()?, small_dimensions()?, frame).or_fail()?;
        ensure_eq!(
            once.data(),
            twice.data(),
            "rendering frame {frame_number} twice must be byte-identical"
        );
    }
    Ok(())
}

#[test]
fn render_into_dirty_buffer_equals_fresh() {
    assert_eq!(check_render_into_dirty_buffer_equals_fresh(), Ok(()));
}

fn check_render_into_dirty_buffer_equals_fresh() -> Result<(), Fail> {
    let blue = Rgb8 {
        red: Channel(30),
        green: Channel(60),
        blue: Channel(220),
    };
    let yellow = Rgb8 {
        red: Channel(230),
        green: Channel(210),
        blue: Channel(40),
    };
    let span = whole_span()?;
    let four = Ratio::from_integer(Numerator(4));
    let object = fixed_disc(
        Point {
            across: four,
            down: four,
        },
        Ratio::from_integer(Numerator(3)),
        yellow,
        span,
    );
    let scene = small_scene(Background::Solid(blue), vec![object]);
    let frame = FrameIndex::new(1);
    let view = identity_view()?;
    let fresh = render_frame(&scene, &view, small_dimensions()?, frame).or_fail()?;
    let mut dirty = Frame::zeroed(small_dimensions()?).or_fail()?;
    dirty.fill(Rgb8 {
        red: Channel(255),
        green: Channel(0),
        blue: Channel(255),
    });
    ensure_eq!(
        render_into(&scene, &view, frame, &mut dirty),
        Ok(()),
        "rendering into a matching buffer must succeed"
    );
    ensure_eq!(
        dirty.data(),
        fresh.data(),
        "rendering into a dirty buffer must equal a fresh render"
    );
    Ok(())
}

#[test]
fn empty_scene_equals_hidden_objects() {
    assert_eq!(check_empty_scene_equals_hidden_objects(), Ok(()));
}

fn check_empty_scene_equals_hidden_objects() -> Result<(), Fail> {
    let ground = Background::Solid(Rgb8 {
        red: Channel(12),
        green: Channel(34),
        blue: Channel(56),
    });
    let empty = small_scene(ground, Vec::new());
    let hidden_span = FrameSpan::new(FrameIndex::new(2)..FrameIndex::new(3)).or_fail()?;
    let four = Ratio::from_integer(Numerator(4));
    let object = fixed_disc(
        Point {
            across: four,
            down: four,
        },
        Ratio::from_integer(Numerator(3)),
        Rgb8 {
            red: Channel(9),
            green: Channel(9),
            blue: Channel(9),
        },
        hidden_span,
    );
    let with_hidden = small_scene(ground, vec![object]);
    let view = identity_view()?;
    let dimensions = small_dimensions()?;
    let background_only = render_frame(&empty, &view, dimensions, FrameIndex::new(0)).or_fail()?;
    let with_objects =
        render_frame(&with_hidden, &view, dimensions, FrameIndex::new(0)).or_fail()?;
    ensure_eq!(
        background_only.data(),
        with_objects.data(),
        "an object outside its span must draw nothing"
    );
    let shown = render_frame(&with_hidden, &view, dimensions, FrameIndex::new(2)).or_fail()?;
    ensure!(
        shown.data() != background_only.data(),
        "an object inside its span must change the frame"
    );
    Ok(())
}

#[test]
fn declaration_order_last_wins() {
    assert_eq!(check_declaration_order_last_wins(), Ok(()));
}

fn check_declaration_order_last_wins() -> Result<(), Fail> {
    let black = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    };
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
    let span = whole_span()?;
    let four = Ratio::from_integer(Numerator(4));
    let three = Ratio::from_integer(Numerator(3));
    let under = fixed_disc(
        Point {
            across: four,
            down: four,
        },
        three,
        red,
        span,
    );
    let over = fixed_disc(
        Point {
            across: four,
            down: four,
        },
        three,
        blue,
        span,
    );
    let first_wins = small_scene(Background::Solid(black), vec![over.clone(), under.clone()]);
    let second_wins = small_scene(Background::Solid(black), vec![under, over]);
    let view = identity_view()?;
    let dimensions = small_dimensions()?;
    let first = render_frame(&first_wins, &view, dimensions, FrameIndex::new(0)).or_fail()?;
    let second = render_frame(&second_wins, &view, dimensions, FrameIndex::new(0)).or_fail()?;
    ensure_eq!(
        first.pixel(PixelCoord::new(PixelColumn(4), PixelRow(4))),
        Some(red),
        "the later declaration must win the shared pixel"
    );
    ensure_eq!(
        second.pixel(PixelCoord::new(PixelColumn(4), PixelRow(4))),
        Some(blue),
        "swapping the order must swap the winning colour"
    );
    Ok(())
}

#[test]
fn view_overflow_names_frame() {
    assert_eq!(check_view_overflow_names_frame(), Ok(()));
}

fn check_view_overflow_names_frame() -> Result<(), Fail> {
    let scene = small_scene(
        Background::Solid(Rgb8 {
            red: Channel(0),
            green: Channel(0),
            blue: Channel(0),
        }),
        Vec::new(),
    );
    let max = Ratio::from_integer(Numerator(i64::MAX));
    let one = Ratio::from_integer(Numerator(1));
    let zero = Ratio::from_integer(Numerator(0));
    let unit = Magnification::new(one).or_fail()?;
    let view = View::new(
        Motion::Fixed(Point {
            across: one,
            down: zero,
        }),
        Rotation::Linear {
            start: Turns::new(max),
            per_frame: Turns::new(max),
        },
        Zoom::Fixed(unit),
    );
    let result = render_frame(&scene, &view, small_dimensions()?, FrameIndex::new(1));
    ensure_eq!(
        result,
        Err(RenderError::ViewOverflow {
            frame: FrameIndex::new(1)
        }),
        "a view placement overflow must name the frame"
    );
    Ok(())
}

#[test]
fn object_overflow_names_object_index() {
    assert_eq!(check_object_overflow_names_object_index(), Ok(()));
}

fn check_object_overflow_names_object_index() -> Result<(), Fail> {
    let span = whole_span()?;
    let unit = Ratio::from_integer(Numerator(1));
    let valid = fixed_disc(
        Point {
            across: unit,
            down: unit,
        },
        unit,
        Rgb8 {
            red: Channel(200),
            green: Channel(40),
            blue: Channel(40),
        },
        span,
    );
    let max = Ratio::from_integer(Numerator(i64::MAX));
    let zero = Ratio::from_integer(Numerator(0));
    let one = Ratio::from_integer(Numerator(1));
    let overflow = Object::new(
        Shape::Rect {
            half_width: one,
            half_height: one,
        },
        Rgb8 {
            red: Channel(20),
            green: Channel(180),
            blue: Channel(60),
        },
        Motion::Fixed(Point {
            across: max,
            down: zero,
        }),
        span,
    );
    let overflowing = small_scene(
        Background::Solid(Rgb8 {
            red: Channel(0),
            green: Channel(0),
            blue: Channel(0),
        }),
        vec![valid, overflow],
    );
    let result = render_frame(
        &overflowing,
        &identity_view()?,
        small_dimensions()?,
        FrameIndex::new(0),
    );
    ensure_eq!(
        result,
        Err(RenderError::ObjectOverflow {
            frame: FrameIndex::new(0),
            object: ObjectIndex::new(1)
        }),
        "an object placement overflow must name the frame and object index"
    );
    Ok(())
}

#[test]
fn off_screen_shape_renders_successfully() {
    assert_eq!(check_off_screen_shape_renders_successfully(), Ok(()));
}

fn check_off_screen_shape_renders_successfully() -> Result<(), Fail> {
    let span = whole_span()?;
    let hundred = Ratio::from_integer(Numerator(100));
    let off = fixed_disc(
        Point {
            across: hundred,
            down: hundred,
        },
        Ratio::from_integer(Numerator(1)),
        Rgb8 {
            red: Channel(255),
            green: Channel(0),
            blue: Channel(0),
        },
        span,
    );
    let scene = small_scene(
        Background::Solid(Rgb8 {
            red: Channel(0),
            green: Channel(0),
            blue: Channel(0),
        }),
        vec![off],
    );
    let frame = render_frame(
        &scene,
        &identity_view()?,
        small_dimensions()?,
        FrameIndex::new(0),
    )
    .or_fail()?;
    let expected = Frame::from_color(
        small_dimensions()?,
        Rgb8 {
            red: Channel(0),
            green: Channel(0),
            blue: Channel(0),
        },
    )
    .or_fail()?;
    ensure_eq!(
        frame.data(),
        expected.data(),
        "a shape entirely outside the frame must draw nothing"
    );
    Ok(())
}

#[test]
fn no_objects_equals_background() {
    assert_eq!(check_no_objects_equals_background(), Ok(()));
}

fn check_no_objects_equals_background() -> Result<(), Fail> {
    let scene = small_scene(
        Background::Solid(Rgb8 {
            red: Channel(12),
            green: Channel(34),
            blue: Channel(56),
        }),
        Vec::new(),
    );
    let frame = render_frame(
        &scene,
        &identity_view()?,
        small_dimensions()?,
        FrameIndex::new(0),
    )
    .or_fail()?;
    let expected = Frame::from_color(
        small_dimensions()?,
        Rgb8 {
            red: Channel(12),
            green: Channel(34),
            blue: Channel(56),
        },
    )
    .or_fail()?;
    ensure_eq!(
        frame.data(),
        expected.data(),
        "a scene with no objects must equal its background alone"
    );
    Ok(())
}

#[test]
fn identity_view_maps_scene_to_pixels() {
    assert_eq!(check_identity_view_maps_scene_to_pixels(), Ok(()));
}

fn check_identity_view_maps_scene_to_pixels() -> Result<(), Fail> {
    let black = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    };
    let red = Rgb8 {
        red: Channel(255),
        green: Channel(0),
        blue: Channel(0),
    };
    let span = whole_span()?;
    let cx = Frac(5, 2).ratio()?;
    let cy = Frac(5, 2).ratio()?;
    let rr = Ratio::from_integer(Numerator(1));
    let object = Object::new(
        Shape::Disc { radius: rr },
        red,
        Motion::Fixed(Point {
            across: cx,
            down: cy,
        }),
        span,
    );
    let scene = small_scene(Background::Solid(black), vec![object]);
    let frame = render_frame(
        &scene,
        &identity_view()?,
        small_dimensions()?,
        FrameIndex::new(0),
    )
    .or_fail()?;
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(2), PixelRow(2))),
        Some(red),
        "a disc at (2.5, 2.5) must cover pixel (2, 2)"
    );
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(0), PixelRow(0))),
        Some(black),
        "a disc at (2.5, 2.5) must not reach pixel (0, 0)"
    );
    Ok(())
}

#[test]
fn view_translation_shifts_objects() {
    assert_eq!(check_view_translation_shifts_objects(), Ok(()));
}

fn check_view_translation_shifts_objects() -> Result<(), Fail> {
    let black = Rgb8 {
        red: Channel(0),
        green: Channel(0),
        blue: Channel(0),
    };
    let red = Rgb8 {
        red: Channel(255),
        green: Channel(0),
        blue: Channel(0),
    };
    let span = whole_span()?;
    let half = Frac(5, 2).ratio()?;
    let radius = Ratio::from_integer(Numerator(1));
    let object = Object::new(
        Shape::Disc { radius },
        red,
        Motion::Fixed(Point {
            across: half,
            down: half,
        }),
        span,
    );
    let two = Ratio::from_integer(Numerator(2));
    let zero = Ratio::from_integer(Numerator(0));
    let one = Ratio::from_integer(Numerator(1));
    let unit = Magnification::new(one).or_fail()?;
    let shifted_view = View::new(
        Motion::Fixed(Point {
            across: two,
            down: zero,
        }),
        Rotation::Fixed(Turns::new(zero)),
        Zoom::Fixed(unit),
    );
    let scene = Scene {
        scale: None,
        background: Background::Solid(black),
        objects: vec![object],
    };
    let frame = render_frame(
        &scene,
        &shifted_view,
        small_dimensions()?,
        FrameIndex::new(0),
    )
    .or_fail()?;
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(0), PixelRow(2))),
        Some(red),
        "moving the view by (2, 0) must shift the disc left by two pixels"
    );
    ensure_eq!(
        frame.pixel(PixelCoord::new(PixelColumn(2), PixelRow(2))),
        Some(black),
        "the disc must vacate the pixel it covered before the shift"
    );
    Ok(())
}
