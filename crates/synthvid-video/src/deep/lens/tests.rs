use super::*;
use crate::optics::{ApertureShape, FStop, FocusDistance};
use core::num::{NonZeroU16, NonZeroU32, NonZeroU8};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// A focus at the given distance in scene units.
fn focus_at(units: NonZeroU32) -> Result<Focus, Fail> {
    Ok(Focus::At(FocusDistance::new(units).or_fail()?))
}

/// An iris with a disc outline, the given focus, stop, sampling grid, and frame height.
fn iris(focus: Focus, stop: FStop, grid: ApertureGrid, frame_height: NonZeroU16) -> Iris {
    Iris::new(
        stop,
        ApertureShape::Disc,
        focus,
        FrameHeight(frame_height),
        grid,
    )
}

/// A lens sample from its across and down numerators on a grid.
fn sample(numerators: LensNumerators, grid: ApertureGrid) -> Result<LensSample, Fail> {
    LensSample::new(numerators, grid).or_fail()
}

/// The shift input for an f/2 iris with frame height 24, a 48 sensel frame, a focal length of 16
/// sixteenths, and a layer at the given depth.
fn f2_input(
    focus: Focus,
    depth: NonZeroU32,
    lens_sample: LensSample,
) -> Result<LensShiftInput, Fail> {
    Ok(LensShiftInput::from_iris_sample(
        iris(
            focus,
            FStop::F2,
            ApertureGrid::Two,
            NonZeroU16::new(24).or_fail()?,
        ),
        FocalLength(NonZeroU8::new(16).or_fail()?),
        Height(NonZeroU16::new(48).or_fail()?),
        Depth(depth),
        lens_sample,
    ))
}

/// The sample at the across numerator 1 and the down numerator 1, on the two by two grid.
fn unit_sample() -> Result<LensSample, Fail> {
    sample(
        LensNumerators {
            across: LensNumerator(1),
            down: LensNumerator(1),
        },
        ApertureGrid::Two,
    )
}

#[test]
fn focus_at_200_on_depth_100_shifts_by_minus_184_point_32() {
    assert_eq!(
        check_focus_at_200_on_depth_100_shifts_by_minus_184_point_32(),
        Ok(())
    );
}

fn check_focus_at_200_on_depth_100_shifts_by_minus_184_point_32() -> Result<(), Fail> {
    let focus = focus_at(NonZeroU32::new(200).or_fail()?)?;
    let shift = lens_shift(f2_input(
        focus,
        NonZeroU32::new(100).or_fail()?,
        unit_sample()?,
    )?);
    ensure_eq!(
        shift,
        Ok(LatticeShift {
            across: SubSensel(-184),
            down: SubSensel(-184),
        })
    );
    Ok(())
}

#[test]
fn focus_at_infinity_on_depth_100_shifts_by_minus_368_point_64() {
    assert_eq!(
        check_focus_at_infinity_on_depth_100_shifts_by_minus_368_point_64(),
        Ok(())
    );
}

fn check_focus_at_infinity_on_depth_100_shifts_by_minus_368_point_64() -> Result<(), Fail> {
    let shift = lens_shift(f2_input(
        Focus::Infinity,
        NonZeroU32::new(100).or_fail()?,
        unit_sample()?,
    )?);
    ensure_eq!(
        shift,
        Ok(LatticeShift {
            across: SubSensel(-369),
            down: SubSensel(-369),
        })
    );
    Ok(())
}

#[test]
fn focus_at_200_on_depth_400_shifts_by_plus_92_point_16() {
    assert_eq!(
        check_focus_at_200_on_depth_400_shifts_by_plus_92_point_16(),
        Ok(())
    );
}

fn check_focus_at_200_on_depth_400_shifts_by_plus_92_point_16() -> Result<(), Fail> {
    let focus = focus_at(NonZeroU32::new(200).or_fail()?)?;
    let shift = lens_shift(f2_input(
        focus,
        NonZeroU32::new(400).or_fail()?,
        unit_sample()?,
    )?);
    ensure_eq!(
        shift,
        Ok(LatticeShift {
            across: SubSensel(92),
            down: SubSensel(92),
        })
    );
    Ok(())
}

#[test]
fn layer_on_the_plane_of_focus_has_no_shift() {
    assert_eq!(check_layer_on_the_plane_of_focus_has_no_shift(), Ok(()));
}

fn check_layer_on_the_plane_of_focus_has_no_shift() -> Result<(), Fail> {
    let focus = focus_at(NonZeroU32::new(200).or_fail()?)?;
    let shift = lens_shift(f2_input(
        focus,
        NonZeroU32::new(200).or_fail()?,
        unit_sample()?,
    )?);
    ensure_eq!(
        shift,
        Ok(LatticeShift {
            across: SubSensel(0),
            down: SubSensel(0),
        })
    );
    Ok(())
}

#[test]
fn opposite_sample_gives_the_negated_shift() {
    assert_eq!(check_opposite_sample_gives_the_negated_shift(), Ok(()));
}

fn check_opposite_sample_gives_the_negated_shift() -> Result<(), Fail> {
    let focus = focus_at(NonZeroU32::new(200).or_fail()?)?;
    let opposite = sample(
        LensNumerators {
            across: LensNumerator(-1),
            down: LensNumerator(-1),
        },
        ApertureGrid::Two,
    )?;
    let shift = lens_shift(f2_input(focus, NonZeroU32::new(100).or_fail()?, opposite)?);
    ensure_eq!(
        shift,
        Ok(LatticeShift {
            across: SubSensel(184),
            down: SubSensel(184),
        })
    );
    let infinite = lens_shift(f2_input(
        Focus::Infinity,
        NonZeroU32::new(100).or_fail()?,
        opposite,
    )?);
    ensure_eq!(
        infinite,
        Ok(LatticeShift {
            across: SubSensel(369),
            down: SubSensel(369),
        })
    );
    let behind = lens_shift(f2_input(focus, NonZeroU32::new(400).or_fail()?, opposite)?);
    ensure_eq!(
        behind,
        Ok(LatticeShift {
            across: SubSensel(-92),
            down: SubSensel(-92),
        })
    );
    Ok(())
}

#[test]
fn axes_take_their_own_numerators() {
    assert_eq!(check_axes_take_their_own_numerators(), Ok(()));
}

fn check_axes_take_their_own_numerators() -> Result<(), Fail> {
    let focus = focus_at(NonZeroU32::new(200).or_fail()?)?;
    let mixed = sample(
        LensNumerators {
            across: LensNumerator(1),
            down: LensNumerator(-1),
        },
        ApertureGrid::Two,
    )?;
    let shift = lens_shift(f2_input(focus, NonZeroU32::new(100).or_fail()?, mixed)?);
    ensure_eq!(
        shift,
        Ok(LatticeShift {
            across: SubSensel(-184),
            down: SubSensel(184),
        })
    );
    Ok(())
}

#[test]
fn even_numerator_is_refused_when_the_sample_is_formed() {
    assert_eq!(
        check_even_numerator_is_refused_when_the_sample_is_formed(),
        Ok(())
    );
}

fn check_even_numerator_is_refused_when_the_sample_is_formed() -> Result<(), Fail> {
    let even = LensSample::new(
        LensNumerators {
            across: LensNumerator(0),
            down: LensNumerator(1),
        },
        ApertureGrid::Two,
    );
    ensure_eq!(even, Err(LensSampleError::EvenNumerator));
    Ok(())
}

#[test]
fn numerator_beyond_the_aperture_is_refused_when_the_sample_is_formed() {
    assert_eq!(
        check_numerator_beyond_the_aperture_is_refused_when_the_sample_is_formed(),
        Ok(())
    );
}

fn check_numerator_beyond_the_aperture_is_refused_when_the_sample_is_formed() -> Result<(), Fail> {
    let beyond = LensSample::new(
        LensNumerators {
            across: LensNumerator(3),
            down: LensNumerator(1),
        },
        ApertureGrid::Two,
    );
    ensure_eq!(beyond, Err(LensSampleError::OutsideAperture));
    let last = LensSample::new(
        LensNumerators {
            across: LensNumerator(7),
            down: LensNumerator(-7),
        },
        ApertureGrid::Eight,
    );
    ensure!(
        last.is_ok(),
        "the outermost numerators of an eight by eight grid are valid"
    );
    Ok(())
}

#[test]
fn extreme_inputs_are_refused_as_beyond_the_guard_band() {
    assert_eq!(
        check_extreme_inputs_are_refused_as_beyond_the_guard_band(),
        Ok(())
    );
}

fn check_extreme_inputs_are_refused_as_beyond_the_guard_band() -> Result<(), Fail> {
    let focus = focus_at(NonZeroU32::new(0x0010_0000).or_fail()?)?;
    let extreme = LensShiftInput::from_iris_sample(
        iris(focus, FStop::F5_6, ApertureGrid::Eight, NonZeroU16::MAX),
        FocalLength(NonZeroU8::MAX),
        Height(NonZeroU16::MAX),
        Depth(NonZeroU32::new(1).or_fail()?),
        sample(
            LensNumerators {
                across: LensNumerator(7),
                down: LensNumerator(7),
            },
            ApertureGrid::Eight,
        )?,
    );
    ensure_eq!(lens_shift(extreme), Err(ProjectionError::BeyondGuardBand));
    Ok(())
}

#[test]
fn aperture_count_of_the_shift_is_the_sample_grid() {
    assert_eq!(
        check_aperture_count_of_the_shift_is_the_sample_grid(),
        Ok(())
    );
}

fn check_aperture_count_of_the_shift_is_the_sample_grid() -> Result<(), Fail> {
    // The iris is f/2 on the two by two grid, with frame height H = 24, and the sample is the
    // eight by eight point with numerators 7 and -7. The count is A = 8, the grid of the sample.
    // The scale is h * H * fn^2 * Nd = 48 * 24 * 256 = 294912 for fn = 16, the depth is z = 100
    // and the focus is s = 200. The across shift is
    // -294912 * 7 * (200 - 100) / (2 * 2 * 8 * 100 * 200) = -206438400 / 640000 = -322.56,
    // which rounds half up to -323. The down numerator -7 gives +322.56, which rounds to 323.
    // A count of 2 taken from the iris would give -1290.24 instead.
    let focus = focus_at(NonZeroU32::new(200).or_fail()?)?;
    let eight_by_eight = sample(
        LensNumerators {
            across: LensNumerator(7),
            down: LensNumerator(-7),
        },
        ApertureGrid::Eight,
    )?;
    let shift = lens_shift(LensShiftInput::from_iris_sample(
        iris(
            focus,
            FStop::F2,
            ApertureGrid::Two,
            NonZeroU16::new(24).or_fail()?,
        ),
        FocalLength(NonZeroU8::new(16).or_fail()?),
        Height(NonZeroU16::new(48).or_fail()?),
        Depth(NonZeroU32::new(100).or_fail()?),
        eight_by_eight,
    ));
    ensure_eq!(
        shift,
        Ok(LatticeShift {
            across: SubSensel(-323),
            down: SubSensel(323),
        })
    );
    Ok(())
}
