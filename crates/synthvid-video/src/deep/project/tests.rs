use super::*;
use crate::deep::TrigNumerator;
use core::num::{NonZeroI64, NonZeroU16, NonZeroU8};
use synthvid_scene::{Numerator, Ratio, Turns};
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

/// A camera roll of the given fraction of a turn.
fn turns(numerator: Numerator, denominator: NonZeroI64) -> Result<Turns, Fail> {
    Ok(Turns::new(Ratio::new(numerator, denominator).or_fail()?))
}

/// The trigonometry of a camera with no roll.
fn no_roll() -> Result<TrigPair, Fail> {
    TrigPair::of(turns(Numerator(0), NonZeroI64::new(1).or_fail()?)?).or_fail()
}

/// The trigonometry of a camera rolled by a quarter turn.
fn quarter_roll() -> Result<TrigPair, Fail> {
    TrigPair::of(turns(Numerator(1), NonZeroI64::new(4).or_fail()?)?).or_fail()
}

/// A 64 by 48 frame, so the frame center is at lattice `(8192, 6144)`.
fn frame() -> Result<Dimensions, Fail> {
    Ok(Dimensions::new(
        Width(NonZeroU16::new(64).or_fail()?),
        Height(NonZeroU16::new(48).or_fail()?),
    ))
}

/// A focal length of 16 sixteenths, which is one frame height.
fn focal() -> Result<FocalLength, Fail> {
    Ok(FocalLength(NonZeroU8::new(16).or_fail()?))
}

/// A radius of the given non-zero number of world units.
fn radius(units: NonZeroU32) -> Result<Extent, Fail> {
    Extent::new(units).or_fail()
}

#[test]
fn trig_pair_of_no_roll_is_unit_cosine() {
    assert_eq!(check_trig_pair_of_no_roll_is_unit_cosine(), Ok(()));
}

fn check_trig_pair_of_no_roll_is_unit_cosine() -> Result<(), Fail> {
    let pair = no_roll()?;
    ensure_eq!(pair.cosine(), TrigNumerator(0x8000));
    ensure_eq!(pair.sine(), TrigNumerator(0));
    Ok(())
}

#[test]
fn vertex_without_roll_lands_on_hand_computed_lattice_point() {
    assert_eq!(
        check_vertex_without_roll_lands_on_hand_computed_lattice_point(),
        Ok(())
    );
}

fn check_vertex_without_roll_lands_on_hand_computed_lattice_point() -> Result<(), Fail> {
    let point = project_vertex(
        PlaneOffset {
            across: WorldOffset(10),
            down: WorldOffset(-5),
        },
        Depth(NonZeroU32::new(100).or_fail()?),
        no_roll()?,
        focal()?,
        frame()?,
    );
    ensure_eq!(
        point,
        Ok(LatticePoint {
            across: SubSensel(9421),
            down: SubSensel(5530),
        })
    );
    Ok(())
}

#[test]
fn vertex_under_quarter_roll_is_rotated_before_projection() {
    assert_eq!(
        check_vertex_under_quarter_roll_is_rotated_before_projection(),
        Ok(())
    );
}

fn check_vertex_under_quarter_roll_is_rotated_before_projection() -> Result<(), Fail> {
    let point = project_vertex(
        PlaneOffset {
            across: WorldOffset(10),
            down: WorldOffset(-5),
        },
        Depth(NonZeroU32::new(100).or_fail()?),
        quarter_roll()?,
        focal()?,
        frame()?,
    );
    ensure_eq!(
        point,
        Ok(LatticePoint {
            across: SubSensel(7578),
            down: SubSensel(4915),
        })
    );
    Ok(())
}

#[test]
fn vertex_half_way_offsets_round_toward_positive_infinity() {
    assert_eq!(
        check_vertex_half_way_offsets_round_toward_positive_infinity(),
        Ok(())
    );
}

fn check_vertex_half_way_offsets_round_toward_positive_infinity() -> Result<(), Fail> {
    let positive = project_vertex(
        PlaneOffset {
            across: WorldOffset(1),
            down: WorldOffset(0),
        },
        Depth(NonZeroU32::new(8192).or_fail()?),
        no_roll()?,
        focal()?,
        frame()?,
    );
    ensure_eq!(
        positive,
        Ok(LatticePoint {
            across: SubSensel(8194),
            down: SubSensel(6144),
        })
    );
    let negative = project_vertex(
        PlaneOffset {
            across: WorldOffset(-1),
            down: WorldOffset(0),
        },
        Depth(NonZeroU32::new(8192).or_fail()?),
        no_roll()?,
        focal()?,
        frame()?,
    );
    ensure_eq!(
        negative,
        Ok(LatticePoint {
            across: SubSensel(8191),
            down: SubSensel(6144),
        })
    );
    Ok(())
}

#[test]
fn vertex_beyond_the_guard_band_is_refused_without_clamping() {
    assert_eq!(
        check_vertex_beyond_the_guard_band_is_refused_without_clamping(),
        Ok(())
    );
}

fn check_vertex_beyond_the_guard_band_is_refused_without_clamping() -> Result<(), Fail> {
    let point = project_vertex(
        PlaneOffset {
            across: WorldOffset(0x0020_0000),
            down: WorldOffset(0),
        },
        Depth(NonZeroU32::new(1).or_fail()?),
        no_roll()?,
        focal()?,
        frame()?,
    );
    ensure_eq!(point, Err(ProjectionError::BeyondGuardBand));
    Ok(())
}

#[test]
fn vertex_one_thousand_units_across_at_depth_one_is_beyond_the_guard_band() {
    assert_eq!(
        check_vertex_one_thousand_units_across_at_depth_one_is_beyond_the_guard_band(),
        Ok(())
    );
}

fn check_vertex_one_thousand_units_across_at_depth_one_is_beyond_the_guard_band() -> Result<(), Fail>
{
    // The frame is 1080 sensels high and the focal length is 16 sixteenths, so the across offset
    // projects to 16 * 1080 * 16 * 1000 = 276480000 lattice units. That exceeds 2^28 = 268435456
    // before the frame center is added, so the vertex is refused.
    let dimensions = Dimensions::new(
        Width(NonZeroU16::new(64).or_fail()?),
        Height(NonZeroU16::new(1080).or_fail()?),
    );
    let point = project_vertex(
        PlaneOffset {
            across: WorldOffset(1000),
            down: WorldOffset(0),
        },
        Depth(NonZeroU32::new(1).or_fail()?),
        no_roll()?,
        focal()?,
        dimensions,
    );
    ensure_eq!(point, Err(ProjectionError::BeyondGuardBand));
    Ok(())
}

#[test]
fn vertex_at_extreme_bounds_is_refused_without_overflow() {
    assert_eq!(
        check_vertex_at_extreme_bounds_is_refused_without_overflow(),
        Ok(())
    );
}

fn check_vertex_at_extreme_bounds_is_refused_without_overflow() -> Result<(), Fail> {
    let dimensions = Dimensions::new(Width(NonZeroU16::MAX), Height(NonZeroU16::MAX));
    let focal_extreme = FocalLength(NonZeroU8::MAX);
    let point = project_vertex(
        PlaneOffset {
            across: WorldOffset(0x0020_0000),
            down: WorldOffset(-0x0020_0000),
        },
        Depth(NonZeroU32::new(1).or_fail()?),
        no_roll()?,
        focal_extreme,
        dimensions,
    );
    ensure_eq!(point, Err(ProjectionError::BeyondGuardBand));
    Ok(())
}

#[test]
fn radius_of_three_units_at_depth_100_is_369_lattice_units() {
    assert_eq!(
        check_radius_of_three_units_at_depth_100_is_369_lattice_units(),
        Ok(())
    );
}

fn check_radius_of_three_units_at_depth_100_is_369_lattice_units() -> Result<(), Fail> {
    let length = project_radius(
        radius(NonZeroU32::new(3).or_fail()?)?,
        Depth(NonZeroU32::new(100).or_fail()?),
        focal()?,
        frame()?,
    );
    ensure_eq!(length, Ok(SubSensel(369)));
    Ok(())
}

#[test]
fn radius_of_five_units_at_depth_seven_rounds_to_8777() {
    assert_eq!(
        check_radius_of_five_units_at_depth_seven_rounds_to_8777(),
        Ok(())
    );
}

fn check_radius_of_five_units_at_depth_seven_rounds_to_8777() -> Result<(), Fail> {
    let length = project_radius(
        radius(NonZeroU32::new(5).or_fail()?)?,
        Depth(NonZeroU32::new(7).or_fail()?),
        focal()?,
        frame()?,
    );
    ensure_eq!(length, Ok(SubSensel(8777)));
    Ok(())
}

#[test]
fn radius_beyond_the_guard_band_is_refused() {
    assert_eq!(check_radius_beyond_the_guard_band_is_refused(), Ok(()));
}

fn check_radius_beyond_the_guard_band_is_refused() -> Result<(), Fail> {
    let length = project_radius(
        radius(NonZeroU32::new(0x0010_0000).or_fail()?)?,
        Depth(NonZeroU32::new(1).or_fail()?),
        FocalLength(NonZeroU8::MAX),
        Dimensions::new(Width(NonZeroU16::MAX), Height(NonZeroU16::MAX)),
    );
    ensure_eq!(length, Err(ProjectionError::BeyondGuardBand));
    Ok(())
}
