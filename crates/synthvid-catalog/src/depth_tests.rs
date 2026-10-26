//! Tests for the depth singletons, which record a deep shot seen through a perspective camera.

extern crate alloc;

use crate::cover::{generate_catalogue, DeepAxis, Entry, RecordingAxis};
use alloc::collections::BTreeSet;
use core::num::{NonZeroI64, NonZeroU16, NonZeroU32, NonZeroU8};
use synthvid_scene::frame::{PixelColumn, PixelCoord, PixelRow};
use synthvid_scene::{
    rotation_at, track_at, Channel, Frame, FrameIndex, Layer, Numerator, Ratio, Rgb8, Track,
};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};
use synthvid_video::deep::render_stage_into;
use synthvid_video::json_names::JsonEntryName;
use synthvid_video::{
    render, Aperture, ApertureGrid, ApertureShape, BladeCount, CaptureRate, Container, DeepShot,
    DisplayRotation, Encoding, FStop, Focus, FocusDistance, Format, FrameHeight, Iris, Lens, Rig,
    Shot, VideoSpec,
};

mod bokeh;

/// The number of entries before the deep shots: the covering array, the capture singletons and
/// the recording singletons, in that order.
const PREVIOUS_ENTRY_COUNT: usize = 148;

/// The number of entries in the catalog: the previous ones and the four deep shots.
const TOTAL_ENTRY_COUNT: usize = 152;

/// The manifest names of the four deep shots, in generation order: parallax, roll, disc bokeh
/// and hexagonal bokeh. Each is the recording scheme of an ideal 32 by 24 entry, with the depth
/// segment after the defect segment.
const DEEP_NAMES: [&str; 4] = [
    "0032x0024-8-30fps-raw-iso-identity-unscaled-nodef-depparallax",
    "0032x0024-8-30fps-raw-iso-identity-unscaled-nodef-deproll",
    "0032x0024-8-30fps-raw-iso-identity-unscaled-nodef-depdisc",
    "0032x0024-8-30fps-raw-iso-identity-unscaled-nodef-dephex",
];

/// The depth of each deep shot, in the same order as [`DEEP_NAMES`].
const DEEP_DEPTHS: [DeepAxis; 4] = [
    DeepAxis::Parallax,
    DeepAxis::Roll,
    DeepAxis::DiscBokeh,
    DeepAxis::HexBokeh,
];

/// The segments the parallax name is split into, with the depth segment last. The deep shot has
/// no background, shape or motion segment.
const PARALLAX_SEGMENTS: [&str; 9] = [
    "0032x0024",
    "8",
    "30fps",
    "raw",
    "iso",
    "identity",
    "unscaled",
    "nodef",
    "depparallax",
];

/// A count of sensels in a band of columns.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct SenselCount(u32);

/// The number of code values by which a sensel's red channel exceeds the backdrop's red channel.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct CodeExcess(u32);

/// A band of columns, from the first column up to but not including the end column.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct ColumnBand {
    /// The first column of the band.
    first: PixelColumn,
    /// The column after the last column of the band.
    end: PixelColumn,
}

/// The four deep entries of the catalog, in generation order.
fn deep_entries() -> Result<Vec<Entry>, Fail> {
    let catalogue = generate_catalogue().or_fail()?;
    Ok(catalogue
        .into_iter()
        .filter(|entry| entry.depth().is_some())
        .collect())
}

/// The deep entry of the given depth effect.
fn deep_entry(depth: DeepAxis) -> Result<Entry, Fail> {
    generate_catalogue()
        .or_fail()?
        .into_iter()
        .find(|entry| entry.depth() == Some(depth))
        .or_fail()
}

/// The deep shot of a specification, or an error when the specification holds a flat shot.
fn deep_shot(spec: &VideoSpec) -> Result<&DeepShot, Fail> {
    match spec.shot() {
        Shot::Flat(_) => Err(Fail::here(format_args!(
            "a depth entry must record a deep shot"
        ))),
        Shot::Deep(deep) => Ok(deep.as_ref()),
    }
}

/// The hexagonal outline: an iris of six blades.
fn hexagon() -> Result<ApertureShape, Fail> {
    let count = NonZeroU8::new(6).or_fail()?;
    Ok(ApertureShape::Polygon(BladeCount::new(count).or_fail()?))
}

/// The iris the bokeh shots are seen through: an F2 iris of the given outline, focused at depth
/// 864, with a frame height of 16 and eight samples along each axis.
fn expected_iris(shape: ApertureShape) -> Result<Iris, Fail> {
    let distance = NonZeroU32::new(864).or_fail()?;
    Ok(Iris::new(
        FStop::F2,
        shape,
        Focus::At(FocusDistance::new(distance).or_fail()?),
        FrameHeight(NonZeroU16::new(16).or_fail()?),
        ApertureGrid::Eight,
    ))
}

/// Checks that an aperture is an iris equal to the expected iris of the given outline.
fn check_iris(aperture: Aperture, shape: ApertureShape) -> Result<(), Fail> {
    match aperture {
        Aperture::Pinhole => Err(Fail::here(format_args!(
            "a bokeh shot must be seen through an iris"
        ))),
        Aperture::Iris(iris) => {
            ensure_eq!(iris.stop(), FStop::F2);
            ensure_eq!(iris.shape(), shape);
            ensure_eq!(iris.focus(), expected_iris(shape)?.focus());
            ensure_eq!(
                iris.frame_height(),
                FrameHeight(NonZeroU16::new(16).or_fail()?)
            );
            ensure_eq!(iris.samples(), ApertureGrid::Eight);
            Ok(())
        }
    }
}

fn check_catalog_has_152_entries_with_the_deep_shots_last() -> Result<(), Fail> {
    let catalogue = generate_catalogue().or_fail()?;
    ensure_eq!(catalogue.len(), TOTAL_ENTRY_COUNT);
    let previous = catalogue.get(..PREVIOUS_ENTRY_COUNT).or_fail()?;
    for entry in previous {
        ensure_eq!(entry.depth(), None, "{}", entry.manifest_name());
    }
    let added = catalogue.get(PREVIOUS_ENTRY_COUNT..).or_fail()?;
    let names: Vec<String> = added
        .iter()
        .map(|entry| entry.manifest_name().as_str().to_owned())
        .collect();
    let expected: Vec<String> = DEEP_NAMES.iter().map(|name| (*name).to_owned()).collect();
    ensure_eq!(
        names,
        expected,
        "the deep shots follow the recording singletons"
    );
    let depths: Vec<Option<DeepAxis>> = added.iter().map(Entry::depth).collect();
    ensure_eq!(depths, DEEP_DEPTHS.map(Some).to_vec());
    Ok(())
}

#[test]
fn catalog_has_152_entries_with_the_deep_shots_last() {
    assert_eq!(
        check_catalog_has_152_entries_with_the_deep_shots_last(),
        Ok(())
    );
}

fn check_names_are_unique_and_the_flat_ones_carry_no_depth_segment() -> Result<(), Fail> {
    let catalogue = generate_catalogue().or_fail()?;
    let mut names = BTreeSet::new();
    for entry in &catalogue {
        let name = entry.manifest_name();
        ensure!(
            names.insert(name.as_str().to_owned()),
            "entry names must be unique: {name}"
        );
        if entry.depth().is_none() {
            ensure!(
                !name.as_str().contains("-dep"),
                "flat entry {name} must not carry a depth segment"
            );
        }
    }
    Ok(())
}

#[test]
fn names_are_unique_and_the_flat_ones_carry_no_depth_segment() {
    assert_eq!(
        check_names_are_unique_and_the_flat_ones_carry_no_depth_segment(),
        Ok(())
    );
}

fn check_deep_names_are_exact_and_follow_the_scheme() -> Result<(), Fail> {
    let entries = deep_entries()?;
    let names: Vec<String> = entries
        .iter()
        .map(|entry| entry.manifest_name().as_str().to_owned())
        .collect();
    let expected: Vec<String> = DEEP_NAMES.iter().map(|name| (*name).to_owned()).collect();
    ensure_eq!(names, expected);
    let parallax = entries.first().or_fail()?.manifest_name();
    let segments: Vec<&str> = parallax.as_str().split('-').collect();
    ensure_eq!(segments, PARALLAX_SEGMENTS.to_vec());
    Ok(())
}

#[test]
fn deep_names_are_exact_and_follow_the_scheme() {
    assert_eq!(check_deep_names_are_exact_and_follow_the_scheme(), Ok(()));
}

fn check_deep_entries_are_ideal_with_playback_only_capture_and_no_defect() -> Result<(), Fail> {
    for entry in deep_entries()? {
        ensure_eq!(entry.recording(), RecordingAxis::Ideal);
        ensure_eq!(entry.capture(), CaptureRate::PlaybackOnly);
        ensure_eq!(entry.defect(), None);
    }
    Ok(())
}

#[test]
fn deep_entries_are_ideal_with_playback_only_capture_and_no_defect() {
    assert_eq!(
        check_deep_entries_are_ideal_with_playback_only_capture_and_no_defect(),
        Ok(())
    );
}

fn check_deep_entries_hold_no_flat_scene() -> Result<(), Fail> {
    for entry in deep_entries()? {
        ensure!(
            entry.scene().flat().is_none(),
            "a deep entry holds no background, shape or motion: {}",
            entry.manifest_name()
        );
    }
    Ok(())
}

#[test]
fn deep_entries_hold_no_flat_scene() {
    assert_eq!(check_deep_entries_hold_no_flat_scene(), Ok(()));
}

fn check_each_deep_entry_builds_its_shot(entry: &Entry) -> Result<(), Fail> {
    let spec = entry.to_video_spec().or_fail()?;
    let deep = deep_shot(&spec)?;
    let recorder = spec.recorder();
    ensure_eq!(
        recorder.dimensions().width.get().get(),
        32,
        "{}",
        entry.manifest_name()
    );
    ensure_eq!(
        recorder.dimensions().height.get().get(),
        24,
        "{}",
        entry.manifest_name()
    );
    ensure_eq!(
        recorder.timing().count().get().get(),
        8,
        "{}",
        entry.manifest_name()
    );
    match entry.depth() {
        None => Err(Fail::here(format_args!("a depth entry cannot be flat"))),
        Some(DeepAxis::Parallax | DeepAxis::Roll) => match deep.rig().lens().aperture() {
            Aperture::Pinhole => Ok(()),
            Aperture::Iris(_) => Err(Fail::here(format_args!(
                "parallax and roll shots are seen through a pinhole"
            ))),
        },
        Some(DeepAxis::DiscBokeh) => check_iris(deep.rig().lens().aperture(), ApertureShape::Disc),
        Some(DeepAxis::HexBokeh) => check_iris(deep.rig().lens().aperture(), hexagon()?),
    }
}

fn check_deep_entries_build_their_shots() -> Result<(), Fail> {
    for entry in deep_entries()? {
        check_each_deep_entry_builds_its_shot(&entry)?;
    }
    Ok(())
}

#[test]
fn deep_entries_build_their_shots() {
    assert_eq!(check_deep_entries_build_their_shots(), Ok(()));
}

fn check_deep_entries_render_perspective_manifests_identically_twice() -> Result<(), Fail> {
    for entry in deep_entries()? {
        let spec = entry.to_video_spec().or_fail()?;
        let first = render(&spec).or_fail()?;
        let second = render(&spec).or_fail()?;
        let manifest = first.manifest_json();
        ensure!(
            manifest.as_str().contains("\"projection\":\"perspective\""),
            "manifest of {} must declare a perspective projection",
            entry.manifest_name()
        );
        ensure!(
            manifest.as_str().contains("\"schema_version\":2"),
            "manifest of {} must declare schema version 2",
            entry.manifest_name()
        );
        ensure!(
            !first.media_bytes().as_bytes().is_empty(),
            "media of {} must not be empty",
            entry.manifest_name()
        );
        ensure_eq!(
            manifest.as_str(),
            second.manifest_json().as_str(),
            "manifests of {}",
            entry.manifest_name()
        );
        ensure_eq!(
            first.media_bytes().as_bytes(),
            second.media_bytes().as_bytes(),
            "media of {}",
            entry.manifest_name()
        );
    }
    Ok(())
}

#[test]
fn deep_entries_render_perspective_manifests_identically_twice() {
    assert_eq!(
        check_deep_entries_render_perspective_manifests_identically_twice(),
        Ok(())
    );
}

/// The specification of the entry's shot seen through a pinhole rather than its iris, with
/// everything else the same.
fn pinhole_twin(entry: &Entry) -> Result<VideoSpec, Fail> {
    let spec = entry.to_video_spec().or_fail()?;
    let deep = deep_shot(&spec)?;
    let rig = deep.rig();
    let lens = Lens::new(
        rig.lens().focal_length(),
        Aperture::Pinhole,
        rig.lens().vignette(),
    );
    let pinhole = Rig::new(&rig.pose(), lens, &rig.recorder());
    let shot = DeepShot::new(deep.stage().clone(), &pinhole, deep.pixels()).or_fail()?;
    let format = Format::new(
        Encoding::Raw,
        Container::Iso {
            rotation: DisplayRotation::Identity,
            capture: CaptureRate::PlaybackOnly,
        },
        None,
    );
    VideoSpec::deep(
        JsonEntryName::new(entry.manifest_name().as_str()).or_fail()?,
        shot,
        format,
    )
    .or_fail()
}

fn check_bokeh_media_differs_from_pinhole_and_from_each_other() -> Result<(), Fail> {
    let disc = deep_entry(DeepAxis::DiscBokeh)?;
    let hex = deep_entry(DeepAxis::HexBokeh)?;
    let disc_media = render(&disc.to_video_spec().or_fail()?).or_fail()?;
    let hex_media = render(&hex.to_video_spec().or_fail()?).or_fail()?;
    for (entry, blurred) in [(&disc, &disc_media), (&hex, &hex_media)] {
        let pinhole = render(&pinhole_twin(entry)?).or_fail()?;
        ensure!(
            blurred.media_bytes().as_bytes() != pinhole.media_bytes().as_bytes(),
            "the iris must change the media of {}",
            entry.manifest_name()
        );
    }
    ensure!(
        disc_media.media_bytes().as_bytes() != hex_media.media_bytes().as_bytes(),
        "the disc and hexagonal shots must differ"
    );
    Ok(())
}

#[test]
fn bokeh_media_differs_from_pinhole_and_from_each_other() {
    assert_eq!(
        check_bokeh_media_differs_from_pinhole_and_from_each_other(),
        Ok(())
    );
}

/// The position of a layer across the frame, in sensels from the left edge, seen from the camera
/// at one frame. The layer and the camera are read from their tracks, and the projection is
/// exact: the sensel position is `16 + 24 * X / z`, where `X` and `z` are the layer's across
/// offset and depth relative to the camera.
fn sensel_across(layer: &Layer, camera: &Track, frame: FrameIndex) -> Result<Ratio, Fail> {
    let placed = track_at(&layer.track, frame).or_fail()?;
    let seen = track_at(camera, frame).or_fail()?;
    let across = Ratio::from_integer(Numerator(i64::from(placed.across.get().0)))
        .checked_sub(Ratio::from_integer(Numerator(i64::from(
            seen.across.get().0,
        ))))
        .or_fail()?;
    let depth = Ratio::from_integer(Numerator(i64::from(placed.depth.get().0)))
        .checked_sub(Ratio::from_integer(Numerator(i64::from(
            seen.depth.get().0,
        ))))
        .or_fail()?;
    let offset = Ratio::from_integer(Numerator(24))
        .checked_mul(across)
        .or_fail()?
        .checked_div(depth)
        .or_fail()?;
    Ratio::from_integer(Numerator(16))
        .checked_add(offset)
        .or_fail()
}

/// The parallax shot: the near red disc moves farther across the frame than the far blue disc
/// between frames 0 and 7.
///
/// The camera at frame 7 sits 70 units to the right and 140 units forward of its frame 0 position.
/// The near disc is at across -40 and depth 400, so its sensel position is 16 + 24 * -40 / 400 =
/// 68/5 at frame 0, and 16 + 24 * (-40 - 70) / (400 - 140) = 76/13 at frame 7. It moves 504/65,
/// about 7.75 sensels. The far disc is at across 200 and depth 1600, so it sits at 19 at frame 0,
/// and at 16 + 24 * (200 - 70) / (1600 - 140) = 1324/73 at frame 7. It moves 63/73, about 0.86
/// sensels.
fn check_parallax_near_layer_moves_farther_than_far_layer() -> Result<(), Fail> {
    let spec = deep_entry(DeepAxis::Parallax)?.to_video_spec().or_fail()?;
    let deep = deep_shot(&spec)?;
    let camera = deep.rig().pose().track();
    let far = deep.stage().layers.first().or_fail()?;
    let near = deep.stage().layers.get(2).or_fail()?;
    let start = FrameIndex::new(0);
    let later = FrameIndex::new(7);
    let near_start = sensel_across(near, &camera, start)?;
    let near_later = sensel_across(near, &camera, later)?;
    let far_start = sensel_across(far, &camera, start)?;
    let far_later = sensel_across(far, &camera, later)?;
    ensure_eq!(
        near_start,
        Ratio::new(Numerator(68), NonZeroI64::new(5).or_fail()?).or_fail()?
    );
    ensure_eq!(
        near_later,
        Ratio::new(Numerator(76), NonZeroI64::new(13).or_fail()?).or_fail()?
    );
    ensure_eq!(far_start, Ratio::from_integer(Numerator(19)));
    ensure_eq!(
        far_later,
        Ratio::new(Numerator(1324), NonZeroI64::new(73).or_fail()?).or_fail()?
    );
    let near_moved = near_start.checked_sub(near_later).or_fail()?;
    let far_moved = far_start.checked_sub(far_later).or_fail()?;
    ensure!(
        near_moved > far_moved,
        "the near layer moved {near_moved} sensels and the far layer {far_moved}"
    );
    Ok(())
}

#[test]
fn parallax_near_layer_moves_farther_than_far_layer() {
    assert_eq!(
        check_parallax_near_layer_moves_farther_than_far_layer(),
        Ok(())
    );
}

/// The roll shot turns 1/32 of a turn per frame from a start of zero, so frame 7 is turned by
/// 7/32 of a turn and frame 0 by none.
fn check_roll_turns_one_thirty_second_of_a_turn_per_frame() -> Result<(), Fail> {
    let spec = deep_entry(DeepAxis::Roll)?.to_video_spec().or_fail()?;
    let deep = deep_shot(&spec)?;
    let roll = deep.rig().pose().roll();
    let first = rotation_at(&roll, FrameIndex::new(0)).or_fail()?;
    let seventh = rotation_at(&roll, FrameIndex::new(7)).or_fail()?;
    ensure_eq!(first.get(), Ratio::from_integer(Numerator(0)));
    ensure_eq!(
        seventh.get(),
        Ratio::new(Numerator(7), NonZeroI64::new(32).or_fail()?).or_fail()?
    );
    Ok(())
}

#[test]
fn roll_turns_one_thirty_second_of_a_turn_per_frame() {
    assert_eq!(
        check_roll_turns_one_thirty_second_of_a_turn_per_frame(),
        Ok(())
    );
}
