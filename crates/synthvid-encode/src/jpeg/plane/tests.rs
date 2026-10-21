use super::*;
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

#[test]
fn plane_downsample() {
    assert_eq!(check_plane_downsample(), Ok(()));
}

fn check_plane_downsample() -> Result<(), Fail> {
    let width_2 = Width::new(2).or_fail()?;
    let height_2 = Height::new(2).or_fail()?;
    let mut plane = Plane::filled(PlaneDimensions::new(width_2, height_2), Component::Chroma);
    plane.data_mut().0 = vec![100, 101, 101, 101];
    let downsampled = plane.downsample_2x2();
    ensure_eq!(
        (
            downsampled.dims.width().get().get(),
            downsampled.dims.height().get().get(),
            downsampled.data.0.as_slice()
        ),
        (1, 1, [101_u8].as_slice())
    );
    Ok(())
}
