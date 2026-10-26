//! Pins the named constants of the depth and recording presets to the values their doc comments
//! give, so that a typo in a literal cannot change a preset without a test failing.

use super::cover_depth::{
    BOKEH_FOCAL_SIXTEENTHS, BOKEH_FRAME_HEIGHT, FAR_HIGHLIGHT_RADIUS, FOCAL_SIXTEENTHS,
    FOCUSED_HIGHLIGHT_RADIUS, FOCUS_DISTANCE, HEXAGON_BLADES, NEAR_HIGHLIGHT_RADIUS,
    PARALLAX_FAR_RADIUS, PARALLAX_MIDDLE_HALF_HEIGHT, PARALLAX_MIDDLE_HALF_WIDTH,
    PARALLAX_NEAR_RADIUS, ROLL_HALF_HEIGHT, ROLL_HALF_WIDTH, ROLL_TURN_DENOMINATOR,
};
use super::cover_recording::{COARSE_GRAIN_DENOMINATOR, FINE_GRAIN_DENOMINATOR, GRAIN_DENSITY_ONE};
use synthvid_testkit::{ensure_eq, Fail};

fn check_depth_constants_match_their_documented_values() -> Result<(), Fail> {
    // One frame height is sixteen sixteenths.
    ensure_eq!(FOCAL_SIXTEENTHS.get(), 16);
    // A radius of 160 at depth 1600 projects to 24 * 160 / 1600 = 2.4 sensels.
    ensure_eq!(PARALLAX_FAR_RADIUS.get(), 160);
    // The middle rectangle is 240 by 160 units, so its half sizes are 120 and 80.
    ensure_eq!(PARALLAX_MIDDLE_HALF_WIDTH.get(), 120);
    ensure_eq!(PARALLAX_MIDDLE_HALF_HEIGHT.get(), 80);
    // A radius of 60 at depth 400 projects to 24 * 60 / 400 = 3.6 sensels.
    ensure_eq!(PARALLAX_NEAR_RADIUS.get(), 60);
    // The roll rectangle is 120 by 60 units, so its half sizes are 60 and 30.
    ensure_eq!(ROLL_HALF_WIDTH.get(), 60);
    ensure_eq!(ROLL_HALF_HEIGHT.get(), 30);
    // One thirty-second of a turn per frame.
    ensure_eq!(ROLL_TURN_DENOMINATOR.get(), 32);
    // The bokeh shots are focused at depth 864 through a lens sixteen units high.
    ensure_eq!(FOCUS_DISTANCE.get(), 864);
    ensure_eq!(BOKEH_FRAME_HEIGHT.get(), 16);
    // The bokeh shots use a focal length of six frame heights, 96 sixteenths.
    ensure_eq!(BOKEH_FOCAL_SIXTEENTHS.get(), 96);
    // A hexagonal iris has six blades.
    ensure_eq!(HEXAGON_BLADES.get(), 6);
    // Each highlight projects to 2 sensels: 144 * 6 / 432 = 2 at depth 432,
    // 144 * 12 / 864 = 2 at depth 864, and 144 * 48 / 3456 = 2 at depth 3456.
    ensure_eq!(NEAR_HIGHLIGHT_RADIUS.get(), 6);
    ensure_eq!(FOCUSED_HIGHLIGHT_RADIUS.get(), 12);
    ensure_eq!(FAR_HIGHLIGHT_RADIUS.get(), 48);
    Ok(())
}

#[test]
fn depth_constants_match_their_documented_values() {
    assert_eq!(
        check_depth_constants_match_their_documented_values(),
        Ok(())
    );
}

fn check_recording_constants_match_their_documented_values() -> Result<(), Fail> {
    // The fine grain has a radius of one four-hundredth of the frame height.
    ensure_eq!(FINE_GRAIN_DENOMINATOR.get(), 400);
    // The coarse grain has a radius of one sixteenth of the frame height.
    ensure_eq!(COARSE_GRAIN_DENOMINATOR.get(), 16);
    // A density of 1.0 is 256 in Q8.8.
    ensure_eq!(GRAIN_DENSITY_ONE.get(), 256);
    Ok(())
}

#[test]
fn recording_constants_match_their_documented_values() {
    assert_eq!(
        check_recording_constants_match_their_documented_values(),
        Ok(())
    );
}
