use super::*;
use crate::optics::{ApertureGrid, ApertureShape, FStop, Focus, FrameHeight};
use core::num::NonZeroU16;
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

/// An f/2 iris with a disc outline, focused at infinity, with frame height 24 and the given grid.
fn iris(grid: ApertureGrid) -> Result<Iris, Fail> {
    Ok(Iris::new(
        FStop::F2,
        ApertureShape::Disc,
        Focus::Infinity,
        FrameHeight(NonZeroU16::new(24).or_fail()?),
        grid,
    ))
}

#[test]
fn points_are_taken_on_the_grid_of_their_iris() {
    assert_eq!(check_points_are_taken_on_the_grid_of_their_iris(), Ok(()));
}

fn check_points_are_taken_on_the_grid_of_their_iris() -> Result<(), Fail> {
    let four = iris(ApertureGrid::Four)?;
    let paired = IrisPoints::new(four).or_fail()?;
    ensure_eq!(paired.iris(), four);
    ensure_eq!(paired.points().grid(), ApertureGrid::Four);
    Ok(())
}
