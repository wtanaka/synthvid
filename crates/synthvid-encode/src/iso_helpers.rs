//! Helper types and functions for ISO base media container writing.

use crate::avi_helpers::FrameCount;
use crate::byte_views::{ByteSink, FrameList};
use crate::iso::{IsoCodec, IsoError, TrackMatrix};
use crate::iso_types::{BoxSize, BoxType, ByteOffset, I32Be, U16Be, U32Be};
use crate::media::EncodedFrame;
use crate::sample_table::{write_stbl, StcoOffsetInfo};
use core::num::NonZeroU32;
use synthvid_scene::{CaptureCarrier, Dimensions, FrameRate, FrameRates, Height, Width};

mod binary32;
use binary32::{binary32_bits, Numerator};

/// Writes a complete ISO box: a placeholder size, `tag`, then `body`'s own
/// writes, then patches the size field from the bytes `body` actually
/// wrote. There is no separate "close" step for a caller to forget -- the
/// patch always happens, since it is the last thing this function itself
/// does, not something delegated back to the caller.
pub(crate) fn write_box<T>(
    sink: &mut ByteSink<'_>,
    tag: BoxType,
    body: impl FnOnce(&mut ByteSink<'_>) -> Result<T, IsoError>,
) -> Result<T, IsoError> {
    let start_pos = sink.0.len();
    sink.0.extend_from_slice(&[0, 0, 0, 0]); // size placeholder
    sink.0.extend_from_slice(&tag.0);
    let result = body(sink)?;

    // Total: start_pos was buffer.len() earlier in this call, and a Vec
    // only grows, so this never underflows.
    let len = match sink.0.len().checked_sub(start_pos) {
        Some(len) => len,
        None if start_pos == 0 => 0,
        None => 0,
    };

    // Boxes larger than 4 GB cannot be represented in the ISO base media format (uses u32 size field).
    // Fail early if the box exceeds this limit, ensuring the error is properly propagated.
    let size = u32::try_from(len).map_err(IsoError::size_overflow)?;

    // Patch the size field at the start position.
    patch_u32_be(sink, ByteOffset(start_pos), U32Be(size));

    Ok(result)
}

/// Total duration of the movie in movie-timescale units
/// (`frame_count * playback_rate.denom()`), computed once and written
/// identically by `mvhd` and `tkhd`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct MovieDuration(u32);

impl MovieDuration {
    /// Computes the duration of `frame_count` frames at the playback rate.
    fn new(playback: FrameRate, frame_count: FrameCount) -> Result<Self, IsoError> {
        let denom =
            u32::try_from(playback.ratio().denom().get()).map_err(IsoError::size_overflow)?;
        frame_count
            .get()
            .checked_mul(denom)
            .map(Self)
            .ok_or(IsoError::SizeOverflow)
    }
}

/// The picture parameters of an ISO track.
#[derive(Copy, Clone, Debug)]
pub struct TrackParams {
    /// Frame width in pixels.
    pub width: Width,
    /// Frame height in pixels.
    pub height: Height,
    /// Codec type (uncompressed RGB or motion-JPEG).
    pub codec: IsoCodec,
    /// Display transformation matrix.
    pub track_matrix: TrackMatrix,
}

/// Writes a 32-bit big-endian value to the buffer.
pub(crate) fn write_u32_be(sink: &mut ByteSink<'_>, value: U32Be) {
    sink.0.extend_from_slice(&value.0.to_be_bytes());
}

/// Writes a 16-bit big-endian value to the buffer.
pub(crate) fn write_u16_be(sink: &mut ByteSink<'_>, value: U16Be) {
    sink.0.extend_from_slice(&value.0.to_be_bytes());
}

/// Writes a signed 32-bit big-endian value to the buffer.
pub(crate) fn write_i32_be(sink: &mut ByteSink<'_>, value: I32Be) {
    sink.0.extend_from_slice(&value.0.to_be_bytes());
}

/// Writes a box header (size and type).
pub(crate) fn write_box_header(sink: &mut ByteSink<'_>, box_type: BoxType, size: BoxSize) {
    write_u32_be(sink, U32Be(size.0));
    sink.0.extend_from_slice(&box_type.0);
}

/// Updates a 32-bit big-endian value at a specific position in the buffer.
///
/// The buffer range calculation uses `checked_add` to avoid overflow. A write
/// position recorded from `buffer.len()` is always less than any later
/// position, so the checked arithmetic never actually fails.
pub(crate) fn patch_u32_be(sink: &mut ByteSink<'_>, pos: ByteOffset, value: U32Be) {
    let bytes = value.0.to_be_bytes();
    if let Some(end) = pos.0.checked_add(4) {
        if let Some(slice) = sink.0.get_mut(pos.0..end) {
            slice.copy_from_slice(&bytes);
        }
    }
}

/// Writes the ftyp box (file type).
pub(crate) fn write_ftyp(sink: &mut ByteSink<'_>) {
    // ftyp box: 4-byte size + 4-byte type + 4-byte major brand + 4-byte minor version + compatible brands
    // Size: 20 bytes (box header) + 4 (major_brand) + 4 (minor_version) + 4 (compatible_brand)
    write_box_header(sink, BoxType(*b"ftyp"), BoxSize(20));
    sink.0.extend_from_slice(b"isom"); // Major brand: ISO Base Media
    write_u32_be(sink, U32Be(512)); // Minor version
    sink.0.extend_from_slice(b"isom"); // Compatible brand
}

/// Writes the mdat box (media data).
pub(crate) fn write_mdat(
    sink: &mut ByteSink<'_>,
    frames: FrameList<'_>,
    _codec: IsoCodec,
) -> Result<(), IsoError> {
    // Calculate total frame data size
    let mut frame_data_size: u64 = 0;
    for EncodedFrame(frame) in frames.0 {
        let frame_len = u32::try_from(frame.len()).map_err(IsoError::size_overflow)?;
        frame_data_size = frame_data_size
            .checked_add(u64::from(frame_len))
            .ok_or(IsoError::SizeOverflow)?;
    }

    // Box header is 8 bytes
    let total_size = frame_data_size
        .checked_add(8)
        .ok_or(IsoError::SizeOverflow)?;

    // Write box header
    let size_u32 = u32::try_from(total_size).map_err(IsoError::size_overflow)?;
    write_box_header(sink, BoxType(*b"mdat"), BoxSize(size_u32));

    // Write frame data
    for EncodedFrame(frame) in frames.0 {
        sink.0.extend_from_slice(frame);
    }

    Ok(())
}

/// Writes the moov box (movie metadata).
pub(crate) fn write_moov(
    sink: &mut ByteSink<'_>,
    frame_rates: FrameRates,
    frames: FrameList<'_>,
    track_params: TrackParams,
) -> Result<StcoOffsetInfo, IsoError> {
    write_box(
        sink,
        BoxType(*b"moov"),
        |moov_sink| -> Result<StcoOffsetInfo, IsoError> {
            // Write mvhd (movie header)
            let frame_count =
                FrameCount::new(u32::try_from(frames.0.len()).map_err(IsoError::size_overflow)?);
            let movie_duration = MovieDuration::new(frame_rates.playback(), frame_count)?;
            write_mvhd(moov_sink, frame_rates.playback(), movie_duration)?;

            // Write trak (track) and get stco offset info
            let stco_info =
                write_trak(moov_sink, frame_rates, frames, track_params, movie_duration)?;

            // A distinct capture rate is recorded as metadata only, as the last child of moov.
            // Its carrier is one value, so udta and meta are never both written and their
            // order is never a choice.
            if let Some(capture) = frame_rates.distinct_capture() {
                match capture.carrier {
                    CaptureCarrier::ProjectBox => write_udta(moov_sink, capture.rate)?,
                    CaptureCarrier::QuickTimeMeta => write_meta(moov_sink, capture.rate)?,
                }
            }

            Ok(stco_info)
        },
    )
}

/// Writes the mvhd box (movie header).
fn write_mvhd(
    sink: &mut ByteSink<'_>,
    frame_rate: FrameRate,
    duration: MovieDuration,
) -> Result<(), IsoError> {
    write_box(
        sink,
        BoxType(*b"mvhd"),
        |mvhd_sink| -> Result<(), IsoError> {
            mvhd_sink.0.push(0); // Version 0
            mvhd_sink.0.extend_from_slice(&[0, 0, 0]); // Flags
            write_u32_be(mvhd_sink, U32Be(0)); // Creation time
            write_u32_be(mvhd_sink, U32Be(0)); // Modification time

            // Timescale: use frame rate numerator (exact playback rate numerator)
            let rate = frame_rate.ratio();
            let fps_numer = u32::try_from(rate.numer().0).map_err(IsoError::size_overflow)?;
            write_u32_be(mvhd_sink, U32Be(fps_numer));

            // Duration in movie-timescale units
            write_u32_be(mvhd_sink, U32Be(duration.0));

            write_u32_be(mvhd_sink, U32Be(0x0001_0000)); // Playback speed (1.0 in 16.16)
            write_u16_be(mvhd_sink, U16Be(0x0100)); // Volume (1.0 in 8.8)
            mvhd_sink.0.extend_from_slice(&[0; 10]); // Reserved

            // Matrix (identity)
            write_i32_be(mvhd_sink, I32Be(0x0001_0000));
            write_i32_be(mvhd_sink, I32Be(0));
            write_i32_be(mvhd_sink, I32Be(0));
            write_i32_be(mvhd_sink, I32Be(0));
            write_i32_be(mvhd_sink, I32Be(0x0001_0000));
            write_i32_be(mvhd_sink, I32Be(0));
            write_i32_be(mvhd_sink, I32Be(0));
            write_i32_be(mvhd_sink, I32Be(0));
            write_i32_be(mvhd_sink, I32Be(0x4000_0000));

            // ISO/IEC 14496-12 MovieHeaderBox: six reserved `pre_defined` u32
            // fields (preview_time, preview_duration, poster_time,
            // selection_time, selection_duration, current_time), then
            // next_track_ID -- not three fields, which undercounts this box's
            // real body by 16 bytes.
            write_u32_be(mvhd_sink, U32Be(0)); // Preview time
            write_u32_be(mvhd_sink, U32Be(0)); // Preview duration
            write_u32_be(mvhd_sink, U32Be(0)); // Poster time
            write_u32_be(mvhd_sink, U32Be(0)); // Selection time
            write_u32_be(mvhd_sink, U32Be(0)); // Selection duration
            write_u32_be(mvhd_sink, U32Be(0)); // Current time
            write_u32_be(mvhd_sink, U32Be(2)); // Next track ID

            Ok(())
        },
    )
}

/// Writes the trak box (track).
fn write_trak(
    sink: &mut ByteSink<'_>,
    frame_rates: FrameRates,
    frames: FrameList<'_>,
    track_params: TrackParams,
    movie_duration: MovieDuration,
) -> Result<StcoOffsetInfo, IsoError> {
    write_box(
        sink,
        BoxType(*b"trak"),
        |trak_sink| -> Result<StcoOffsetInfo, IsoError> {
            // Write tkhd (track header)
            write_tkhd(
                trak_sink,
                movie_duration,
                track_params.width,
                track_params.height,
                &track_params.track_matrix,
            )?;

            // Write mdia (media) and get stco offset info
            write_mdia(trak_sink, frame_rates, frames, track_params)
        },
    )
}

/// Writes the tkhd box (track header).
fn write_tkhd(
    sink: &mut ByteSink<'_>,
    movie_duration: MovieDuration,
    width: Width,
    height: Height,
    track_matrix: &TrackMatrix,
) -> Result<(), IsoError> {
    write_box(sink, BoxType(*b"tkhd"), |tkhd_sink| {
        tkhd_sink.0.push(0); // Version 0
        tkhd_sink.0.extend_from_slice(&[0, 0, 0x0f]); // Flags (track_enabled | track_in_movie | track_in_preview)
        write_u32_be(tkhd_sink, U32Be(0)); // Creation time
        write_u32_be(tkhd_sink, U32Be(0)); // Modification time
        write_u32_be(tkhd_sink, U32Be(1)); // Track ID
        write_u32_be(tkhd_sink, U32Be(0)); // Reserved

        // Duration in the movie timescale, equal to the mvhd duration
        // (ISO/IEC 14496-12, TrackHeaderBox)
        write_u32_be(tkhd_sink, U32Be(movie_duration.0));
        tkhd_sink.0.extend_from_slice(&[0; 8]); // Reserved

        write_u16_be(tkhd_sink, U16Be(0)); // Layer
        write_u16_be(tkhd_sink, U16Be(0)); // Alternate group
        write_u16_be(tkhd_sink, U16Be(0x0100)); // Volume (1.0 in 8.8)
        tkhd_sink.0.extend_from_slice(&[0; 2]); // Reserved

        // Write the track matrix
        let matrix = track_matrix.as_array();
        for &value in &matrix {
            write_i32_be(tkhd_sink, I32Be(value));
        }

        // Width and height in 16.16 fixed point: the integer part in the
        // high 16 bits (big-endian, so written first) and a zero fraction.
        let [width_hi, width_lo] = width.get().get().to_be_bytes();
        let [height_hi, height_lo] = height.get().get().to_be_bytes();
        tkhd_sink.0.extend_from_slice(&[width_hi, width_lo, 0, 0]);
        tkhd_sink.0.extend_from_slice(&[height_hi, height_lo, 0, 0]);

        Ok(())
    })
}

/// Writes the mdia box (media).
fn write_mdia(
    sink: &mut ByteSink<'_>,
    frame_rates: FrameRates,
    frames: FrameList<'_>,
    track_params: TrackParams,
) -> Result<StcoOffsetInfo, IsoError> {
    write_box(
        sink,
        BoxType(*b"mdia"),
        |mdia_sink| -> Result<StcoOffsetInfo, IsoError> {
            // Media timing follows the playback rate, so the container advertises
            // one consistent rate regardless of the capture rate.
            let playback = frame_rates.playback();

            // Write mdhd (media header) at the playback rate
            let frame_count =
                FrameCount::new(u32::try_from(frames.0.len()).map_err(IsoError::size_overflow)?);
            write_mdhd(mdia_sink, playback, frame_count)?;

            // Write hdlr (handler reference)
            write_hdlr(mdia_sink)?;

            // Write minf (media information) and get stco offset info
            let stco_info = write_minf(
                mdia_sink,
                frames,
                Dimensions::new(track_params.width, track_params.height),
                track_params.codec,
                playback,
            )?;

            Ok(stco_info)
        },
    )
}

/// Writes the udta box (user data), which holds the capture-rate metadata.
fn write_udta(sink: &mut ByteSink<'_>, capture: FrameRate) -> Result<(), IsoError> {
    write_box(sink, BoxType(*b"udta"), |udta_sink| {
        write_cfps(udta_sink, capture)
    })
}

/// Writes the cfps box: the exact reduced capture rate as big-endian
/// numerator then denominator, after a version 0 header with zero flags.
fn write_cfps(sink: &mut ByteSink<'_>, capture: FrameRate) -> Result<(), IsoError> {
    write_box(
        sink,
        BoxType(*b"cfps"),
        |cfps_sink| -> Result<(), IsoError> {
            cfps_sink.0.push(0); // Version 0
            cfps_sink.0.extend_from_slice(&[0, 0, 0]); // Flags

            let rate = capture.ratio();
            let numer = u32::try_from(rate.numer().0).map_err(IsoError::capture_rate_too_large)?;
            let denom =
                u32::try_from(rate.denom().get()).map_err(IsoError::capture_rate_too_large)?;
            write_u32_be(cfps_sink, U32Be(numer));
            write_u32_be(cfps_sink, U32Be(denom));

            Ok(())
        },
    )
}

/// The `QuickTime`-style metadata key that names the capture rate.
const CAPTURE_FPS_KEY: &[u8] = b"com.apple.quicktime.capture.fps";

/// The namespace of a `keys` entry: the metadata handler `mdta`.
const MDTA: [u8; 4] = *b"mdta";

/// The `ilst` item type: the big-endian index of the capture key, which is 1.
const CAPTURE_FPS_INDEX: [u8; 4] = [0, 0, 0, 1];

/// The `data` box type indicator for a big-endian float32 value.
const FLOAT32_BE_INDICATOR: u32 = 23;

/// Writes the meta box: the capture rate as a `QuickTime`-style metadata item.
///
/// It is the last child of `moov`, after `trak`. A capture rate is carried either here or
/// in `udta`, never both. Its layout is a full box, version 0 with zero flags, holding in
/// order
/// - `hdlr`: a full box, handler type `mdta`, the other fields zero, a one-byte null name;
/// - `keys`: a full box, entry count 1, then one entry of u32 size, namespace `mdta`
///   and the key bytes `com.apple.quicktime.capture.fps` (the entry is not a box);
/// - `ilst`: one item box whose type is the big-endian key index 1, holding one `data`
///   box: type indicator 23, locale 0, then the capture rate as a big-endian float32.
fn write_meta(sink: &mut ByteSink<'_>, capture: FrameRate) -> Result<(), IsoError> {
    write_box(
        sink,
        BoxType(*b"meta"),
        |meta_sink| -> Result<(), IsoError> {
            meta_sink.0.extend_from_slice(&[0, 0, 0, 0]); // Version 0 and flags
            write_meta_hdlr(meta_sink)?;
            write_meta_keys(meta_sink)?;
            write_meta_ilst(meta_sink, capture)
        },
    )
}

/// Writes the hdlr box of a meta box: handler type `mdta`, the other fields zero, and
/// a name holding only its null terminator.
fn write_meta_hdlr(sink: &mut ByteSink<'_>) -> Result<(), IsoError> {
    write_box(
        sink,
        BoxType(*b"hdlr"),
        |hdlr_sink| -> Result<(), IsoError> {
            hdlr_sink.0.extend_from_slice(&[0, 0, 0, 0]); // Version 0 and flags
            write_u32_be(hdlr_sink, U32Be(0)); // Pre-defined
            hdlr_sink.0.extend_from_slice(&MDTA); // Handler type
            hdlr_sink.0.extend_from_slice(&[0; 12]); // Reserved
            hdlr_sink.0.push(0); // Name: a null-terminated string holding no characters
            Ok(())
        },
    )
}

/// Writes the keys box: one entry naming the capture rate key in the `mdta` namespace.
fn write_meta_keys(sink: &mut ByteSink<'_>) -> Result<(), IsoError> {
    write_box(
        sink,
        BoxType(*b"keys"),
        |keys_sink| -> Result<(), IsoError> {
            keys_sink.0.extend_from_slice(&[0, 0, 0, 0]); // Version 0 and flags
            write_u32_be(keys_sink, U32Be(1)); // Entry count

            // The entry's size counts its own 4-byte size field and the 4-byte namespace.
            let key_size = u32::try_from(CAPTURE_FPS_KEY.len())
                .map_err(IsoError::size_overflow)?
                .checked_add(8)
                .ok_or(IsoError::SizeOverflow)?;
            write_u32_be(keys_sink, U32Be(key_size));
            keys_sink.0.extend_from_slice(&MDTA);
            keys_sink.0.extend_from_slice(CAPTURE_FPS_KEY);
            Ok(())
        },
    )
}

/// Writes the ilst box: the capture rate as the item the key index 1 names.
fn write_meta_ilst(sink: &mut ByteSink<'_>, capture: FrameRate) -> Result<(), IsoError> {
    let rate = capture.ratio();
    let numer_raw = u32::try_from(rate.numer().0).map_err(IsoError::capture_rate_too_large)?;
    // A zero numerator has no binary32 word; the type refuses it, as an oversized term is refused.
    let numer = NonZeroU32::try_from(numer_raw).map_err(|_error| IsoError::CaptureRateTooLarge)?;
    let denom_raw = u32::try_from(rate.denom().get()).map_err(IsoError::capture_rate_too_large)?;
    let denom = NonZeroU32::try_from(denom_raw).map_err(|_error| IsoError::CaptureRateTooLarge)?;
    let bits = binary32_bits(Numerator(numer), denom)?;

    write_box(
        sink,
        BoxType(*b"ilst"),
        |ilst_sink| -> Result<(), IsoError> {
            write_box(
                ilst_sink,
                BoxType(CAPTURE_FPS_INDEX),
                |item_sink| -> Result<(), IsoError> {
                    write_box(
                        item_sink,
                        BoxType(*b"data"),
                        |data_sink| -> Result<(), IsoError> {
                            write_u32_be(data_sink, U32Be(FLOAT32_BE_INDICATOR));
                            write_u32_be(data_sink, U32Be(0)); // Locale
                            write_u32_be(data_sink, bits);
                            Ok(())
                        },
                    )
                },
            )
        },
    )
}

/// Writes the mdhd box (media header), timed at the playback rate.
fn write_mdhd(
    sink: &mut ByteSink<'_>,
    playback: FrameRate,
    frame_count: FrameCount,
) -> Result<(), IsoError> {
    write_box(
        sink,
        BoxType(*b"mdhd"),
        |mdhd_sink| -> Result<(), IsoError> {
            mdhd_sink.0.push(0); // Version 0
            mdhd_sink.0.extend_from_slice(&[0, 0, 0]); // Flags
            write_u32_be(mdhd_sink, U32Be(0)); // Creation time
            write_u32_be(mdhd_sink, U32Be(0)); // Modification time

            // Timescale: the playback rate numerator
            let rate = playback.ratio();
            let fps_numer = u32::try_from(rate.numer().0).map_err(IsoError::size_overflow)?;
            let fps_denom = u32::try_from(rate.denom().get()).map_err(IsoError::size_overflow)?;
            write_u32_be(mdhd_sink, U32Be(fps_numer));

            // Duration in timescale units: frame_count * fps_denom (no division needed)
            let duration = frame_count
                .get()
                .checked_mul(fps_denom)
                .ok_or(IsoError::SizeOverflow)?;
            write_u32_be(mdhd_sink, U32Be(duration));

            write_u16_be(mdhd_sink, U16Be(0x55c4)); // Language (und - undetermined)
            write_u16_be(mdhd_sink, U16Be(0)); // Quality

            Ok(())
        },
    )
}

/// Writes the hdlr box (handler reference).
fn write_hdlr(sink: &mut ByteSink<'_>) -> Result<(), IsoError> {
    write_box(sink, BoxType(*b"hdlr"), |hdlr_sink| {
        hdlr_sink.0.push(0); // Version 0
        hdlr_sink.0.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(hdlr_sink, U32Be(0)); // Pre-defined
        hdlr_sink.0.extend_from_slice(b"vide"); // Handler type (video)
        hdlr_sink.0.extend_from_slice(&[0; 12]); // Reserved
        let name = b"VideoHandler";
        hdlr_sink.0.extend_from_slice(name); // Handler name
        Ok(())
    })
}

/// Writes the minf box (media information).
fn write_minf(
    sink: &mut ByteSink<'_>,
    frames: FrameList<'_>,
    dimensions: Dimensions,
    codec: IsoCodec,
    playback: FrameRate,
) -> Result<StcoOffsetInfo, IsoError> {
    write_box(
        sink,
        BoxType(*b"minf"),
        |minf_sink| -> Result<StcoOffsetInfo, IsoError> {
            // Write vmhd (video media header)
            write_vmhd(minf_sink)?;

            // Write dinf (data information)
            write_dinf(minf_sink)?;

            // Write stbl (sample table) and get stco offset info
            write_stbl(minf_sink, frames, dimensions, codec, playback)
        },
    )
}

/// Writes the vmhd box (video media header).
fn write_vmhd(sink: &mut ByteSink<'_>) -> Result<(), IsoError> {
    write_box(sink, BoxType(*b"vmhd"), |vmhd_sink| {
        vmhd_sink.0.push(0); // Version 0
        vmhd_sink.0.extend_from_slice(&[0, 0, 0]); // Flags
        write_u16_be(vmhd_sink, U16Be(0)); // Graphics mode
        vmhd_sink
            .0
            .extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff]); // Op color (white)
        Ok(())
    })
}

/// Writes the dinf box (data information).
fn write_dinf(sink: &mut ByteSink<'_>) -> Result<(), IsoError> {
    write_box(sink, BoxType(*b"dinf"), |dinf_sink| {
        // Write dref (data reference)
        write_dref(dinf_sink)
    })
}

/// Writes the dref box (data reference).
fn write_dref(sink: &mut ByteSink<'_>) -> Result<(), IsoError> {
    write_box(sink, BoxType(*b"dref"), |dref_sink| {
        dref_sink.0.push(0); // Version 0
        dref_sink.0.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(dref_sink, U32Be(1)); // Entry count

        // Write url box (self-contained)
        write_box(dref_sink, BoxType(*b"url "), |url_sink| {
            url_sink.0.push(0); // Version 0
            url_sink.0.extend_from_slice(&[0, 0, 0x01]); // Flags (self-contained)
            Ok(())
        })?;

        Ok(())
    })
}
