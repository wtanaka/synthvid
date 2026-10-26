//! Sampling of a deep shot's prepared layers into the sensels of a frame.
//!
//! [`render_stage_into`] prepares the layers of a shot for one frame, then gives each sensel the
//! average of the colors its samples see. A sample lies at one lattice point inside its sensel.
//! Through each point of the lens, the sample sees each layer at the point shifted by that
//! point's shift for the layer. The first layer in front-to-back order whose outline covers the
//! shifted point supplies the color, and the backdrop supplies it when no layer does.
//!
//! Each channel's average is rounded half up, so it stays within the range of the channel values
//! that were averaged. Every frame depends only on its own frame index, so no state is kept
//! between frames.

use core::num::NonZeroU32;

use synthvid_scene::frame::{Frame, PixelColumn, PixelCoord, PixelOutOfBounds, PixelRow};
use synthvid_scene::{
    covers, sample_point, Channel, Containment, FrameIndex, LatticePoint, LatticeShift, Overflow,
    PixelGrid, Rgb8, SampleSlot,
};

use crate::rig::DeepShot;

use super::{prepare_frame, DeepRenderError, PreparedFrame, PreparedLayer};

/// The number of samples that one sensel averages over: every sample of its grid, seen through
/// every point of the lens. Both factors are non-zero, so the count is never zero.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct SampleCount(pub NonZeroU32);

/// The sum of one channel's values over the samples of a sensel.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct ChannelSum(pub u32);

/// The sums of the three channels over the samples of a sensel.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct ChannelSums {
    /// The sum of the red values.
    red: ChannelSum,
    /// The sum of the green values.
    green: ChannelSum,
    /// The sum of the blue values.
    blue: ChannelSum,
}

/// The column and row of one sensel.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct SenselSite {
    /// The column of the sensel.
    column: PixelColumn,
    /// The row of the sensel.
    row: PixelRow,
}

/// What every sensel of one frame shares: the prepared layers with their lens rows, the sample
/// grid, and the number of samples that each sensel averages over.
#[derive(Clone, Debug)]
struct SamplingPlan<'frame> {
    /// The backdrop, the visible layers from front to back, and the lens rows of their shifts.
    prepared: &'frame PreparedFrame<'frame>,
    /// The number of samples along each axis of a sensel.
    grid: PixelGrid,
    /// The number of samples that each sensel averages over.
    count: SampleCount,
}

/// The sums of no samples, which every sensel starts from.
const NO_SUMS: ChannelSums = ChannelSums {
    red: ChannelSum(0),
    green: ChannelSum(0),
    blue: ChannelSum(0),
};

/// Renders a deep shot into a frame, as the frame at one index.
///
/// The frame must have exactly the dimensions of the rig's recorder. Each sensel takes the average
/// of its samples, channel by channel, over every point of the lens.
///
/// # Errors
///
/// Returns `Err(DeepRenderError::FrameSizeMismatch)` if the frame's dimensions differ from the
/// recorder's, and the errors of [`prepare_frame`] if the shot cannot be prepared for the frame.
/// Returns `Err(DeepRenderError::Overflow)` if a sum or count leaves its integer range,
/// `Err(DeepRenderError::Lattice(_))` if a sample cannot be placed in its sensel, and
/// `Err(DeepRenderError::PixelOutOfBounds)` if a sensel cannot be written to the frame.
pub fn render_stage_into(
    shot: &DeepShot,
    index: FrameIndex,
    out: &mut Frame,
) -> Result<(), DeepRenderError> {
    let recorded = shot.rig().recorder().dimensions();
    if out.dimensions() != recorded {
        return Err(DeepRenderError::FrameSizeMismatch);
    }
    let prepared = prepare_frame(shot, index)?;
    let plan = SamplingPlan::new(&prepared, shot.pixels())?;
    let width = recorded.width.get().get();
    let height = recorded.height.get().get();
    for row in 0..height {
        for column in 0..width {
            let site = SenselSite {
                column: PixelColumn(column),
                row: PixelRow(row),
            };
            let color = sensel_color(&plan, site)?;
            out.set_pixel(PixelCoord::new(site.column, site.row), color)
                .map_err(|PixelOutOfBounds| DeepRenderError::PixelOutOfBounds)?;
        }
    }
    Ok(())
}

impl<'frame> SamplingPlan<'frame> {
    /// Builds the plan for a prepared frame and a sample grid, counting the samples per sensel.
    ///
    /// # Errors
    ///
    /// Returns `Err(DeepRenderError::Overflow)` if the sample count does not fit a `u32`.
    fn new(
        prepared: &'frame PreparedFrame<'frame>,
        grid: PixelGrid,
    ) -> Result<Self, DeepRenderError> {
        let per_axis = grid.per_axis();
        let lens = prepared.lens_points();
        // Both factors are non-zero, so the product is never zero.
        let count = per_axis
            .checked_mul(per_axis)
            .and_then(|area| area.checked_mul(lens.non_zero()))
            .ok_or(DeepRenderError::Overflow)?;
        Ok(Self {
            prepared,
            grid,
            count: SampleCount(count),
        })
    }
}

/// Returns the color of one sensel: the average of its samples, channel by channel.
///
/// # Errors
///
/// Returns the errors of [`sensel_sums`] and [`average_channel`].
fn sensel_color(plan: &SamplingPlan<'_>, site: SenselSite) -> Result<Rgb8, DeepRenderError> {
    let sums = sensel_sums(plan, site)?;
    Ok(Rgb8 {
        red: average_channel(sums.red, plan.count)?,
        green: average_channel(sums.green, plan.count)?,
        blue: average_channel(sums.blue, plan.count)?,
    })
}

/// Sums the colors of every sample of one sensel, through every point of the lens.
///
/// # Errors
///
/// Returns the errors of [`add_sample`].
fn sensel_sums(plan: &SamplingPlan<'_>, site: SenselSite) -> Result<ChannelSums, DeepRenderError> {
    let mut sums = NO_SUMS;
    for slot in plan.grid.slots() {
        sums = add_sample(sums, plan, site, slot)?;
    }
    Ok(sums)
}

/// Adds the colors that one sample sees through every point of the lens to the sums.
///
/// # Errors
///
/// Returns the errors of [`lens_color`] and [`add_color`].
fn add_sample(
    before: ChannelSums,
    plan: &SamplingPlan<'_>,
    site: SenselSite,
    slot: SampleSlot,
) -> Result<ChannelSums, DeepRenderError> {
    let point = sample_point(site.column, site.row, slot);
    let mut sums = before;
    for shifts in plan.prepared.lens_rows() {
        let color = lens_color(plan.prepared, point, shifts)?;
        sums = add_color(sums, color)?;
    }
    Ok(sums)
}

/// Returns the color that a sample sees through one point of the lens.
///
/// The row of shifts is the one for that point, with one shift for each layer. The layers are
/// tested from front to back, each paired with its shift. The first layer whose outline covers
/// the shifted point gives its fill, and the backdrop is the color when no layer covers it.
///
/// # Errors
///
/// Returns the errors of [`layer_containment`].
fn lens_color(
    prepared: &PreparedFrame<'_>,
    point: LatticePoint,
    shifts: &[LatticeShift],
) -> Result<Rgb8, DeepRenderError> {
    for (layer, shift) in prepared.layers().iter().zip(shifts) {
        match layer_containment(layer, point, *shift)? {
            Containment::Inside => return Ok(layer.fill()),
            Containment::Outside => {}
        }
    }
    Ok(prepared.backdrop())
}

/// Tests whether a layer covers a sample, seen through one point of the lens.
///
/// The layer's shift for the lens point moves the sample before the test. The shifted point must
/// lie inside the layer's bounds, which are inclusive, before its outline is tested.
///
/// # Errors
///
/// Returns the errors of [`shifted_point`] and the lattice coverage test.
fn layer_containment(
    layer: &PreparedLayer,
    point: LatticePoint,
    shift: LatticeShift,
) -> Result<Containment, DeepRenderError> {
    let shifted = shifted_point(point, shift)?;
    let bounds = layer.bounds();
    if shifted.across < bounds.low.across
        || bounds.high.across < shifted.across
        || shifted.down < bounds.low.down
        || bounds.high.down < shifted.down
    {
        return Ok(Containment::Outside);
    }
    Ok(covers(layer.shape(), shifted)?)
}

/// Returns the point that a sample is seen at, once a layer's shift is taken away.
///
/// # Errors
///
/// Returns `Err(Overflow)` if either coordinate leaves the `i64` range.
fn shifted_point(point: LatticePoint, shift: LatticeShift) -> Result<LatticePoint, Overflow> {
    point.less(shift)
}

/// Adds one sample's color to the sums of the three channels.
///
/// # Errors
///
/// Returns `Err(Overflow)` if a sum leaves its `u32` range.
fn add_color(sums: ChannelSums, color: Rgb8) -> Result<ChannelSums, Overflow> {
    Ok(ChannelSums {
        red: add_channel(sums.red, color.red)?,
        green: add_channel(sums.green, color.green)?,
        blue: add_channel(sums.blue, color.blue)?,
    })
}

/// Adds one channel value to its sum.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the sum leaves its `u32` range.
fn add_channel(sum: ChannelSum, value: Channel) -> Result<ChannelSum, Overflow> {
    let ChannelSum(total) = sum;
    let Channel(raw) = value;
    Ok(ChannelSum(
        total.checked_add(u32::from(raw)).ok_or(Overflow)?,
    ))
}

/// Averages a channel's sum over a count of samples, rounding half up.
///
/// The result is `floor((2 * sum + n) / (2 * n))`. Every sample is at most 255, so the average is
/// too, and the conversion to a channel cannot fail for a sum that the samples produced.
///
/// # Errors
///
/// Returns `Err(DeepRenderError::Overflow)` if an intermediate value leaves its `u32` range or the
/// quotient does not fit a channel.
fn average_channel(sum: ChannelSum, count: SampleCount) -> Result<Channel, DeepRenderError> {
    let ChannelSum(total) = sum;
    let SampleCount(samples) = count;
    let divisor = samples.get().checked_mul(2).ok_or(Overflow)?;
    let numerator = total
        .checked_mul(2)
        .and_then(|doubled| doubled.checked_add(samples.get()))
        .ok_or(Overflow)?;
    let quotient = numerator.checked_div(divisor).ok_or(Overflow)?;
    let raw = u8::try_from(quotient).map_err(Overflow::from)?;
    Ok(Channel(raw))
}

#[cfg(test)]
mod tests;
