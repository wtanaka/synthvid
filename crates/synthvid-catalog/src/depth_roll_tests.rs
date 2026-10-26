//! Rendered checks of the roll shot: the sensels its red rectangle covers change as the camera
//! turns.
//!
//! The sensel `(c, r)` has its center at `(c + 1/2, r + 1/2)` in lattice sensels, and the frame
//! center is at `(16, 12)`. The roll entry is 32 by 24 sensels, so a point at across `X`, down
//! `Y` and depth `z` lands `24 * X / z` and `24 * Y / z` sensels from the center before the roll.
//! The renderer rotates those offsets by the cosine and sine of the negated roll angle `-theta`
//! (`deep/roll.rs`), written `cos` and `sin` here. The across offset becomes `cos * X - sin * Y`
//! and the down offset becomes `sin * X + cos * Y`. A roll of a quarter turn therefore maps
//! `(10, -5)` to `(-5, -10)`, and at the seventh frame of the roll shot the sine is negative.

use crate::cover::{generate_catalogue, DeepAxis, Entry};
use synthvid_scene::frame::{PixelColumn, PixelCoord, PixelRow};
use synthvid_scene::{Channel, Frame, FrameIndex, Rgb8};
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};
use synthvid_video::deep::render_stage_into;
use synthvid_video::{Shot, VideoSpec};

/// One rendered frame of the roll shot, with the backdrop color of its stage.
struct RollFrame {
    /// The rendered frame.
    frame: Frame,
    /// The backdrop color of the stage.
    backdrop: Rgb8,
}

/// The red rectangle's color.
const RED: Rgb8 = Rgb8 {
    red: Channel(255),
    green: Channel(0),
    blue: Channel(0),
};

/// The roll entry, the deep shot of the roll effect.
fn roll_entry() -> Result<Entry, Fail> {
    generate_catalogue()
        .or_fail()?
        .into_iter()
        .find(|entry| entry.depth() == Some(DeepAxis::Roll))
        .or_fail()
}

/// Renders the roll shot at the given frame.
fn render_roll_frame(index: FrameIndex) -> Result<RollFrame, Fail> {
    let spec: VideoSpec = roll_entry()?.to_video_spec().or_fail()?;
    let deep = match spec.shot() {
        Shot::Flat(_) => {
            return Err(Fail::here(format_args!(
                "a roll entry must record a deep shot"
            )))
        }
        Shot::Deep(deep) => deep.as_ref(),
    };
    let mut frame = Frame::zeroed(spec.recorder().dimensions()).or_fail()?;
    render_stage_into(deep, index, &mut frame).or_fail()?;
    Ok(RollFrame {
        frame,
        backdrop: deep.stage().backdrop,
    })
}

/// The color of the sensel at the given column and row of a rendered frame.
fn color_at(rendered: &RollFrame, column: PixelColumn, row: PixelRow) -> Result<Rgb8, Fail> {
    rendered
        .frame
        .get_pixel(PixelCoord::new(column, row))
        .or_fail()
}

// The red rectangle is 120 by 60 units at across -100, down 0, depth 400. At depth 400 the scale
// is 24 / 400 = 3/50, so its center is at (16 - 6, 12) = (10, 12) with half sizes 60 * 3/50 = 3.6
// across and 30 * 3/50 = 1.8 down. At frame 0 the turn is zero, so it covers across 6.4 to 13.6
// and down 10.2 to 13.8.
//
// At frame 7 the turn is 7/32 of a turn, which is 78.75 degrees. Its cosine is 0.1951 and its
// sine is 0.9808, rounded by hand rather than read from `synthvid_scene::trig`. The chosen
// sensels lie far from every boundary, so the rounding decides none of the assertions. The
// offset (-6, 0) becomes (-6 * 0.1951, 6 * 0.9808) = (-1.170, 5.885). The
// rectangle's center moves to (14.830, 17.885). A rotation is rigid, so the rectangle lies within
// its circumradius of that center, sqrt(3.6^2 + 1.8^2) = 4.03.
//
// The green triangle is at across 150, depth 800, so its scale is 24 / 800 = 0.03 and its anchor
// is at (16 + 4.5, 12) = (20.5, 12). Its vertices are offsets (0, -3.6), (3.6, 1.8) and
// (-3, 1.8), so at frame 0 it covers across 17.5 to 24.1 and down 8.4 to 13.8. At frame 7 its
// anchor is at (16.88, 7.59) and every vertex lies within 3.6 of it.

/// Sensel (9, 12) has its center at (9.5, 12.5), which is inside the frame 0 rectangle by at
/// least 1.3 sensels in every direction, so the sensel is red at frame 0. At frame 7 its
/// distance from the rectangle's center is sqrt(5.33^2 + 5.38^2) = 7.57, above the 4.03
/// circumradius, and it is at least 8.8 from the triangle's anchor, above its 3.6 reach, so it
/// shows the backdrop.
fn check_red_sensel_at_frame_zero_is_backdrop_at_frame_seven() -> Result<(), Fail> {
    let first = render_roll_frame(FrameIndex::new(0))?;
    let seventh = render_roll_frame(FrameIndex::new(7))?;
    ensure_eq!(color_at(&first, PixelColumn(9), PixelRow(12))?, RED);
    ensure_eq!(
        color_at(&seventh, PixelColumn(9), PixelRow(12))?,
        seventh.backdrop
    );
    Ok(())
}

#[test]
fn red_sensel_at_frame_zero_is_backdrop_at_frame_seven() {
    assert_eq!(
        check_red_sensel_at_frame_zero_is_backdrop_at_frame_seven(),
        Ok(())
    );
}

// Sensel (14, 17) has its center at (14.5, 17.5). At frame 0 it lies right of the rectangle's
// edge at 13.6 and below its edge at 13.8, and right of the triangle's box, which starts at 17.5
// across, so it shows the backdrop. At frame 7 its distance from the rectangle's center is
// sqrt(0.33^2 + 0.38^2) = 0.50, well inside the 1.8 inscribed radius, and its quarter-sensel
// sample offsets stay within 0.86 of the center, so it is red. It is about 10 from the triangle's
// anchor, so the triangle does not cover it.

/// Sensel (14, 17) is backdrop at frame 0 and red at frame 7.
fn check_backdrop_sensel_at_frame_zero_is_red_at_frame_seven() -> Result<(), Fail> {
    let first = render_roll_frame(FrameIndex::new(0))?;
    let seventh = render_roll_frame(FrameIndex::new(7))?;
    ensure_eq!(
        color_at(&first, PixelColumn(14), PixelRow(17))?,
        first.backdrop
    );
    ensure_eq!(color_at(&seventh, PixelColumn(14), PixelRow(17))?, RED);
    Ok(())
}

#[test]
fn backdrop_sensel_at_frame_zero_is_red_at_frame_seven() {
    assert_eq!(
        check_backdrop_sensel_at_frame_zero_is_red_at_frame_seven(),
        Ok(())
    );
}
