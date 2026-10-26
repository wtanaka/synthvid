use super::*;
use crate::optics::FocusDistance;
use crate::testutil::Frac;
use core::num::{NonZeroU16, NonZeroU32};
use synthvid_testkit::{ensure, ensure_eq, ensure_ne, Fail, OrFail as _};

/// A blade count built from a non-zero number of blades.
fn blade_count(count: NonZeroU8) -> Result<BladeCount, Fail> {
    BladeCount::new(count).or_fail()
}

/// A focus distance built from a non-zero number of scene units.
fn focus_distance(units: NonZeroU32) -> Result<FocusDistance, Fail> {
    FocusDistance::new(units).or_fail()
}

/// An f/2.8 iris with a seven-blade outline, focused at 17 scene units on a 1080 high frame,
/// sampled on a four by four grid.
fn base_iris() -> Result<Iris, Fail> {
    Ok(Iris::new(
        FStop::F2_8,
        ApertureShape::Polygon(blade_count(NonZeroU8::new(7).or_fail()?)?),
        Focus::At(focus_distance(NonZeroU32::new(17).or_fail()?)?),
        FrameHeight(NonZeroU16::new(1080).or_fail()?),
        ApertureGrid::Four,
    ))
}

/// The iris every single-field variant is derived from, followed by five irises that each differ
/// from it in exactly one field.
///
/// The order is f-number, outline, focus, frame height, and sampling grid.
fn iris_and_single_field_variants() -> Result<Vec<Iris>, Fail> {
    let base = base_iris()?;
    let stop = Iris::new(
        FStop::F4,
        base.shape(),
        base.focus(),
        base.frame_height(),
        base.samples(),
    );
    let shape = Iris::new(
        base.stop(),
        ApertureShape::Disc,
        base.focus(),
        base.frame_height(),
        base.samples(),
    );
    let focus = Iris::new(
        base.stop(),
        base.shape(),
        Focus::Infinity,
        base.frame_height(),
        base.samples(),
    );
    let height = Iris::new(
        base.stop(),
        base.shape(),
        base.focus(),
        FrameHeight(NonZeroU16::new(720).or_fail()?),
        base.samples(),
    );
    let samples = Iris::new(
        base.stop(),
        base.shape(),
        base.focus(),
        base.frame_height(),
        ApertureGrid::Eight,
    );
    Ok(vec![base, stop, shape, focus, height, samples])
}

#[test]
fn blade_count_round_trips_awkward_counts() {
    assert_eq!(check_blade_count_round_trips_awkward_counts(), Ok(()));
}

fn check_blade_count_round_trips_awkward_counts() -> Result<(), Fail> {
    ensure_eq!(
        blade_count(NonZeroU8::new(5).or_fail()?)?.get(),
        NonZeroU8::new(5).or_fail()?
    );
    ensure_eq!(
        blade_count(NonZeroU8::new(7).or_fail()?)?.get(),
        NonZeroU8::new(7).or_fail()?
    );
    Ok(())
}

#[test]
fn blade_count_rejects_two_and_seventeen() {
    assert_eq!(check_blade_count_rejects_two_and_seventeen(), Ok(()));
}

fn check_blade_count_rejects_two_and_seventeen() -> Result<(), Fail> {
    let two = NonZeroU8::new(2).or_fail()?;
    ensure_eq!(BladeCount::new(two), Err(BladeCountError::BelowThree));
    let seventeen = NonZeroU8::new(17).or_fail()?;
    ensure_eq!(
        BladeCount::new(seventeen),
        Err(BladeCountError::AboveSixteen)
    );
    Ok(())
}

#[test]
fn blade_count_accepts_three_and_sixteen() {
    assert_eq!(check_blade_count_accepts_three_and_sixteen(), Ok(()));
}

fn check_blade_count_accepts_three_and_sixteen() -> Result<(), Fail> {
    ensure_eq!(
        blade_count(NonZeroU8::new(3).or_fail()?)?.get(),
        NonZeroU8::new(3).or_fail()?
    );
    ensure_eq!(
        blade_count(NonZeroU8::new(16).or_fail()?)?.get(),
        NonZeroU8::new(16).or_fail()?
    );
    Ok(())
}

#[test]
fn fstop_nominal_values_are_exact() {
    assert_eq!(check_fstop_nominal_values_are_exact(), Ok(()));
}

fn check_fstop_nominal_values_are_exact() -> Result<(), Fail> {
    ensure_eq!(FStop::F1_4.nominal().or_fail()?, Frac(7, 5).ratio()?);
    ensure_eq!(FStop::F2.nominal().or_fail()?, Frac(2, 1).ratio()?);
    ensure_eq!(FStop::F2_8.nominal().or_fail()?, Frac(14, 5).ratio()?);
    ensure_eq!(FStop::F4.nominal().or_fail()?, Frac(4, 1).ratio()?);
    ensure_eq!(FStop::F5_6.nominal().or_fail()?, Frac(28, 5).ratio()?);
    ensure_eq!(FStop::F8.nominal().or_fail()?, Frac(8, 1).ratio()?);
    ensure_eq!(FStop::F11.nominal().or_fail()?, Frac(11, 1).ratio()?);
    ensure_eq!(FStop::F16.nominal().or_fail()?, Frac(16, 1).ratio()?);
    ensure_eq!(FStop::F22.nominal().or_fail()?, Frac(22, 1).ratio()?);
    Ok(())
}

#[test]
fn fstop_variants_are_distinguishable() {
    assert_eq!(check_fstop_variants_are_distinguishable(), Ok(()));
}

fn check_fstop_variants_are_distinguishable() -> Result<(), Fail> {
    ensure_ne!(FStop::F2_8, FStop::F4);
    ensure_ne!(FStop::F1_4, FStop::F22);
    ensure_eq!(FStop::F2_8, FStop::F2_8);
    Ok(())
}

#[test]
fn aperture_shape_separates_disc_from_polygon() {
    assert_eq!(check_aperture_shape_separates_disc_from_polygon(), Ok(()));
}

fn check_aperture_shape_separates_disc_from_polygon() -> Result<(), Fail> {
    let disc = ApertureShape::Disc;
    let five = ApertureShape::Polygon(blade_count(NonZeroU8::new(5).or_fail()?)?);
    let seven = ApertureShape::Polygon(blade_count(NonZeroU8::new(7).or_fail()?)?);
    ensure_ne!(disc, five);
    ensure_ne!(five, seven);
    let is_disc = match disc {
        ApertureShape::Disc => true,
        ApertureShape::Polygon(_) => false,
    };
    ensure!(is_disc, "the round iris must match the disc variant");
    Ok(())
}

#[test]
fn iris_round_trips_each_field() {
    assert_eq!(check_iris_round_trips_each_field(), Ok(()));
}

fn check_iris_round_trips_each_field() -> Result<(), Fail> {
    let iris = base_iris()?;
    ensure_eq!(iris.stop(), FStop::F2_8);
    ensure_eq!(
        iris.shape(),
        ApertureShape::Polygon(blade_count(NonZeroU8::new(7).or_fail()?)?),
        "the blade count must survive the iris"
    );
    ensure_eq!(
        iris.focus(),
        Focus::At(focus_distance(NonZeroU32::new(17).or_fail()?)?)
    );
    ensure_eq!(
        iris.frame_height(),
        FrameHeight(NonZeroU16::new(1080).or_fail()?)
    );
    ensure_eq!(iris.samples(), ApertureGrid::Four);
    Ok(())
}

#[test]
fn iris_differing_in_one_field_is_unequal() {
    assert_eq!(check_iris_differing_in_one_field_is_unequal(), Ok(()));
}

fn check_iris_differing_in_one_field_is_unequal() -> Result<(), Fail> {
    let variants = iris_and_single_field_variants()?;
    let (base, rest) = variants.split_first().or_fail()?;
    ensure_eq!(rest.len(), 5, "one variant per field");
    for variant in rest {
        ensure_ne!(base, variant);
    }
    Ok(())
}

#[test]
fn aperture_grid_variants_are_distinguishable() {
    assert_eq!(check_aperture_grid_variants_are_distinguishable(), Ok(()));
}

fn check_aperture_grid_variants_are_distinguishable() -> Result<(), Fail> {
    ensure_ne!(ApertureGrid::Two, ApertureGrid::Four);
    ensure_ne!(ApertureGrid::Four, ApertureGrid::Eight);
    Ok(())
}

#[test]
fn pinhole_differs_from_every_iris() {
    assert_eq!(check_pinhole_differs_from_every_iris(), Ok(()));
}

fn check_pinhole_differs_from_every_iris() -> Result<(), Fail> {
    for iris in iris_and_single_field_variants()? {
        ensure_ne!(Aperture::Pinhole, Aperture::Iris(iris));
    }
    let is_pinhole = match Aperture::Pinhole {
        Aperture::Pinhole => true,
        Aperture::Iris(_) => false,
    };
    ensure!(is_pinhole, "the pinhole must match the pinhole variant");
    Ok(())
}
