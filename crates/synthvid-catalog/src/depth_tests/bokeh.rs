//! Tests of the bokeh shots: the highlights seen through a disc or a hexagonal iris.

use super::*;

/// The number of sensels in a band of columns whose color differs from the backdrop.
fn off_backdrop_sensels(
    frame: &Frame,
    band: ColumnBand,
    backdrop: Rgb8,
) -> Result<SenselCount, Fail> {
    let height = frame.dimensions().height.get().get();
    let mut count = SenselCount(0);
    for column in band.first.0..band.end.0 {
        for row in 0..height {
            let coord = PixelCoord::new(PixelColumn(column), PixelRow(row));
            let color = frame.get_pixel(coord).or_fail()?;
            if color != backdrop {
                let next = count.0.checked_add(1).or_fail()?;
                count = SenselCount(next);
            }
        }
    }
    Ok(count)
}

/// The largest number of code values by which a sensel's red channel exceeds the backdrop's, over
/// a band of columns. The highlights are white, so the red channel carries the brightness.
fn peak_excess(frame: &Frame, band: ColumnBand, backdrop: Rgb8) -> Result<CodeExcess, Fail> {
    let height = frame.dimensions().height.get().get();
    let Channel(base) = backdrop.red;
    let mut peak = CodeExcess(0);
    for column in band.first.0..band.end.0 {
        for row in 0..height {
            let coord = PixelCoord::new(PixelColumn(column), PixelRow(row));
            let Channel(value) = frame.get_pixel(coord).or_fail()?.red;
            if value > base {
                let excess = u32::from(value).checked_sub(u32::from(base)).or_fail()?;
                if excess > peak.0 {
                    peak = CodeExcess(excess);
                }
            }
        }
    }
    Ok(peak)
}

/// Two frames to compare.
struct FramePair<'frames> {
    /// The first frame.
    first: &'frames Frame,
    /// The second frame.
    second: &'frames Frame,
}

/// The number of sensels at which two frames of the same dimensions differ.
fn differing_sensels(pair: &FramePair<'_>) -> Result<SenselCount, Fail> {
    let (first, second) = (pair.first, pair.second);
    ensure!(
        first.dimensions() == second.dimensions(),
        "the frames being compared must have the same dimensions"
    );
    let dimensions = first.dimensions();
    let width = dimensions.width.get().get();
    let height = dimensions.height.get().get();
    let mut count = SenselCount(0);
    for column in 0..width {
        for row in 0..height {
            let coord = PixelCoord::new(PixelColumn(column), PixelRow(row));
            if first.get_pixel(coord).or_fail()? != second.get_pixel(coord).or_fail()? {
                let next = count.0.checked_add(1).or_fail()?;
                count = SenselCount(next);
            }
        }
    }
    Ok(count)
}

/// The frame at index 0 of a bokeh entry, with the backdrop of its stage.
struct BokehFrame {
    /// The rendered frame.
    frame: Frame,
    /// The backdrop color of the stage.
    backdrop: Rgb8,
}

/// Renders the frame at index 0 of the bokeh entry of the given depth.
fn bokeh_frame(depth: DeepAxis) -> Result<BokehFrame, Fail> {
    let spec = deep_entry(depth)?.to_video_spec().or_fail()?;
    let deep = deep_shot(&spec)?;
    let mut frame = Frame::zeroed(spec.recorder().dimensions()).or_fail()?;
    render_stage_into(deep, FrameIndex::new(0), &mut frame).or_fail()?;
    Ok(BokehFrame {
        frame,
        backdrop: deep.stage().backdrop,
    })
}

/// The disc bokeh frame: the near and far highlights smear brightly inside the frame, and the
/// focused highlight is smaller than the far one.
///
/// The near smear covers columns 1 to 12, and the far smear covers columns 20 to 29. The focused
/// highlight covers columns 14 to 17. The test requires each smear to peak at least 30 above the
/// backdrop, and nothing above the backdrop to touch the left or right border column.
fn check_disc_bokeh_smears_are_bright_and_inside_the_frame() -> Result<(), Fail> {
    let bokeh = bokeh_frame(DeepAxis::DiscBokeh)?;
    let frame = &bokeh.frame;
    let backdrop = bokeh.backdrop;
    let width = frame.dimensions().width.get().get();
    let last = width.checked_sub(1).or_fail()?;
    let near = ColumnBand {
        first: PixelColumn(0),
        end: PixelColumn(13),
    };
    let focused = ColumnBand {
        first: PixelColumn(13),
        end: PixelColumn(19),
    };
    let far = ColumnBand {
        first: PixelColumn(19),
        end: PixelColumn(width),
    };
    let near_peak = peak_excess(frame, near, backdrop)?;
    ensure!(
        near_peak >= CodeExcess(30),
        "the near smear peaks {near_peak:?} code values above the backdrop"
    );
    let far_peak = peak_excess(frame, far, backdrop)?;
    ensure!(
        far_peak >= CodeExcess(30),
        "the far smear peaks {far_peak:?} code values above the backdrop"
    );
    let left_border = off_backdrop_sensels(
        frame,
        ColumnBand {
            first: PixelColumn(0),
            end: PixelColumn(1),
        },
        backdrop,
    )?;
    ensure_eq!(left_border, SenselCount(0), "the left border column");
    let right_border = off_backdrop_sensels(
        frame,
        ColumnBand {
            first: PixelColumn(last),
            end: PixelColumn(width),
        },
        backdrop,
    )?;
    ensure_eq!(right_border, SenselCount(0), "the right border column");
    let focused_count = off_backdrop_sensels(frame, focused, backdrop)?;
    let far_count = off_backdrop_sensels(frame, far, backdrop)?;
    ensure!(
        focused_count < far_count,
        "the focused highlight covers {focused_count:?} sensels and the one at depth 3456 covers {far_count:?}"
    );
    Ok(())
}

#[test]
fn disc_bokeh_smears_are_bright_and_inside_the_frame() {
    assert_eq!(
        check_disc_bokeh_smears_are_bright_and_inside_the_frame(),
        Ok(())
    );
}

/// The red channel of the sensel at the given column and row of a frame.
fn red_at(frame: &Frame, column: PixelColumn, row: PixelRow) -> Result<Channel, Fail> {
    let color = frame.get_pixel(PixelCoord::new(column, row)).or_fail()?;
    Ok(color.red)
}

/// The columns of a frame that hold at least one sensel off the backdrop, in increasing order.
fn columns_off_backdrop(frame: &Frame, backdrop: Rgb8) -> Result<Vec<PixelColumn>, Fail> {
    let width = frame.dimensions().width.get().get();
    let mut columns = Vec::new();
    for column in 0..width {
        let next = column.checked_add(1).or_fail()?;
        let band = ColumnBand {
            first: PixelColumn(column),
            end: PixelColumn(next),
        };
        if off_backdrop_sensels(frame, band, backdrop)? > SenselCount(0) {
            columns.push(PixelColumn(column));
        }
    }
    Ok(columns)
}

/// The disc bokeh frame at index 0 holds the rounded values of the bokeh documentation: the near
/// smear center is 71, the far smear center is 121, and the focused highlight center is 255. The
/// columns above the backdrop are exactly the three smears, and every other column is the backdrop.
#[test]
fn disc_bokeh_frame_holds_the_documented_values() {
    assert_eq!(check_disc_bokeh_frame_holds_the_documented_values(), Ok(()));
}

fn check_disc_bokeh_frame_holds_the_documented_values() -> Result<(), Fail> {
    let bokeh = bokeh_frame(DeepAxis::DiscBokeh)?;
    let frame = &bokeh.frame;
    for (column, row) in [(6, 11), (7, 11), (6, 12), (7, 12)] {
        ensure_eq!(
            red_at(frame, PixelColumn(column), PixelRow(row))?,
            Channel(71),
            "the near smear center at column {column}, row {row}"
        );
    }
    for (column, row) in [(24, 11), (25, 11), (24, 12), (25, 12)] {
        ensure_eq!(
            red_at(frame, PixelColumn(column), PixelRow(row))?,
            Channel(121),
            "the far smear center at column {column}, row {row}"
        );
    }
    for (column, row) in [(15, 11), (16, 11), (15, 12), (16, 12)] {
        ensure_eq!(
            red_at(frame, PixelColumn(column), PixelRow(row))?,
            Channel(255),
            "the focused highlight center at column {column}, row {row}"
        );
    }
    let smears: Vec<PixelColumn> = (1..=12)
        .chain(14..=17)
        .chain(20..=29)
        .map(PixelColumn)
        .collect();
    ensure_eq!(columns_off_backdrop(frame, bokeh.backdrop)?, smears);
    Ok(())
}

/// The disc and hexagonal bokeh frames differ: the blades of the iris change the shape of every
/// smear, so the two frames hold different colors at some sensels.
fn check_disc_and_hex_bokeh_frames_differ() -> Result<(), Fail> {
    let disc = bokeh_frame(DeepAxis::DiscBokeh)?;
    let hex = bokeh_frame(DeepAxis::HexBokeh)?;
    let differing = differing_sensels(&FramePair {
        first: &disc.frame,
        second: &hex.frame,
    })?;
    ensure!(
        differing > SenselCount(0),
        "the disc and hexagonal frames must differ at some sensel"
    );
    Ok(())
}

#[test]
fn disc_and_hex_bokeh_frames_differ() {
    assert_eq!(check_disc_and_hex_bokeh_frames_differ(), Ok(()));
}
