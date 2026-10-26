use super::*;
use crate::deep::ApertureError;
use crate::rig::DeepShotError;

#[test]
fn disc_projects_its_center_and_radius() {
    assert_eq!(check_disc_projects_its_center_and_radius(), Ok(()));
}

fn check_disc_projects_its_center_and_radius() -> Result<(), Fail> {
    let disc = Cutout::Disc {
        radius: extent(NonZeroU32::new(3).or_fail()?)?,
    };
    let camera = shot(
        fixed(on_axis(RawCoord(0)))?,
        upright(),
        lens(Aperture::Pinhole)?,
        &recorder(wide_short()?)?,
        vec![layer(disc, white(), fixed(on_axis(RawCoord(100)))?)?],
    )?;
    let prepared = prepare_frame(&camera, FrameIndex::new(0)).or_fail()?;
    let found = first_layer(&prepared)?;
    let center_and_radius = match found.shape() {
        LatticeShape::Disc { center, radius } => Some((*center, *radius)),
        LatticeShape::Polygon(_) => None,
    };
    let (center, radius) = center_and_radius.or_fail()?;
    ensure_eq!(
        center,
        LatticePoint {
            across: SubSensel(8192),
            down: SubSensel(6144),
        }
    );
    ensure_eq!(radius, DiscRadius(369));
    ensure_eq!(
        covers(
            found.shape(),
            LatticePoint {
                across: SubSensel(8561),
                down: SubSensel(6144),
            }
        )
        .or_fail()?,
        Containment::Inside
    );
    ensure_eq!(
        covers(
            found.shape(),
            LatticePoint {
                across: SubSensel(8562),
                down: SubSensel(6144),
            }
        )
        .or_fail()?,
        Containment::Outside
    );
    Ok(())
}

#[test]
fn triangle_aperture_on_the_two_by_two_grid_is_refused() {
    assert_eq!(
        check_triangle_aperture_on_the_two_by_two_grid_is_refused(),
        Ok(())
    );
}

fn check_triangle_aperture_on_the_two_by_two_grid_is_refused() -> Result<(), Fail> {
    // The triangle keeps no sample whose reflection is also inside on the two by two grid, so the
    // shot is refused before any frame is prepared.
    let rig = Rig::new(
        &Pose::new(fixed(on_axis(RawCoord(0)))?, upright()),
        lens(triangle_iris()?)?,
        &recorder(eight_by_eight()?)?,
    );
    let stage = Stage {
        backdrop: black(),
        layers: vec![layer(
            minimal_rect()?,
            white(),
            fixed(on_axis(RawCoord(8)))?,
        )?],
    };
    ensure_eq!(
        DeepShot::new(stage, &rig, PixelGrid::One),
        Err(DeepShotError::Aperture(Box::new(
            ApertureError::NoSymmetricPoints
        )))
    );
    Ok(())
}

#[test]
fn point_beyond_the_guard_band_names_its_frame_and_layer() {
    assert_eq!(
        check_point_beyond_the_guard_band_names_its_frame_and_layer(),
        Ok(())
    );
}

fn check_point_beyond_the_guard_band_names_its_frame_and_layer() -> Result<(), Fail> {
    let far = Spot {
        across: RawCoord(0x0010_0000),
        down: RawCoord(0),
        depth: RawCoord(1),
    };
    let camera = shot(
        fixed(on_axis(RawCoord(0)))?,
        upright(),
        lens(Aperture::Pinhole)?,
        &recorder(eight_by_eight()?)?,
        vec![
            layer(minimal_rect()?, white(), fixed(on_axis(RawCoord(8)))?)?,
            layer(minimal_rect()?, white(), fixed(far)?)?,
        ],
    )?;
    ensure_eq!(
        prepare_frame(&camera, FrameIndex::new(0)),
        Err(DeepRenderError::Projection {
            frame: FrameIndex::new(0),
            layer: ObjectIndex::new(1),
            error: ProjectionError::BeyondGuardBand,
        })
    );
    Ok(())
}

#[test]
fn rect_one_thousand_units_wide_at_depth_one_is_refused_beyond_the_guard_band() {
    assert_eq!(
        check_rect_one_thousand_units_wide_at_depth_one_is_refused_beyond_the_guard_band(),
        Ok(())
    );
}

fn check_rect_one_thousand_units_wide_at_depth_one_is_refused_beyond_the_guard_band(
) -> Result<(), Fail> {
    // With a focal length of 16 sixteenths and a frame 1080 sensels high, the corner at across -1000
    // projects to 16 * 1080 * 16 * 1000 = 276480000 lattice units, which exceeds 2^28 = 268435456
    // before the frame center is added. The frame is refused rather than the layer being dropped.
    let far_rect = Cutout::Rect {
        half_width: extent(NonZeroU32::new(1000).or_fail()?)?,
        half_height: extent(NonZeroU32::MIN)?,
    };
    let camera = shot(
        fixed(on_axis(RawCoord(0)))?,
        upright(),
        lens(Aperture::Pinhole)?,
        &recorder(Dimensions::new(
            Width::new(8).or_fail()?,
            Height::new(1080).or_fail()?,
        ))?,
        vec![layer(far_rect, white(), fixed(on_axis(RawCoord(1)))?)?],
    )?;
    ensure_eq!(
        prepare_frame(&camera, FrameIndex::new(0)),
        Err(DeepRenderError::Projection {
            frame: FrameIndex::new(0),
            layer: ObjectIndex::new(0),
            error: ProjectionError::BeyondGuardBand,
        })
    );
    Ok(())
}

#[test]
fn a_frame_does_not_depend_on_the_frames_prepared_before_it() {
    assert_eq!(
        check_a_frame_does_not_depend_on_the_frames_prepared_before_it(),
        Ok(())
    );
}

fn check_a_frame_does_not_depend_on_the_frames_prepared_before_it() -> Result<(), Fail> {
    let camera = shot(
        fixed(on_axis(RawCoord(0)))?,
        upright(),
        lens(Aperture::Pinhole)?,
        &recorder(eight_by_eight()?)?,
        vec![layer(
            minimal_rect()?,
            white(),
            walking_across(on_axis(RawCoord(8)))?,
        )?],
    )?;
    let first = prepare_frame(&camera, FrameIndex::new(3)).or_fail()?;
    let again = prepare_frame(&camera, FrameIndex::new(3)).or_fail()?;
    ensure_eq!(first, again);

    ensure_eq!(
        prepare_frame(&camera, FrameIndex::new(0))
            .or_fail()?
            .layers()
            .len(),
        1
    );
    ensure_eq!(
        prepare_frame(&camera, FrameIndex::new(1))
            .or_fail()?
            .layers()
            .len(),
        1
    );
    ensure_eq!(
        prepare_frame(&camera, FrameIndex::new(2))
            .or_fail()?
            .layers()
            .len(),
        1
    );
    let later = prepare_frame(&camera, FrameIndex::new(3)).or_fail()?;
    ensure_eq!(later, first);

    let expected = LatticeBounds {
        low: LatticePoint {
            across: SubSensel(1536),
            down: SubSensel(768),
        },
        high: LatticePoint {
            across: SubSensel(2048),
            down: SubSensel(1280),
        },
    };
    ensure_eq!(first_layer(&first)?.bounds(), expected);
    Ok(())
}

#[test]
fn quarter_turn_roll_swaps_the_axes_of_a_rectangle() {
    assert_eq!(
        check_quarter_turn_roll_swaps_the_axes_of_a_rectangle(),
        Ok(())
    );
}

fn check_quarter_turn_roll_swaps_the_axes_of_a_rectangle() -> Result<(), Fail> {
    let camera = shot(
        fixed(on_axis(RawCoord(0)))?,
        quarter_turn()?,
        lens(Aperture::Pinhole)?,
        &recorder(eight_by_eight()?)?,
        vec![layer(wide_rect()?, white(), fixed(on_axis(RawCoord(8)))?)?],
    )?;
    let prepared = prepare_frame(&camera, FrameIndex::new(0)).or_fail()?;
    let expected = LatticeBounds {
        low: LatticePoint {
            across: SubSensel(768),
            down: SubSensel(512),
        },
        high: LatticePoint {
            across: SubSensel(1280),
            down: SubSensel(1536),
        },
    };
    ensure_eq!(first_layer(&prepared)?.bounds(), expected);
    Ok(())
}
