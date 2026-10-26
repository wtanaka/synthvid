//! The noise a digital sensor adds to the light it records.
//!
//! A sensor records each channel value of a rendered frame as the linear light at one sensel.
//! Its [`SensorNoise`] adds four terms to that light, in Q8.8 fixed point, where a raw value of
//! 256 is one code value:
//!
//! - a gain for each sensel, from the fixed pattern, which scales the light;
//! - an offset for each column and channel, from the fixed pattern;
//! - shot noise, whose spread grows with the light after the sensel's gain, from the temporal
//!   stream;
//! - read noise, independent of the light, from the temporal stream.
//!
//! The fixed pattern is the same on every frame. The temporal terms are drawn afresh for each
//! sensel and channel of each frame. Every draw comes from a stream seed computed in closed form,
//! so a frame never depends on the frames rendered before it.
//!
//! Every intermediate value is checked. A value that does not fit its integer type is
//! [`SensorError::Overflow`], never a wrapped or substituted value. Clamping to the code range at
//! the end is the sensor's saturation, not an error substitute.

use core::error::Error;
use core::fmt;

use synthvid_scene::frame::{Frame, PixelColumn, PixelCoord, PixelOutOfBounds, PixelRow};
use synthvid_scene::{
    isqrt, normal_draw, stream_seed, Channel, Dimensions, FrameIndex, IntSqrtInput, NormalDraw,
    Overflow, Rgb8, Rng, StreamIndex,
};

use crate::medium::{ColumnSigma, PatternSeed, PrnuSpread, ReadSigma, SensorNoise, ShotGain, Q8_8};

/// The raw Q8.8 value of one code value, used by the signed noise terms.
pub(crate) const CODE_VALUE: i64 = 256;

/// The raw gain of a sensel with no variation, in parts per 65536.
const UNITY_GAIN: i64 = 0x1_0000;

/// Half a code value, the rounding offset that makes ties round toward positive infinity.
const HALF_CODE_VALUE: i64 = 128;

/// The first stream index of the column offsets.
///
/// The sensel streams number `sensel * 3 + channel`, where a sensel index is below
/// `65535 * 65535 + 65535`, so every sensel stream is below 2^34. The column streams start at
/// 2^40 and number `column * 3 + channel` for a column below 2^16, so they stay below 2^41.
const COLUMN_STREAM_BASE: u64 = 0x100_0000_0000;

/// The first stream index of the temporal draws.
///
/// The fixed pattern uses only the sensel streams and the column streams, which all lie below
/// 2^41. The temporal draws of a sensel are its sensel stream offset by this base, so the two
/// kinds of draw never share a stream index. Their independence therefore does not depend on the
/// pattern seed and the temporal seed being different values.
const TEMPORAL_STREAM_BASE: u64 = 0x400_0000_0000;

/// Returns the stream of the temporal draws of a sensel, given the sensel's own stream.
///
/// The temporal stream is the sensel stream offset by [`TEMPORAL_STREAM_BASE`], so the temporal
/// draws of a sensel never share a stream with its fixed-pattern draws.
///
/// # Errors
///
/// Returns `Err(SensorError::Overflow)` if the offset leaves the `u64` range.
pub(crate) fn temporal_stream_of(sensel_stream: StreamIndex) -> Result<StreamIndex, SensorError> {
    TEMPORAL_STREAM_BASE
        .checked_add(sensel_stream.0)
        .map(StreamIndex)
        .ok_or(SensorError::Overflow)
}

/// Why a sensor's noise could not be applied to a frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SensorError {
    /// An intermediate value of the sensor arithmetic does not fit its integer type.
    Overflow,
    /// A sensel lies outside the frame.
    PixelOutOfBounds,
}

impl fmt::Display for SensorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => write!(formatter, "sensor noise arithmetic overflowed"),
            Self::PixelOutOfBounds => write!(formatter, "sensel lies outside the frame"),
        }
    }
}

impl Error for SensorError {}

/// The gain of one sensel, in parts per 65536, where 65536 is unity.
///
/// The gain is signed because a gain draw can fall below zero. Light scaled by a negative gain
/// is negative, and the final clamp records it as no light. The type is private: a gain is only
/// ever computed inside this module and never leaves it.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct Gain(pub i64);

/// The light at one sensel before noise, as a signed Q8.8 value.
///
/// A raw value of 256 is one code value. The value is signed, so a negative gain stays
/// representable until the final clamp.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Level(pub i64);

/// The fixed offset of one column in one channel, as a signed Q8.8 value.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ColumnOffset(pub i64);

/// The shot-noise term of one sensel in one channel, as a signed Q8.8 value.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ShotNoise(pub i64);

/// The read-noise term of one sensel in one channel, as a signed Q8.8 value.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ReadNoise(pub i64);

/// A signed Q8.8 value that is not yet tied to one term of the sensor noise.
///
/// A raw value of 256 is one code value. Every `i64` is a legal value, so the field is public.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SignedQ8(pub i64);

/// The index of a sensel in the frame, counted row by row from the top left.
///
/// The field is private, so an index can only be taken with [`SenselIndex::of_sensel`], from a
/// column and a row that lie inside the frame. Every index of a frame is therefore below the
/// frame's sensel count.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SenselIndex(u64);

impl SenselIndex {
    /// Takes the index of the sensel at the given column and row of a frame with the given
    /// dimensions: `row * width + column`.
    ///
    /// # Errors
    ///
    /// Returns `Err(SensorError::PixelOutOfBounds)` if the column or the row lies outside the
    /// frame, and `Err(SensorError::Overflow)` if the index does not fit a `u64`.
    pub fn of_sensel(
        column: PixelColumn,
        row: PixelRow,
        dimensions: Dimensions,
    ) -> Result<Self, SensorError> {
        let PixelColumn(column_index) = column;
        let PixelRow(row_index) = row;
        let width = dimensions.width.get().get();
        let height = dimensions.height.get().get();
        if column_index >= width || row_index >= height {
            return Err(SensorError::PixelOutOfBounds);
        }
        u64::from(row_index)
            .checked_mul(u64::from(width))
            .and_then(|offset| offset.checked_add(u64::from(column_index)))
            .map(Self)
            .ok_or(SensorError::Overflow)
    }
}

/// A non-negative standard deviation in raw Q8.8, with no upper bound short of `u64`.
///
/// The shot noise of a bright sensel can be larger than a [`Q8_8`] value once its light is above
/// one code value of full scale, so the deviation is carried in this wider type until it is
/// multiplied by a draw.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Amplitude(pub u64);

/// The light at one sensel after its gain, in whole code values, with no upper clamp.
///
/// Shot noise follows the light a photosite receives, so the light is not clamped to full scale
/// before the noise is drawn. A value above 255 is the light of a sensel brighter than full scale.
/// The saturation happens once, after the noise is added. The value is never negative: light below
/// zero is no light and is recorded as zero.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Light(pub u64);

/// The standard deviation of the shot noise of a sensel with the given light, in raw Q8.8.
///
/// The shot-noise variance of a sensel whose light is `light` code values is `slope * light` in
/// squared code values, so the standard deviation is the integer square root of
/// `slope_raw * light * 256`, floored. The light is taken as it is, above 255 code values or not,
/// so a sensel brighter than full scale has the deviation of its own light. A slope of 256 (1.0)
/// at light 100 gives a deviation of 10 code values, which is 2560 raw.
///
/// # Errors
///
/// Returns `Err(SensorError::Overflow)` if the variance does not fit a `u64`. Since the slope is at
/// most 65535 and the product carries a factor of 256, this needs a light above 2^40 code values.
pub fn shot_sigma(slope: ShotGain, light: Light) -> Result<Amplitude, SensorError> {
    let ShotGain(Q8_8(slope_raw)) = slope;
    let Light(light_codes) = light;
    let variance = u64::from(slope_raw)
        .checked_mul(light_codes)
        .and_then(|product| product.checked_mul(256))
        .ok_or(SensorError::Overflow)?;
    Ok(Amplitude(isqrt(IntSqrtInput(variance)).0))
}

/// A whole code value after rounding, before any clamp to the code range.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct RoundedCode(i64);

/// Rounds a signed Q8.8 total to the nearest whole code value, with ties toward positive infinity.
///
/// The result is `floor((total + 128) / 256)`, unclamped.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the rounding does not fit an `i64`.
fn round_to_code(total: SignedQ8) -> Result<RoundedCode, Overflow> {
    let SignedQ8(total_raw) = total;
    total_raw
        .checked_add(HALF_CODE_VALUE)
        .and_then(|sum| sum.checked_div_euclid(CODE_VALUE))
        .map(RoundedCode)
        .ok_or(Overflow)
}

/// Rounds a signed Q8.8 total to a code value, then clamps it to the code range.
///
/// The total is rounded to the nearest code value with ties toward positive infinity, by flooring
/// `(total + 128) / 256`. The result is clamped to `0..=255`. This clamp is the physical
/// saturation of a sensor or film: a sensel cannot record less than no light or more than full
/// scale.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the rounding does not fit an `i64`.
pub(crate) fn round_to_channel(total: SignedQ8) -> Result<Channel, Overflow> {
    let code = round_to_code(total)?.0.clamp(0, 255);
    let byte = u8::try_from(code).map_err(Overflow::from)?;
    Ok(Channel(byte))
}

/// Rounds the sum of the light and the three noise terms to a code value, then clamps it.
///
/// The sum is rounded and clamped by `round_to_channel`.
///
/// # Errors
///
/// Returns `Err(SensorError::Overflow)` if the sum does not fit an `i64`.
pub fn combine_level(
    level: Level,
    column: ColumnOffset,
    shot: ShotNoise,
    read: ReadNoise,
) -> Result<Channel, SensorError> {
    let Level(level_raw) = level;
    let ColumnOffset(column_raw) = column;
    let ShotNoise(shot_raw) = shot;
    let ReadNoise(read_raw) = read;
    let total = level_raw
        .checked_add(column_raw)
        .and_then(|sum| sum.checked_add(shot_raw))
        .and_then(|sum| sum.checked_add(read_raw))
        .ok_or(SensorError::Overflow)?;
    round_to_channel(SignedQ8(total)).map_err(|Overflow| SensorError::Overflow)
}

/// The light at one sensel before noise: the channel value scaled by its gain, floored.
///
/// The level is `value * 256 * gain / 65536`, in Q8.8.
fn level_of(value: Channel, gain: Gain) -> Result<Level, SensorError> {
    let Channel(value_raw) = value;
    let Gain(gain_raw) = gain;
    let scaled = i64::from(value_raw)
        .checked_mul(CODE_VALUE)
        .and_then(|product| product.checked_mul(gain_raw))
        .ok_or(SensorError::Overflow)?;
    let level = scaled
        .checked_div_euclid(UNITY_GAIN)
        .ok_or(SensorError::Overflow)?;
    Ok(Level(level))
}

/// The light at one sensel after its gain: the level rounded to whole code values, never below
/// zero and not clamped above.
///
/// # Errors
///
/// Returns `Err(SensorError::Overflow)` if the rounding does not fit a `u64`.
fn light_of(level: Level) -> Result<Light, SensorError> {
    let Level(level_raw) = level;
    let RoundedCode(rounded) =
        round_to_code(SignedQ8(level_raw)).map_err(|Overflow| SensorError::Overflow)?;
    let codes = u64::try_from(rounded.max(0))
        .map_err(Overflow::from)
        .map_err(|Overflow| SensorError::Overflow)?;
    Ok(Light(codes))
}

/// An amplitude times a normal draw, in Q8.8, floored: `amplitude * draw / 256`.
///
/// # Errors
///
/// Returns `Err(SensorError::Overflow)` if the amplitude does not fit an `i64` or the product
/// does not fit an `i64`.
fn scaled_draw(amplitude: Amplitude, draw: NormalDraw) -> Result<SignedQ8, SensorError> {
    let Amplitude(raw) = amplitude;
    let NormalDraw(draw_raw) = draw;
    i64::try_from(raw)
        .map_err(Overflow::from)
        .map_err(|Overflow| SensorError::Overflow)?
        .checked_mul(i64::from(draw_raw))
        .and_then(|product| product.checked_div_euclid(CODE_VALUE))
        .map(SignedQ8)
        .ok_or(SensorError::Overflow)
}

/// One normal draw from the fixed pattern, on the given stream.
///
/// The pattern uses frame index 0 on every frame, so the draw is the same on every frame.
fn fixed_draw(pattern: PatternSeed, stream: StreamIndex) -> Result<NormalDraw, SensorError> {
    let seed = stream_seed(pattern.get(), FrameIndex::new(0), stream);
    normal_draw(&mut Rng::from_seed(seed)).map_err(|Overflow| SensorError::Overflow)
}

/// The gain of one sensel: `65536 + prnu * g / 256`, floored, where `g` is a normal draw.
///
/// A zero spread draws nothing and is exactly unity.
fn gain_of(
    prnu: PrnuSpread,
    pattern: PatternSeed,
    stream: StreamIndex,
) -> Result<Gain, SensorError> {
    let PrnuSpread(spread) = prnu;
    if spread == 0 {
        return Ok(Gain(UNITY_GAIN));
    }
    let NormalDraw(draw) = fixed_draw(pattern, stream)?;
    let deviation = i64::from(spread)
        .checked_mul(i64::from(draw))
        .and_then(|product| product.checked_div_euclid(CODE_VALUE))
        .ok_or(SensorError::Overflow)?;
    let gain = UNITY_GAIN
        .checked_add(deviation)
        .ok_or(SensorError::Overflow)?;
    Ok(Gain(gain))
}

/// The fixed offset of one column in one channel: `sigma * h / 256`, floored.
///
/// A zero spread draws nothing and is exactly zero.
fn column_offset(
    sigma: ColumnSigma,
    pattern: PatternSeed,
    stream: StreamIndex,
) -> Result<ColumnOffset, SensorError> {
    let ColumnSigma(Q8_8(amplitude)) = sigma;
    if amplitude == 0 {
        return Ok(ColumnOffset(0));
    }
    let draw = fixed_draw(pattern, stream)?;
    Ok(ColumnOffset(
        scaled_draw(Amplitude(u64::from(amplitude)), draw)?.0,
    ))
}

/// The shot-noise and read-noise terms of one sensel in one channel, from the temporal stream.
///
/// The shot noise grows with `light`, the channel's light after its gain, rounded to whole code
/// values and not clamped above, so a sensel brighter than full scale is noisier than one at full
/// scale. Both terms draw from one generator, so the shot draw is always taken first. When both
/// terms are absent, nothing is drawn and both are exactly zero.
fn temporal_terms(
    light: Light,
    context: FrameContext<'_>,
    stream: StreamIndex,
) -> Result<(ShotNoise, ReadNoise), SensorError> {
    let ShotGain(Q8_8(shot_slope)) = context.noise.shot();
    let ReadSigma(Q8_8(read_amplitude)) = context.noise.read();
    if shot_slope == 0 && read_amplitude == 0 {
        return Ok((ShotNoise(0), ReadNoise(0)));
    }
    let sigma = shot_sigma(context.noise.shot(), light)?;
    let seed = stream_seed(context.noise.temporal_seed().get(), context.index, stream);
    let mut rng = Rng::from_seed(seed);
    let shot_draw = normal_draw(&mut rng).map_err(|Overflow| SensorError::Overflow)?;
    let read_draw = normal_draw(&mut rng).map_err(|Overflow| SensorError::Overflow)?;
    let shot = scaled_draw(sigma, shot_draw)?;
    let read = scaled_draw(Amplitude(u64::from(read_amplitude)), read_draw)?;
    Ok((ShotNoise(shot.0), ReadNoise(read.0)))
}

/// One of the three channels of a sensel, numbered in the order the streams use.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum ColorPlane {
    /// The red channel.
    Red,
    /// The green channel.
    Green,
    /// The blue channel.
    Blue,
}

/// The slot of a channel among the three channels of an element: 0 for red, 1 for green, and 2
/// for blue.
///
/// A slot is a position, not a stream, so it has its own type and cannot be passed where a
/// [`StreamIndex`] is expected. It becomes a stream only inside [`channel_stream`] and
/// [`channel_block_base`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct ChannelSlot(pub u64);

impl ColorPlane {
    /// The channel's slot among the three channels of an element.
    const fn slot(self) -> ChannelSlot {
        match self {
            Self::Red => ChannelSlot(0),
            Self::Green => ChannelSlot(1),
            Self::Blue => ChannelSlot(2),
        }
    }
}

/// The stream of one channel of an element whose three channels have consecutive streams: the
/// element's index times three, plus the channel's slot.
fn channel_stream(element: StreamIndex, plane: ColorPlane) -> Result<StreamIndex, SensorError> {
    let StreamIndex(index) = element;
    let ChannelSlot(slot) = plane.slot();
    index
        .checked_mul(3)
        .and_then(|base| base.checked_add(slot))
        .map(StreamIndex)
        .ok_or(SensorError::Overflow)
}

/// The number of consecutive streams in one block, such as the padded rows of one channel of a
/// grain field.
///
/// A length counts streams and is not a stream index, so it has its own type and cannot be passed
/// where a [`StreamIndex`] is expected.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct BlockLength(pub u64);

/// The first stream of one channel's block of consecutive streams: the channel's slot times the
/// length of the block.
///
/// # Errors
///
/// Returns `Err(Overflow)` if the product does not fit a `u64`.
pub(crate) fn channel_block_base(
    plane: ColorPlane,
    block: BlockLength,
) -> Result<StreamIndex, Overflow> {
    let ChannelSlot(slot) = plane.slot();
    let BlockLength(length) = block;
    slot.checked_mul(length).map(StreamIndex).ok_or(Overflow)
}

/// The sensel stream of one channel of a sensel, from the sensel's index and the color plane.
///
/// # Errors
///
/// Returns `Err(SensorError::Overflow)` if the stream does not fit a `u64`.
fn sensel_stream_of(sensel: SenselIndex, plane: ColorPlane) -> Result<StreamIndex, SensorError> {
    let SenselIndex(index) = sensel;
    channel_stream(StreamIndex(index), plane)
}

/// The column stream of one channel of a column: the column streams start at
/// [`COLUMN_STREAM_BASE`], and each column has three consecutive streams, one per channel.
///
/// # Errors
///
/// Returns `Err(SensorError::Overflow)` if the stream does not fit a `u64`.
fn column_stream_of(column: PixelColumn, plane: ColorPlane) -> Result<StreamIndex, SensorError> {
    let PixelColumn(column_index) = column;
    let StreamIndex(position) = channel_stream(StreamIndex(u64::from(column_index)), plane)?;
    COLUMN_STREAM_BASE
        .checked_add(position)
        .map(StreamIndex)
        .ok_or(SensorError::Overflow)
}

/// Where one sensel lies: its column, and its index counted row by row across the frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct SenselSite {
    /// The sensel's column.
    column: PixelColumn,
    /// The sensel's index: row times width plus column.
    sensel: SenselIndex,
}

/// What every sensel of one frame shares: the noise and the frame index.
#[derive(Copy, Clone, Debug)]
struct FrameContext<'noise> {
    /// The noise the sensor adds.
    noise: &'noise SensorNoise,
    /// The index of the frame being recorded.
    index: FrameIndex,
}

/// The recorded value of one channel of one sensel.
fn channel_output(
    value: Channel,
    plane: ColorPlane,
    site: SenselSite,
    context: FrameContext<'_>,
) -> Result<Channel, SensorError> {
    let sensel_stream = sensel_stream_of(site.sensel, plane)?;
    let column_stream = column_stream_of(site.column, plane)?;
    let temporal_stream = temporal_stream_of(sensel_stream)?;
    let pattern = context.noise.pattern_seed();
    let gain = gain_of(context.noise.prnu(), pattern, sensel_stream)?;
    let level = level_of(value, gain)?;
    // The shot noise is driven by the light after the gain, rounded but not clamped above.
    let light = light_of(level)?;
    let column = column_offset(context.noise.column(), pattern, column_stream)?;
    let (shot, read) = temporal_terms(light, context, temporal_stream)?;
    combine_level(level, column, shot, read)
}

/// Applies a digital sensor's noise to a frame, in place, as the frame at `index`.
///
/// Each channel value of each sensel is taken as the linear light at that sensel. The fixed
/// pattern (gain and column offsets) is the same on every frame; the shot and read noise are
/// drawn for the given frame index. The shot noise of a sensel grows with its light after the gain,
/// not with the clean value. A noise with every term absent leaves the frame unchanged.
///
/// # Errors
///
/// Returns `Err(SensorError::Overflow)` if any intermediate value does not fit its integer
/// type, and `Err(SensorError::PixelOutOfBounds)` if a sensel cannot be addressed in the frame.
/// On error the frame may hold a partly updated sensel row and must not be used.
pub fn apply_sensor_noise(
    frame: &mut Frame,
    noise: &SensorNoise,
    index: FrameIndex,
) -> Result<(), SensorError> {
    let dimensions = frame.dimensions();
    let context = FrameContext { noise, index };
    let width = dimensions.width.get().get();
    let height = dimensions.height.get().get();
    for row in 0..height {
        for column in 0..width {
            let coord = PixelCoord::new(PixelColumn(column), PixelRow(row));
            let color = frame
                .get_pixel(coord)
                .ok_or(SensorError::PixelOutOfBounds)?;
            let sensel = SenselIndex::of_sensel(PixelColumn(column), PixelRow(row), dimensions)?;
            let site = SenselSite {
                column: PixelColumn(column),
                sensel,
            };
            let rendered = Rgb8 {
                red: channel_output(color.red, ColorPlane::Red, site, context)?,
                green: channel_output(color.green, ColorPlane::Green, site, context)?,
                blue: channel_output(color.blue, ColorPlane::Blue, site, context)?,
            };
            frame
                .set_pixel(coord, rendered)
                .map_err(|PixelOutOfBounds| SensorError::PixelOutOfBounds)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
