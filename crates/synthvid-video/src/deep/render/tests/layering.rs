use super::*;

/// A red square of half size one at depth eight.
fn near_red() -> Result<Layer, Fail> {
    layer(
        square(NonZeroU32::MIN)?,
        red(),
        fixed(on_axis(RawCoord(8)))?,
    )
}

/// A blue square of half size four at depth sixteen, larger and farther from the camera.
fn far_blue() -> Result<Layer, Fail> {
    layer(
        square(NonZeroU32::new(4).or_fail()?)?,
        blue(),
        fixed(on_axis(RawCoord(16)))?,
    )
}

/// A blue square of half size one at depth eight, the same size and depth as the red square.
fn same_depth_blue() -> Result<Layer, Fail> {
    layer(
        square(NonZeroU32::MIN)?,
        blue(),
        fixed(on_axis(RawCoord(8)))?,
    )
}

/// The color expected at a sensel when the red square is in front of the blue one.
fn occlusion_color(cell: Cell) -> Rgb8 {
    match block_at(three_to_four(), cell) {
        Containment::Inside => red(),
        Containment::Outside => match block_at(two_to_five(), cell) {
            Containment::Inside => blue(),
            Containment::Outside => black(),
        },
    }
}

#[test]
fn nearer_layer_occludes_the_farther_layer_in_either_declaration_order() {
    assert_eq!(check_nearer_layer_occludes_the_farther_layer(), Ok(()));
}

fn check_nearer_layer_occludes_the_farther_layer() -> Result<(), Fail> {
    let near_first = shot(
        lens(Aperture::Pinhole)?,
        PixelGrid::One,
        black(),
        vec![near_red()?, far_blue()?],
    )?;
    let far_first = shot(
        lens(Aperture::Pinhole)?,
        PixelGrid::One,
        black(),
        vec![far_blue()?, near_red()?],
    )?;
    let forward = render_at(&near_first, FrameIndex::new(0))?;
    let reversed = render_at(&far_first, FrameIndex::new(0))?;
    ensure_eq!(forward, reversed);
    ensure_every_sensel(&forward, occlusion_color)
}

#[test]
fn later_declared_layer_is_in_front_at_equal_depth() {
    assert_eq!(
        check_later_declared_layer_is_in_front_at_equal_depth(),
        Ok(())
    );
}

fn check_later_declared_layer_is_in_front_at_equal_depth() -> Result<(), Fail> {
    let scene = shot(
        lens(Aperture::Pinhole)?,
        PixelGrid::One,
        black(),
        vec![near_red()?, same_depth_blue()?],
    )?;
    let frame = render_at(&scene, FrameIndex::new(0))?;
    ensure_every_sensel(&frame, |cell| match block_at(three_to_four(), cell) {
        Containment::Inside => blue(),
        Containment::Outside => black(),
    })
}
