use super::*;
use crate::optics::BladeCount;
use core::num::NonZeroI64;
use synthvid_scene::{LocalPoint, Outline};
use synthvid_testkit::ensure_eq;

// Notation used by every derivation below, as in the lens model of `deep::lens`: `H` is the frame
// height of the iris in world units, `h` is the output height in sensels, and `fn` is the focal
// length in sixteenths of a frame height. The focal length is `f = fn * H / 16` world units.

/// Five sensels across and three sensels down.
fn five_by_three() -> Result<Dimensions, Fail> {
    Ok(Dimensions::new(
        Width::new(5).or_fail()?,
        Height::new(3).or_fail()?,
    ))
}

/// A recorder of five by three sensels with one picture at 30 frames per second.
fn five_by_three_recorder() -> Result<Recorder, Fail> {
    let timing = Timing::new(
        FrameRate::from_fraction(30, NonZeroU32::MIN).or_fail()?,
        FrameCount::new(1).or_fail()?,
    );
    Ok(Recorder::new(Medium::Off, five_by_three()?, timing))
}

/// A lens with a focal length of 16 sixteenths of a frame height, and a five-bladed iris of f/2
/// on a frame height of `H = 12` world units, focused at `s = 96` world units and sampled on a four
/// by four grid. The focal length is `f = 12` world units, so the focus is eight focal lengths away,
/// which is the closest focus the rig accepts: `2 * s = fn * H = 192`.
fn five_blade_lens() -> Result<Lens, Fail> {
    let blades = BladeCount::new(NonZeroU8::new(5).or_fail()?).or_fail()?;
    let distance = FocusDistance::new(NonZeroU32::new(96).or_fail()?).or_fail()?;
    let iris = Iris::new(
        FStop::F2,
        ApertureShape::Polygon(blades),
        Focus::At(distance),
        FrameHeight(NonZeroU16::new(12).or_fail()?),
        ApertureGrid::Four,
    );
    Ok(Lens::new(
        FocalLength(NonZeroU8::new(16).or_fail()?),
        Aperture::Iris(iris),
        Vignette::None,
    ))
}

/// A camera at the origin, rolled by one eighth of a turn.
fn eighth_turn_roll_pose() -> Result<Pose, Fail> {
    let eighth = Ratio::new(Numerator(1), NonZeroI64::new(8).or_fail()?).or_fail()?;
    Ok(Pose::new(
        fixed(on_axis(RawCoord(0)))?,
        Rotation::Fixed(Turns::new(eighth)),
    ))
}

/// A white square of half size two, centered one unit left and one unit up of the camera axis,
/// at depth six. The backdrop of the shot is black.
fn negative_square() -> Result<Layer, Fail> {
    let center = Spot {
        across: RawCoord(-1),
        down: RawCoord(-1),
        depth: RawCoord(6),
    };
    Ok(Layer {
        cutout: square(NonZeroU32::new(2).or_fail()?)?,
        fill: white(),
        track: fixed(center)?,
        visible: FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(1)).or_fail()?,
    })
}

/// The shot of the square, seen through the five-bladed iris on the rolled camera, with
/// two samples along each axis of every sensel.
fn awkward_shot() -> Result<DeepShot, Fail> {
    let pose = eighth_turn_roll_pose()?;
    let lens = five_blade_lens()?;
    let recorder = five_by_three_recorder()?;
    let rig = Rig::new(&pose, lens, &recorder);
    let stage = Stage {
        backdrop: black(),
        layers: vec![negative_square()?],
    };
    DeepShot::new(stage, &rig, PixelGrid::Two).or_fail()
}

/// Renders the awkward shot at frame zero into a five by three frame.
fn render_awkward() -> Result<Frame, Fail> {
    let scene = awkward_shot()?;
    let mut out = Frame::zeroed(five_by_three()?).or_fail()?;
    render_stage_into(&scene, FrameIndex::new(0), &mut out).or_fail()?;
    Ok(out)
}

/// The expected gray level of each sensel, indexed as `[row][column]`.
///
/// Derivation. The camera has focal length 16/16 of the iris frame height, and the output is 3
/// sensels high, so a point at world offset `(x, y)` from the axis, at depth `z`, lands
/// `256 * 3 * (x, y) / z` lattice units from the frame center, which is at `(640, 384)`. Since the
/// roll is 1/8 turn, the trigonometry gives cosine `c = 23171/32768` and sine `-c`, which is `P(1/2)`
/// of the degree-7 polynomial in `scene::trig`: `15708/2 - 6459/8 + 794/32 - 43/128`, times 32768,
/// is `23171.08`. The rotated offset is `(c(x + y), c(y - x))`, so a projected coordinate is
/// `round(23171 * k / 256)` for the integer `k` of the offset, and `round(90.51 * k)` on each axis.
///
/// The square has half size 2 and center `(-1, -1, 6)`. Its corners are `(x, y)` = `(-3, -3)`,
/// `(1, -3)`, `(1, 1)`, `(-3, 1)`, which give `k` = `(-6, 0)`, `(-2, -4)`, `(2, 0)`, `(-2, 4)`. The
/// projected corners are `(-543, 0)`, `(-181, -362)`, `(181, 0)`, `(-181, 362)` after rounding,
/// so the lattice corners are `(97, 384)`, `(459, 22)`, `(821, 384)`, `(459, 746)`. That is the
/// outline `|X - 459| + |Y - 384| <= 362`, with its boundary inside.
///
/// The five-bladed iris has vertices at `k/5` turns. Its 4 by 4 candidates have numerators
/// `g` in `{-3, -1, 1, 3}`, at unit coordinates `g/4`. A candidate is kept when it and its
/// reflection through the center both lie inside the pentagon, whose edges have apothem
/// `cos 36 deg = 0.809`. A candidate with both numerators of magnitude 3 projects to 1.05 onto an
/// edge normal, past the apothem, so it is dropped together with its reflection. Every other
/// candidate projects to at most 0.79 onto every normal, so 12 lens points are kept, and the kept
/// set is symmetric under negation.
///
/// The focus is at `s = 96` and the layer at `z = 6`. A lens point with numerators `(g_x, g_y)`
/// shifts the layer by `H * h * fn^2 * Nd * g * (s - z) / (2 * Nn * A * z * s)` on each axis, with
/// `H = 12`, `h = 3`, `fn = 16`, `Nd = 1`, `Nn = 2`, `A = 4`. That is `9216 * 90 * g / 9216 = 90 g`
/// lattice units, so each axis is shifted by `+-90` or `+-270`. The kept set is symmetric under
/// negation, so its shifts are the same set whatever the sign convention.
///
/// A sensel has two samples per axis, at offsets `64` and `192` lattice units from its edge, so
/// it has `4 * 12 = 48` pairs of sample and lens point. A pair is covered when its shifted point
/// lies in the outline. Each covered pair contributes 255 to a channel and each uncovered pair
/// contributes 0, so with `m` covered pairs the sum is `255 * m`, and the average is
/// `floor((2 * 255 m + 48) / 96)`.
///
/// Counting `m` for each sensel from the outline and the shifts gives, row by row:
/// `[9, 20, 15, 5, 0]`, `[14, 28, 28, 8, 0]`, and `[9, 20, 15, 5, 0]`. The third row is the first
/// one mirrored, because the outline and the kept set are symmetric about the horizontal axis
/// through the frame center. The averages of `m` = 0, 5, 8, 9, 14, 15, 20, 28 are 0, 27, 43, 48,
/// 74, 80, 106, and 149.
const EXPECTED: [[u8; 5]; 3] = [
    [48, 106, 80, 27, 0],
    [74, 149, 149, 43, 0],
    [48, 106, 80, 27, 0],
];

/// The rolled, out-of-focus square through the five-bladed iris matches the hand derivation in
/// every sensel.
#[test]
fn rolled_square_through_five_blade_iris_matches_hand_derivation() {
    assert_eq!(check_rolled_square_matches_hand_derivation(), Ok(()));
}

fn check_rolled_square_matches_hand_derivation() -> Result<(), Fail> {
    let frame = render_awkward()?;
    for (row, values) in EXPECTED.iter().enumerate() {
        for (column, level) in values.iter().enumerate() {
            let cell = PixelCoord::new(
                PixelColumn(u16::try_from(column).or_fail()?),
                PixelRow(u16::try_from(row).or_fail()?),
            );
            let found = frame.get_pixel(cell).or_fail()?;
            ensure_eq!(found, gray(Channel(*level)));
        }
    }
    Ok(())
}

/// Two sensels across and two down.
fn two_by_two() -> Result<Dimensions, Fail> {
    Ok(Dimensions::new(
        Width::new(2).or_fail()?,
        Height::new(2).or_fail()?,
    ))
}

/// A recorder of two by two sensels with one picture at 30 frames per second.
fn two_by_two_recorder() -> Result<Recorder, Fail> {
    let timing = Timing::new(
        FrameRate::from_fraction(30, NonZeroU32::MIN).or_fail()?,
        FrameCount::new(1).or_fail()?,
    );
    Ok(Recorder::new(Medium::Off, two_by_two()?, timing))
}

/// A lens with a focal length of 16 sixteenths of a frame height, and a disc iris of f/2 focused at
/// infinity on an iris frame height of `H = 8` world units, sampled on a two by two grid.
fn disc_lens() -> Result<Lens, Fail> {
    let iris = Iris::new(
        FStop::F2,
        ApertureShape::Disc,
        Focus::Infinity,
        FrameHeight(NonZeroU16::new(8).or_fail()?),
        ApertureGrid::Two,
    );
    Ok(Lens::new(
        FocalLength(NonZeroU8::new(16).or_fail()?),
        Aperture::Iris(iris),
        Vignette::None,
    ))
}

/// A position within a layer's plane, named by its two components so that they cannot be
/// transposed.
#[derive(Copy, Clone, Debug)]
struct Offset {
    /// The component to the right of the layer's position.
    across: RawCoord,
    /// The component below the layer's position.
    down: RawCoord,
}

/// The vertex of a layer at the given offset.
fn vertex(offset: Offset) -> Result<LocalPoint, Fail> {
    Ok(LocalPoint {
        across: Coord::new(offset.across).or_fail()?,
        down: Coord::new(offset.down).or_fail()?,
    })
}

/// Where a strip layer sits: its depth and the world coordinate of its left edge.
#[derive(Copy, Clone, Debug)]
struct StripPlacement {
    /// The depth of the layer.
    depth: RawCoord,
    /// The across coordinate of the left edge, left of the layer position.
    left: RawCoord,
}

/// A white strip at the given depth, spanning from `left` to zero across and from zero to two
/// down, in world units. The backdrop of the shot is black.
fn strip_layer_at(placement: StripPlacement) -> Result<Layer, Fail> {
    let StripPlacement { depth, left } = placement;
    let outline = Outline::new(
        [
            vertex(Offset {
                across: left,
                down: RawCoord(0),
            })?,
            vertex(Offset {
                across: RawCoord(0),
                down: RawCoord(0),
            })?,
            vertex(Offset {
                across: RawCoord(0),
                down: RawCoord(2),
            })?,
        ],
        vec![vertex(Offset {
            across: left,
            down: RawCoord(2),
        })?],
    );
    Ok(Layer {
        cutout: Cutout::Polygon(outline),
        fill: white(),
        track: fixed(on_axis(depth))?,
        visible: FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(1)).or_fail()?,
    })
}

/// The strip at depth 4 with a width of one world unit, which is the layer of the infinite-focus
/// test below.
fn strip_layer() -> Result<Layer, Fail> {
    strip_layer_at(StripPlacement {
        depth: RawCoord(4),
        left: RawCoord(-1),
    })
}

/// The shot of a layer seen through the given lens by an upright camera at the origin, with two
/// samples along each axis of every sensel.
fn strip_shot_through(lens: Lens, layer: Layer) -> Result<DeepShot, Fail> {
    let pose = Pose::new(fixed(on_axis(RawCoord(0)))?, upright());
    let recorder = two_by_two_recorder()?;
    let rig = Rig::new(&pose, lens, &recorder);
    let stage = Stage {
        backdrop: black(),
        layers: vec![layer],
    };
    DeepShot::new(stage, &rig, PixelGrid::Two).or_fail()
}

/// Renders the given layer through the given lens at frame zero into a two by two frame.
fn render_strip_through(lens: Lens, layer: Layer) -> Result<Frame, Fail> {
    let scene = strip_shot_through(lens, layer)?;
    let mut out = Frame::zeroed(two_by_two()?).or_fail()?;
    render_stage_into(&scene, FrameIndex::new(0), &mut out).or_fail()?;
    Ok(out)
}

/// Renders the strip shot at frame zero into a two by two frame.
fn render_strip() -> Result<Frame, Fail> {
    render_strip_through(disc_lens()?, strip_layer()?)
}

/// The expected gray level of each sensel of the strip shot, indexed as `[row][column]`.
///
/// Derivation. The camera is upright at the origin, and the output is 2 sensels high, so a point
/// at world offset `(x, y)` at depth 4 lands at `(256 + 16 * 2 * 16 * x / 4, 256 + 16 * 2 * 16 * y / 4)`,
/// that is `(256 + 128 x, 256 + 128 y)` in lattice units. The polygon therefore covers the lattice
/// rectangle `[128, 256] x [256, 512]`. Its corners and edges sit on multiples of 128, and no sample
/// offset below is a multiple of 128, so no sample lies on an edge.
///
/// The disc has radius 1 in unit coordinates. On the two by two grid its candidates are
/// `(+-1/2, +-1/2)`, each at squared distance `1/2` from the center, so all four lie inside, and
/// each one's reflection lies inside too. The lens has four points.
///
/// With focus at infinity the shift along each axis is `-H h fn^2 Nd g / (2 Nn A z)`, which is
/// `-(8 * 2 * 256 * 1 * g) / (2 * 2 * 2 * 4) = -128 g`. The four points therefore shift the image
/// by each of `(+-128, +-128)` once.
///
/// Each sensel has 4 samples and 4 lens points, so 16 pairs, and the sample offsets are `64` and
/// `192` within a sensel. The sample columns are at `x = 64, 192, 320, 448`, and the sample rows
/// are at `y = 64, 192, 320, 448`. A pair is covered when the sample minus its shift lies in the
/// rectangle.
///
/// For the shift `(+128, +128)`, `x - 128` lies in `[128, 256]` only for `x = 320`, and `y - 128`
/// lies in `[256, 512]` only for `y = 448`. That covers the pair `(320, 448)`, in sensel `(1, 1)`.
/// For `(+128, -128)`, `x = 320` again, and `y + 128` lies in `[256, 512]` for `y = 192` and
/// `y = 320`. That covers `(320, 192)` in sensel `(1, 0)` and `(320, 320)` in sensel `(1, 1)`.
/// For `(-128, +128)`, `x + 128` lies in `[128, 256]` only for `x = 64`, and `y - 128` only for
/// `y = 448`, which covers `(64, 448)` in sensel `(0, 1)`. For `(-128, -128)`, `x = 64` and
/// `y + 128` lies in `[256, 512]` for `y = 192` and `y = 320`, which covers `(64, 192)` in sensel
/// `(0, 0)` and `(64, 320)` in sensel `(0, 1)`.
///
/// Sensel `(0, 0)` and sensel `(1, 0)` have one covered pair each, so their sums are 255 and their
/// averages are `floor((2 * 255 + 16) / 32) = 16`. Sensels `(0, 1)` and `(1, 1)` have two each, so
/// their averages are `floor((2 * 510 + 16) / 32) = 32`.
///
/// The kept points are symmetric under negation, so the four shifts are the same set whatever the
/// sign of the shift, and this frame cannot detect a sign error in the shift.
const STRIP_EXPECTED: [[u8; 2]; 2] = [[16, 16], [32, 32]];

/// The strip through the disc iris focused at infinity matches the hand derivation in every sensel.
#[test]
fn strip_through_the_disc_iris_matches_hand_derivation() {
    assert_eq!(
        check_strip_through_the_disc_iris_matches_hand_derivation(),
        Ok(())
    );
}

fn check_strip_through_the_disc_iris_matches_hand_derivation() -> Result<(), Fail> {
    let frame = render_strip()?;
    for (row, values) in STRIP_EXPECTED.iter().enumerate() {
        for (column, level) in values.iter().enumerate() {
            let cell = PixelCoord::new(
                PixelColumn(u16::try_from(column).or_fail()?),
                PixelRow(u16::try_from(row).or_fail()?),
            );
            let found = frame.get_pixel(cell).or_fail()?;
            ensure_eq!(found, gray(Channel(*level)));
        }
    }
    Ok(())
}

/// A disc lens of f/2 on the two by two grid, with the given focal length in sixteenths of a frame
/// height, the given focus, and an iris of the given frame height in world units.
fn disc_lens_with_focus(focal: NonZeroU8, focus: Focus, frame_height: NonZeroU16) -> Lens {
    let iris = Iris::new(
        FStop::F2,
        ApertureShape::Disc,
        focus,
        FrameHeight(frame_height),
        ApertureGrid::Two,
    );
    Lens::new(FocalLength(focal), Aperture::Iris(iris), Vignette::None)
}

/// The disc lens of the near test: a focal length of 16 sixteenths, an iris frame height of
/// `H = 25` world units, and the given focus.
fn near_lens(focus: Focus) -> Result<Lens, Fail> {
    Ok(disc_lens_with_focus(
        NonZeroU8::new(16).or_fail()?,
        focus,
        NonZeroU16::new(25).or_fail()?,
    ))
}

/// The disc lens of the beyond test: a focal length of 128 sixteenths, an iris frame height of
/// `H = 1` world unit, and the given focus.
fn beyond_lens(focus: Focus) -> Result<Lens, Fail> {
    Ok(disc_lens_with_focus(
        NonZeroU8::new(128).or_fail()?,
        focus,
        NonZeroU16::new(1).or_fail()?,
    ))
}

/// The focus of the near test: 200 world units, which is eight focal lengths of `f = 25`.
fn near_focus() -> Result<Focus, Fail> {
    let distance = FocusDistance::new(NonZeroU32::new(200).or_fail()?).or_fail()?;
    Ok(Focus::At(distance))
}

/// The focus of the beyond test: 64 world units, which is eight focal lengths of `f = 8`.
fn beyond_focus() -> Result<Focus, Fail> {
    let distance = FocusDistance::new(NonZeroU32::new(64).or_fail()?).or_fail()?;
    Ok(Focus::At(distance))
}

/// The strip of the near test: a white strip at depth `z = 16`, nearer than the plane of focus.
fn near_layer() -> Result<Layer, Fail> {
    strip_layer_at(StripPlacement {
        depth: RawCoord(16),
        left: RawCoord(-1),
    })
}

/// The strip of the beyond test: a white strip at depth `z = 256`, beyond the plane of focus.
fn beyond_layer() -> Result<Layer, Fail> {
    strip_layer_at(StripPlacement {
        depth: RawCoord(256),
        left: RawCoord(-2),
    })
}

/// The expected gray level of each sensel of the near strip shot in focus at `s = 200`, indexed
/// as `[row][column]`.
///
/// Derivation. The iris frame height is `H = 25` and the focal length is `f = fn * H / 16 = 25`, so
/// the focus `s = 200` is `8f` and `2 * s = fn * H = 400` is accepted. The strip is at depth
/// `z = 16`, so the layer is nearer than the plane of focus. The output is 2 sensels high, so the
/// layer projects to `(256 + 32 x, 256 + 32 y)`, which covers the lattice rectangle
/// `[224, 256] x [256, 320]`.
///
/// A lens point with numerators `(+-1, +-1)` shifts the layer on each axis by
/// `H h fn^2 Nd g (s - z) / (2 Nn A z s)`, which is `25 * 2 * 256 * 184 g / (2 * 2 * 2 * 16 * 200)
/// = 92 g`. The four shifts are therefore `(+-92, +-92)`, and the pair with sample minus shift in
/// the rectangle is the only one covered:
///
/// - the sample `x = 320` with shift `+92` lands at `228`, inside `[224, 256]`; the samples `x = 64`,
///   `192` and `448` need shifts in `[-192, -160]`, `[-64, -32]` and `[192, 224]`, which `92` is not;
/// - the sample `y = 192` with shift `-92` lands at `284`, inside `[256, 320]`; the samples `y = 64`,
///   `320` and `448` need shifts in `[-256, -192]`, `[0, 64]` and `[128, 192]`, which `92` is not.
///
/// So the lens point `(+92, -92)` covers the pair `(320, 192)`, which lies in sensel `(1, 0)`. Each
/// sensel has 16 pairs, so one covered pair averages to `floor((2 * 255 + 16) / 32) = 16`.
///
/// The wrong formula that treats the focus as infinite would shift by `100 g`, which covers no pair,
/// as the infinite expectation below shows. Ignoring the `(s - z) / s` factor gives the same `100`.
const NEAR_EXPECTED: [[Channel; 2]; 2] = [[Channel(0), Channel(16)], [Channel(0), Channel(0)]];

/// The expected gray level of each sensel of the near strip shot focused at infinity, indexed as
/// `[row][column]`.
///
/// Derivation. With focus at infinity the shift is `H h fn^2 Nd g / (2 Nn A z)`, which is
/// `25 * 2 * 256 g / (2 * 2 * 2 * 16) = 100 g`. The shifts are `(+-100, +-100)`. A sample `x = 320`
/// with shift `+100` lands at `220`, outside `[224, 256]`, and no other sample lands inside the
/// strip for any shift of magnitude 100 on either axis. The frame is therefore black.
///
/// This excludes the reading of the focused lens as the infinite one: the focused frame has a 16 at
/// sensel `(1, 0)`, which the infinite frame must not have.
const NEAR_AT_INFINITY: [[Channel; 2]; 2] = [[Channel(0), Channel(0)], [Channel(0), Channel(0)]];

/// The strip layer in front of the plane of focus matches the hand derivation in every sensel,
/// both through the focused lens and through the lens focused at infinity.
///
/// The focused shift is `92`, which covers one pair. The infinite shift is `100`, which covers
/// none, so the two frames differ. A lens that treats the focus as infinite, or that drops the
/// `(s - z) / s` factor, produces the infinite frame and fails the first assertion.
#[test]
fn near_layer_before_the_plane_of_focus_matches_hand_derivation() {
    assert_eq!(
        check_near_layer_before_the_plane_of_focus_matches_hand_derivation(),
        Ok(())
    );
}

fn check_near_layer_before_the_plane_of_focus_matches_hand_derivation() -> Result<(), Fail> {
    let focused = render_strip_through(near_lens(near_focus()?)?, near_layer()?)?;
    check_two_by_two(&focused, NEAR_EXPECTED)?;
    let infinite = render_strip_through(near_lens(Focus::Infinity)?, near_layer()?)?;
    check_two_by_two(&infinite, NEAR_AT_INFINITY)?;
    Ok(())
}

/// Checks each sensel of a two by two frame against the expected gray level.
fn check_two_by_two(frame: &Frame, expected: [[Channel; 2]; 2]) -> Result<(), Fail> {
    for (row, values) in expected.iter().enumerate() {
        for (column, level) in values.iter().enumerate() {
            let cell = PixelCoord::new(
                PixelColumn(u16::try_from(column).or_fail()?),
                PixelRow(u16::try_from(row).or_fail()?),
            );
            let found = frame.get_pixel(cell).or_fail()?;
            ensure_eq!(found, gray(*level));
        }
    }
    Ok(())
}

/// The expected gray level of each sensel of the beyond strip shot focused at `s = 64`, indexed as
/// `[row][column]`.
///
/// Derivation. The iris frame height is `H = 1` and the focal length is `f = 128 * 1 / 16 = 8`, so
/// the focus `s = 64` is `8f` and `2 * s = fn * H = 128` is accepted. The strip is at depth
/// `z = 256`, beyond the plane of focus. The output is 2 sensels high, so the layer projects to
/// `(256 + 16 x, 256 + 16 y)`, which covers the lattice rectangle `[224, 256] x [256, 288]`.
///
/// A lens point shifts the layer on each axis by `H h fn^2 Nd g (z - s) / (2 Nn A z s)`, which is
/// `1 * 2 * 16384 * 192 g / (2 * 2 * 2 * 256 * 64) = 48 g`. The four shifts are `(+-48, +-48)`.
/// Only one pair is covered:
///
/// - the sample `x = 192` with shift `-48` lands at `240`, inside `[224, 256]`; the other samples
///   need shifts in `[-192, -160]`, `[64, 96]` and `[192, 224]`, which `+-48` is not;
/// - the sample `y = 320` with shift `+48` lands at `272`, inside `[256, 288]`; the other samples
///   need shifts in `[-224, -192]`, `[-96, -64]` and `[160, 192]`, which `+-48` is not.
///
/// So the lens point `(-48, +48)` covers the pair `(192, 320)`, which lies in sensel `(0, 1)`. That
/// sensel has one covered pair of 16, which averages to `floor((2 * 255 + 16) / 32) = 16`.
///
/// This excludes the reading of the focused lens as the infinite one, and the reading that ignores
/// the `(z - s) / s` factor: with the infinite shift `16 g` the frame is black, as the expectation
/// below records.
const BEYOND_EXPECTED: [[Channel; 2]; 2] = [[Channel(0), Channel(0)], [Channel(16), Channel(0)]];

/// The expected gray level of each sensel of the beyond strip shot focused at infinity, indexed as
/// `[row][column]`.
///
/// Derivation. With focus at infinity the shift is `H h fn^2 Nd g / (2 Nn A z)`, which is
/// `1 * 2 * 16384 g / (2 * 2 * 2 * 256) = 16 g`. The shifts are `(+-16, +-16)`. The sample `x = 192`
/// lands at `176` or `208`, and the sample `x = 320` lands at `304` or `336`, both outside
/// `[224, 256]`, and the other samples are farther still. No sample lands in the strip, so the
/// frame is black.
const BEYOND_AT_INFINITY: [[Channel; 2]; 2] = [[Channel(0), Channel(0)], [Channel(0), Channel(0)]];

/// The strip layer beyond the plane of focus matches the hand derivation in every sensel, both
/// through the focused lens and through the lens focused at infinity.
///
/// The focused shift is `48`, which covers one pair. The infinite shift is `16`, which covers
/// none, so the two frames differ. A lens that treats the focus as infinite, or that drops the
/// `(z - s) / s` factor, produces the infinite frame and fails the first assertion.
#[test]
fn far_layer_beyond_the_plane_of_focus_matches_hand_derivation() {
    assert_eq!(
        check_far_layer_beyond_the_plane_of_focus_matches_hand_derivation(),
        Ok(())
    );
}

fn check_far_layer_beyond_the_plane_of_focus_matches_hand_derivation() -> Result<(), Fail> {
    let focused = render_strip_through(beyond_lens(beyond_focus()?)?, beyond_layer()?)?;
    check_two_by_two(&focused, BEYOND_EXPECTED)?;
    let infinite = render_strip_through(beyond_lens(Focus::Infinity)?, beyond_layer()?)?;
    check_two_by_two(&infinite, BEYOND_AT_INFINITY)?;
    Ok(())
}
