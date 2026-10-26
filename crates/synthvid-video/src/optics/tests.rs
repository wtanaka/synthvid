use super::*;
use crate::testutil::Frac;
use synthvid_testkit::{ensure, ensure_eq, ensure_ne, Fail, OrFail as _};

/// A focus distance built from a non-zero number of scene units.
fn focus_distance(units: NonZeroU32) -> Result<FocusDistance, Fail> {
    FocusDistance::new(units).or_fail()
}

/// A vignette strength built from a non-zero raw [`Q8_8`] value.
fn vignette_strength(raw: NonZeroU16) -> Result<VignetteStrength, Fail> {
    VignetteStrength::new(raw).or_fail()
}

/// An f/2.8 iris with a seven-blade outline, focused at 17 scene units on a 1080 high frame,
/// sampled on a four by four grid.
fn base_iris() -> Result<Iris, Fail> {
    let blades = BladeCount::new(NonZeroU8::new(7).or_fail()?).or_fail()?;
    Ok(Iris::new(
        FStop::F2_8,
        ApertureShape::Polygon(blades),
        Focus::At(focus_distance(NonZeroU32::new(17).or_fail()?)?),
        FrameHeight(NonZeroU16::new(1080).or_fail()?),
        ApertureGrid::Four,
    ))
}

/// The lens every single-field variant is derived from: a focal length of 24 sixteenths, the
/// base iris, and a vignette with a raw strength of 64.
fn base_lens() -> Result<Lens, Fail> {
    Ok(Lens::new(
        FocalLength(NonZeroU8::new(24).or_fail()?),
        Aperture::Iris(base_iris()?),
        Vignette::Radial(vignette_strength(NonZeroU16::new(64).or_fail()?)?),
    ))
}

/// The base lens followed by three lenses that each differ from it in exactly one field.
///
/// The order is focal length, aperture, and vignette.
fn lens_and_single_field_variants() -> Result<Vec<Lens>, Fail> {
    let base = base_lens()?;
    let focal = Lens::new(
        FocalLength(NonZeroU8::new(25).or_fail()?),
        base.aperture(),
        base.vignette(),
    );
    let aperture = Lens::new(base.focal_length(), Aperture::Pinhole, base.vignette());
    let vignette = Lens::new(base.focal_length(), base.aperture(), Vignette::None);
    Ok(vec![base, focal, aperture, vignette])
}

#[test]
fn focal_length_counts_sixteenths_of_a_frame_height() {
    assert_eq!(
        check_focal_length_counts_sixteenths_of_a_frame_height(),
        Ok(())
    );
}

fn check_focal_length_counts_sixteenths_of_a_frame_height() -> Result<(), Fail> {
    ensure_eq!(
        FocalLength(NonZeroU8::new(24).or_fail()?)
            .ratio()
            .or_fail()?,
        Frac(3, 2).ratio()?
    );
    ensure_eq!(
        FocalLength(NonZeroU8::new(1).or_fail()?)
            .ratio()
            .or_fail()?,
        Frac(1, 16).ratio()?
    );
    ensure_eq!(
        FocalLength(NonZeroU8::new(255).or_fail()?)
            .ratio()
            .or_fail()?,
        Frac(255, 16).ratio()?
    );
    Ok(())
}

#[test]
fn focus_distance_refuses_above_two_to_the_twentieth() {
    assert_eq!(
        check_focus_distance_refuses_above_two_to_the_twentieth(),
        Ok(())
    );
}

fn check_focus_distance_refuses_above_two_to_the_twentieth() -> Result<(), Fail> {
    let beyond = NonZeroU32::new(0x0010_0001).or_fail()?;
    ensure_eq!(FocusDistance::new(beyond), Err(FocusDistanceError::TooFar));
    Ok(())
}

#[test]
fn focus_distance_accepts_two_to_the_twentieth_and_round_trips() {
    assert_eq!(
        check_focus_distance_accepts_two_to_the_twentieth_and_round_trips(),
        Ok(())
    );
}

fn check_focus_distance_accepts_two_to_the_twentieth_and_round_trips() -> Result<(), Fail> {
    let limit = NonZeroU32::new(0x0010_0000).or_fail()?;
    ensure_eq!(focus_distance(limit)?.get(), limit);
    let near = NonZeroU32::new(17).or_fail()?;
    ensure_eq!(focus_distance(near)?.get(), near);
    Ok(())
}

#[test]
fn focus_separates_infinity_from_a_distance() {
    assert_eq!(check_focus_separates_infinity_from_a_distance(), Ok(()));
}

fn check_focus_separates_infinity_from_a_distance() -> Result<(), Fail> {
    let at = Focus::At(focus_distance(NonZeroU32::new(17).or_fail()?)?);
    ensure_ne!(Focus::Infinity, at);
    let is_infinity = match Focus::Infinity {
        Focus::Infinity => true,
        Focus::At(_) => false,
    };
    ensure!(
        is_infinity,
        "focus at infinity must match the infinity variant"
    );
    Ok(())
}

#[test]
fn vignette_strength_rejects_above_one() {
    assert_eq!(check_vignette_strength_rejects_above_one(), Ok(()));
}

fn check_vignette_strength_rejects_above_one() -> Result<(), Fail> {
    let raw = NonZeroU16::new(257).or_fail()?;
    ensure_eq!(
        VignetteStrength::new(raw),
        Err(VignetteStrengthError::AboveOne)
    );
    Ok(())
}

#[test]
fn vignette_strength_accepts_one_and_the_awkward_middle() {
    assert_eq!(
        check_vignette_strength_accepts_one_and_the_awkward_middle(),
        Ok(())
    );
}

fn check_vignette_strength_accepts_one_and_the_awkward_middle() -> Result<(), Fail> {
    ensure_eq!(
        vignette_strength(NonZeroU16::new(1).or_fail()?)?.get(),
        Q8_8(1)
    );
    ensure_eq!(
        vignette_strength(NonZeroU16::new(256).or_fail()?)?.get(),
        Q8_8(256),
        "1.0 is legal"
    );
    ensure_eq!(
        vignette_strength(NonZeroU16::new(64).or_fail()?)?.get(),
        Q8_8(64)
    );
    Ok(())
}

#[test]
fn vignette_separates_none_from_radial() {
    assert_eq!(check_vignette_separates_none_from_radial(), Ok(()));
}

fn check_vignette_separates_none_from_radial() -> Result<(), Fail> {
    let radial = Vignette::Radial(vignette_strength(NonZeroU16::new(64).or_fail()?)?);
    ensure_ne!(Vignette::None, radial);
    let is_none = match Vignette::None {
        Vignette::None => true,
        Vignette::Radial(_) => false,
    };
    ensure!(is_none, "the absent vignette must match the none variant");
    Ok(())
}

#[test]
fn lens_round_trips_each_field() {
    assert_eq!(check_lens_round_trips_each_field(), Ok(()));
}

fn check_lens_round_trips_each_field() -> Result<(), Fail> {
    let lens = base_lens()?;
    ensure_eq!(
        lens.focal_length(),
        FocalLength(NonZeroU8::new(24).or_fail()?)
    );
    ensure_eq!(lens.aperture(), Aperture::Iris(base_iris()?));
    ensure_eq!(
        lens.vignette(),
        Vignette::Radial(vignette_strength(NonZeroU16::new(64).or_fail()?)?)
    );
    Ok(())
}

#[test]
fn lens_differing_in_one_field_is_unequal() {
    assert_eq!(check_lens_differing_in_one_field_is_unequal(), Ok(()));
}

fn check_lens_differing_in_one_field_is_unequal() -> Result<(), Fail> {
    let variants = lens_and_single_field_variants()?;
    let (base, rest) = variants.split_first().or_fail()?;
    ensure_eq!(rest.len(), 3, "one variant per field");
    for variant in rest {
        ensure_ne!(base, variant);
    }
    Ok(())
}

#[test]
fn optics_off_differs_from_a_vignette() {
    assert_eq!(check_optics_off_differs_from_a_vignette(), Ok(()));
}

fn check_optics_off_differs_from_a_vignette() -> Result<(), Fail> {
    let strength = vignette_strength(NonZeroU16::new(128).or_fail()?)?;
    ensure_ne!(Optics::Off, Optics::Vignette(strength));
    let is_off = match Optics::Off {
        Optics::Off => true,
        Optics::Vignette(_) => false,
    };
    ensure!(is_off, "the ideal lens must match the off variant");
    Ok(())
}

#[test]
fn optics_vignette_differs_by_strength() {
    assert_eq!(check_optics_vignette_differs_by_strength(), Ok(()));
}

fn check_optics_vignette_differs_by_strength() -> Result<(), Fail> {
    let half = vignette_strength(NonZeroU16::new(128).or_fail()?)?;
    let quarter = vignette_strength(NonZeroU16::new(64).or_fail()?)?;
    ensure_eq!(Optics::Vignette(half), Optics::Vignette(half));
    ensure_ne!(Optics::Vignette(half), Optics::Vignette(quarter));
    Ok(())
}
