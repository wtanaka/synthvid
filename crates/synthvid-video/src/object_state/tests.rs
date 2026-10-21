use super::*;
use crate::testutil::Frac;
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

/// An axis-aligned box written as its corners `(min_x, min_y, max_x, max_y)`.
#[derive(Copy, Clone, Debug)]
struct Corners4(Frac, Frac, Frac, Frac);

impl Corners4 {
    /// Builds the bounds these corners name.
    fn bounds(self) -> Result<Bounds, Fail> {
        let Self(min_x, min_y, max_x, max_y) = self;
        Bounds::new(Corners {
            min: Point {
                across: min_x.ratio()?,
                down: min_y.ratio()?,
            },
            max: Point {
                across: max_x.ratio()?,
                down: max_y.ratio()?,
            },
        })
        .or_fail()
    }
}

/// The frame [0, 4] x [0, 4].
fn frame_4x4() -> Result<FrameBounds, Fail> {
    Ok(FrameBounds(
        Corners4(Frac(0, 1), Frac(0, 1), Frac(4, 1), Frac(4, 1)).bounds()?,
    ))
}

fn check_visible_fraction_partial_overlap() -> Result<(), Fail> {
    let bbox = Corners4(Frac(-1, 1), Frac(0, 1), Frac(3, 1), Frac(2, 1)).bounds()?;
    ensure_eq!(
        visible_fraction(bbox, frame_4x4()?),
        Ok(Frac(3, 4).ratio()?)
    );
    Ok(())
}

/// bbox [-1, 3] x [0, 2] (area 8) against the frame [0, 4] x [0, 4]:
/// the overlap is [0, 3] x [0, 2], area 6, so 6 / 8 = 3 / 4.
#[test]
fn visible_fraction_partial_overlap() {
    assert_eq!(check_visible_fraction_partial_overlap(), Ok(()));
}

fn check_visible_fraction_disjoint_and_degenerate_are_zero() -> Result<(), Fail> {
    let frame = frame_4x4()?;
    let away = Corners4(Frac(5, 1), Frac(0, 1), Frac(7, 1), Frac(2, 1)).bounds()?;
    ensure_eq!(visible_fraction(away, frame), Ok(Frac(0, 1).ratio()?));
    let flat = Corners4(Frac(1, 1), Frac(1, 1), Frac(3, 1), Frac(1, 1)).bounds()?;
    ensure_eq!(visible_fraction(flat, frame), Ok(Frac(0, 1).ratio()?));
    Ok(())
}

/// A box entirely outside the frame, and a box with no area, are both
/// not on screen, and neither is an error.
#[test]
fn visible_fraction_disjoint_and_degenerate_are_zero() {
    assert_eq!(
        check_visible_fraction_disjoint_and_degenerate_are_zero(),
        Ok(())
    );
}
