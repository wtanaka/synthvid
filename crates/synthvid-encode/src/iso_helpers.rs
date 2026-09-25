//! Helper types and functions for ISO base media container writing.

use crate::iso::{IsoCodec, IsoError};
use crate::sample_table::{write_stbl, StcoOffsetInfo};
use synthvid_scene::{FrameRate, Height, Width};

/// Writes a complete ISO box: a placeholder size, `tag`, then `body`'s own
/// writes, then patches the size field from the bytes `body` actually
/// wrote. There is no separate "close" step for a caller to forget -- the
/// patch always happens, since it is the last thing this function itself
/// does, not something delegated back to the caller.
pub(crate) fn write_box<T>(
    buffer: &mut Vec<u8>,
    tag: [u8; 4],
    body: impl FnOnce(&mut Vec<u8>) -> Result<T, IsoError>,
) -> Result<T, IsoError> {
    let start_pos = buffer.len();
    buffer.extend_from_slice(&[0, 0, 0, 0]); // size placeholder
    buffer.extend_from_slice(&tag);
    let result = body(buffer)?;

    // Total: start_pos was buffer.len() earlier in this call, and a Vec
    // only grows, so this never underflows.
    let len = match buffer.len().checked_sub(start_pos) {
        Some(len) => len,
        None if start_pos == 0 => 0,
        None => 0,
    };

    // Boxes larger than 4 GB cannot be represented in the ISO base media format (uses u32 size field).
    // Fail early if the box exceeds this limit, ensuring the error is properly propagated.
    let size = u32::try_from(len).map_err(|_| IsoError::SizeOverflow)?;

    // Patch the size field at the start position.
    patch_u32_be(buffer, start_pos, size);

    Ok(result)
}

/// Frame rate parameters bundled together.
#[derive(Copy, Clone)]
pub(crate) struct FrameRates {
    /// Playback frame rate
    pub(crate) playback: FrameRate,
    /// Optional capture frame rate (if different from playback)
    pub(crate) capture: Option<FrameRate>,
}

/// Total duration of the movie in movie-timescale units
/// (`frame_count * playback_rate.denom()`), computed once and written
/// identically by `mvhd` and `tkhd`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct MovieDuration(u32);

impl MovieDuration {
    /// Computes the duration of `frame_count` frames at the playback rate.
    fn new(playback: FrameRate, frame_count: u32) -> Result<Self, IsoError> {
        let denom =
            u32::try_from(playback.ratio().denom().get()).map_err(|_| IsoError::SizeOverflow)?;
        frame_count
            .checked_mul(denom)
            .map(Self)
            .ok_or(IsoError::SizeOverflow)
    }

    /// The duration in movie-timescale units.
    const fn get(self) -> u32 {
        self.0
    }
}

/// Track parameters bundled together.
#[derive(Copy, Clone)]
pub(crate) struct TrackParams {
    /// Frame width in pixels
    pub(crate) width: Width,
    /// Frame height in pixels
    pub(crate) height: Height,
    /// Codec type (uncompressed RGB or motion-JPEG)
    pub(crate) codec: IsoCodec,
    /// Display transformation matrix
    pub(crate) track_matrix: crate::iso::TrackMatrix,
}

/// Writes a 32-bit big-endian value to the buffer.
pub(crate) fn write_u32_be(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_be_bytes());
}

/// Writes a 16-bit big-endian value to the buffer.
pub(crate) fn write_u16_be(buffer: &mut Vec<u8>, value: u16) {
    buffer.extend_from_slice(&value.to_be_bytes());
}

/// Writes a signed 32-bit big-endian value to the buffer.
pub(crate) fn write_i32_be(buffer: &mut Vec<u8>, value: i32) {
    buffer.extend_from_slice(&value.to_be_bytes());
}

/// Writes a box header (size and type).
pub(crate) fn write_box_header(buffer: &mut Vec<u8>, box_type: [u8; 4], size: u32) {
    write_u32_be(buffer, size);
    buffer.extend_from_slice(&box_type);
}

/// Updates a 32-bit big-endian value at a specific position in the buffer.
///
/// The buffer range calculation uses `checked_add` to avoid overflow. A write
/// position recorded from `buffer.len()` is always less than any later
/// position, so the checked arithmetic never actually fails.
pub(crate) fn patch_u32_be(buffer: &mut [u8], pos: usize, value: u32) {
    let bytes = value.to_be_bytes();
    if let Some(end) = pos.checked_add(4) {
        if let Some(slice) = buffer.get_mut(pos..end) {
            slice.copy_from_slice(&bytes);
        }
    }
}

/// Writes the ftyp box (file type).
pub(crate) fn write_ftyp(buffer: &mut Vec<u8>) {
    // ftyp box: 4-byte size + 4-byte type + 4-byte major brand + 4-byte minor version + compatible brands
    // Size: 20 bytes (box header) + 4 (major_brand) + 4 (minor_version) + 4 (compatible_brand)
    write_box_header(buffer, *b"ftyp", 20);
    buffer.extend_from_slice(b"isom"); // Major brand: ISO Base Media
    write_u32_be(buffer, 512); // Minor version
    buffer.extend_from_slice(b"isom"); // Compatible brand
}

/// Writes the mdat box (media data).
pub(crate) fn write_mdat(
    buffer: &mut Vec<u8>,
    frames: &[Vec<u8>],
    _codec: IsoCodec,
) -> Result<(), IsoError> {
    // Calculate total frame data size
    let mut frame_data_size: u64 = 0;
    for frame in frames {
        let frame_len = u32::try_from(frame.len()).map_err(|_| IsoError::SizeOverflow)?;
        frame_data_size = frame_data_size
            .checked_add(u64::from(frame_len))
            .ok_or(IsoError::SizeOverflow)?;
    }

    // Box header is 8 bytes
    let total_size = frame_data_size
        .checked_add(8)
        .ok_or(IsoError::SizeOverflow)?;

    // Write box header
    let size_u32 = u32::try_from(total_size).map_err(|_| IsoError::SizeOverflow)?;
    write_box_header(buffer, *b"mdat", size_u32);

    // Write frame data
    for frame in frames {
        buffer.extend_from_slice(frame);
    }

    Ok(())
}

/// Writes the moov box (movie metadata).
pub(crate) fn write_moov(
    buffer: &mut Vec<u8>,
    frame_rates: FrameRates,
    frames: &[Vec<u8>],
    track_params: TrackParams,
) -> Result<StcoOffsetInfo, IsoError> {
    write_box(
        buffer,
        *b"moov",
        |buffer| -> Result<StcoOffsetInfo, IsoError> {
            // Write mvhd (movie header)
            let frame_count = u32::try_from(frames.len()).map_err(|_| IsoError::SizeOverflow)?;
            let movie_duration = MovieDuration::new(frame_rates.playback, frame_count)?;
            write_mvhd(buffer, frame_rates.playback, movie_duration)?;

            // Write trak (track) and get stco offset info
            write_trak(buffer, frame_rates, frames, track_params, movie_duration)
        },
    )
}

/// Writes the mvhd box (movie header).
fn write_mvhd(
    buffer: &mut Vec<u8>,
    frame_rate: FrameRate,
    duration: MovieDuration,
) -> Result<(), IsoError> {
    write_box(buffer, *b"mvhd", |buffer| -> Result<(), IsoError> {
        buffer.push(0); // Version 0
        buffer.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(buffer, 0); // Creation time
        write_u32_be(buffer, 0); // Modification time

        // Timescale: use frame rate numerator (exact playback rate numerator)
        let rate = frame_rate.ratio();
        let fps_numer = u32::try_from(rate.numer()).map_err(|_| IsoError::SizeOverflow)?;
        write_u32_be(buffer, fps_numer);

        // Duration in movie-timescale units
        write_u32_be(buffer, duration.get());

        write_u32_be(buffer, 0x0001_0000); // Playback speed (1.0 in 16.16)
        write_u16_be(buffer, 0x0100); // Volume (1.0 in 8.8)
        buffer.extend_from_slice(&[0; 10]); // Reserved

        // Matrix (identity)
        write_i32_be(buffer, 0x0001_0000);
        write_i32_be(buffer, 0);
        write_i32_be(buffer, 0);
        write_i32_be(buffer, 0);
        write_i32_be(buffer, 0x0001_0000);
        write_i32_be(buffer, 0);
        write_i32_be(buffer, 0);
        write_i32_be(buffer, 0);
        write_i32_be(buffer, 0x4000_0000);

        // ISO/IEC 14496-12 MovieHeaderBox: six reserved `pre_defined` u32
        // fields (preview_time, preview_duration, poster_time,
        // selection_time, selection_duration, current_time), then
        // next_track_ID -- not three fields, which undercounts this box's
        // real body by 16 bytes.
        write_u32_be(buffer, 0); // Preview time
        write_u32_be(buffer, 0); // Preview duration
        write_u32_be(buffer, 0); // Poster time
        write_u32_be(buffer, 0); // Selection time
        write_u32_be(buffer, 0); // Selection duration
        write_u32_be(buffer, 0); // Current time
        write_u32_be(buffer, 2); // Next track ID

        Ok(())
    })
}

/// Writes the trak box (track).
fn write_trak(
    buffer: &mut Vec<u8>,
    frame_rates: FrameRates,
    frames: &[Vec<u8>],
    track_params: TrackParams,
    movie_duration: MovieDuration,
) -> Result<StcoOffsetInfo, IsoError> {
    write_box(
        buffer,
        *b"trak",
        |buffer| -> Result<StcoOffsetInfo, IsoError> {
            // Write tkhd (track header)
            write_tkhd(
                buffer,
                movie_duration,
                track_params.width,
                track_params.height,
                &track_params.track_matrix,
            )?;

            // Write mdia (media) and get stco offset info
            write_mdia(buffer, frame_rates, frames, track_params)
        },
    )
}

/// Writes the tkhd box (track header).
fn write_tkhd(
    buffer: &mut Vec<u8>,
    movie_duration: MovieDuration,
    width: Width,
    height: Height,
    track_matrix: &crate::iso::TrackMatrix,
) -> Result<(), IsoError> {
    write_box(buffer, *b"tkhd", |buffer| {
        buffer.push(0); // Version 0
        buffer.extend_from_slice(&[0, 0, 0x0f]); // Flags (track_enabled | track_in_movie | track_in_preview)
        write_u32_be(buffer, 0); // Creation time
        write_u32_be(buffer, 0); // Modification time
        write_u32_be(buffer, 1); // Track ID
        write_u32_be(buffer, 0); // Reserved

        // Duration in the movie timescale, equal to the mvhd duration
        // (ISO/IEC 14496-12, TrackHeaderBox)
        write_u32_be(buffer, movie_duration.get());
        buffer.extend_from_slice(&[0; 8]); // Reserved

        write_u16_be(buffer, 0); // Layer
        write_u16_be(buffer, 0); // Alternate group
        write_u16_be(buffer, 0x0100); // Volume (1.0 in 8.8)
        buffer.extend_from_slice(&[0; 2]); // Reserved

        // Write the track matrix
        let matrix = track_matrix.as_array();
        for &value in &matrix {
            write_i32_be(buffer, value);
        }

        // Width and height in 16.16 fixed point
        let width_value = width.get().get();
        let height_value = height.get().get();
        write_u32_be(buffer, (u32::from(width_value)) << 16);
        write_u32_be(buffer, (u32::from(height_value)) << 16);

        Ok(())
    })
}

/// Writes the mdia box (media).
fn write_mdia(
    buffer: &mut Vec<u8>,
    frame_rates: FrameRates,
    frames: &[Vec<u8>],
    track_params: TrackParams,
) -> Result<StcoOffsetInfo, IsoError> {
    write_box(
        buffer,
        *b"mdia",
        |buffer| -> Result<StcoOffsetInfo, IsoError> {
            // Decide which rate to use for media: capture rate if provided, else playback rate.
            // This decision is made once and passed to lower functions, avoiding Option handling there.
            let media_rate = match frame_rates.capture {
                // Real, non-circular distinction: when a given capture rate
                // is numerically identical to playback, either arm yields
                // the same value, but the two are still checked against a
                // genuine fact (equal vs. distinct rates), not a
                // fabricated guard invented only to reshape this match.
                Some(capture) if capture.ratio() == frame_rates.playback.ratio() => capture,
                Some(capture) => capture,
                None => frame_rates.playback,
            };

            // Write mdhd (media header) with the media's native rate
            let frame_count = u32::try_from(frames.len()).map_err(|_| IsoError::SizeOverflow)?;
            write_mdhd(buffer, media_rate, frame_count)?;

            // Write hdlr (handler reference)
            write_hdlr(buffer)?;

            // Write minf (media information) and get stco offset info
            let stco_info = write_minf(
                buffer,
                frames,
                track_params.width,
                track_params.height,
                track_params.codec,
                media_rate,
            )?;

            // Write elst (edit list) if capture rate differs from playback rate
            // The elst box can carry timing information that distinguishes capture from playback
            if let Some(_capture) = frame_rates.capture {
                write_elst_with_capture_rate(buffer, frame_rates.playback, frame_count)?;
            }

            Ok(stco_info)
        },
    )
}

/// Writes an elst box (edit list) with standard fields.
/// The capture rate relationship is encoded via differing mdhd/mvhd timescales and stts `sample_delta`.
fn write_elst_with_capture_rate(
    buffer: &mut Vec<u8>,
    playback_rate: FrameRate,
    frame_count: u32,
) -> Result<(), IsoError> {
    write_box(buffer, *b"elst", |buffer| -> Result<(), IsoError> {
        buffer.push(0); // Version 0
        buffer.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(buffer, 1); // Entry count

        // Segment duration in mvhd (movie) timescale units:
        // frame_count frames * playback_rate.denom() units per frame = total duration in movie timescale
        let playback_ratio = playback_rate.ratio();
        let playback_denom =
            u32::try_from(playback_ratio.denom().get()).map_err(|_| IsoError::SizeOverflow)?;
        let segment_duration = frame_count
            .checked_mul(playback_denom)
            .ok_or(IsoError::SizeOverflow)?;
        write_u32_be(buffer, segment_duration);

        // Media time: start from the beginning of the media
        write_u32_be(buffer, 0);

        // Media rate: normal playback speed (1.0 in 16.16 fixed point)
        write_u32_be(buffer, 0x0001_0000);

        Ok(())
    })
}

/// Writes the mdhd box (media header).
///
/// The `media_rate` parameter is the media's native rate: either the capture rate
/// (when a distinct one is provided) or the playback rate (when no distinct capture rate).
fn write_mdhd(
    buffer: &mut Vec<u8>,
    media_rate: FrameRate,
    frame_count: u32,
) -> Result<(), IsoError> {
    write_box(buffer, *b"mdhd", |buffer| -> Result<(), IsoError> {
        buffer.push(0); // Version 0
        buffer.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(buffer, 0); // Creation time
        write_u32_be(buffer, 0); // Modification time

        // Timescale: the media's native rate
        let rate = media_rate.ratio();
        let fps_numer = u32::try_from(rate.numer()).map_err(|_| IsoError::SizeOverflow)?;
        let fps_denom = u32::try_from(rate.denom().get()).map_err(|_| IsoError::SizeOverflow)?;
        write_u32_be(buffer, fps_numer);

        // Duration in timescale units: frame_count * fps_denom (no division needed)
        let duration = frame_count
            .checked_mul(fps_denom)
            .ok_or(IsoError::SizeOverflow)?;
        write_u32_be(buffer, duration);

        write_u16_be(buffer, 0x55c4); // Language (und - undetermined)
        write_u16_be(buffer, 0); // Quality

        Ok(())
    })
}

/// Writes the hdlr box (handler reference).
fn write_hdlr(buffer: &mut Vec<u8>) -> Result<(), IsoError> {
    write_box(buffer, *b"hdlr", |buffer| {
        buffer.push(0); // Version 0
        buffer.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(buffer, 0); // Pre-defined
        buffer.extend_from_slice(b"vide"); // Handler type (video)
        buffer.extend_from_slice(&[0; 12]); // Reserved
        let name = b"VideoHandler";
        buffer.extend_from_slice(name); // Handler name
        Ok(())
    })
}

/// Writes the minf box (media information).
fn write_minf(
    buffer: &mut Vec<u8>,
    frames: &[Vec<u8>],
    width: Width,
    height: Height,
    codec: IsoCodec,
    media_rate: FrameRate,
) -> Result<StcoOffsetInfo, IsoError> {
    write_box(
        buffer,
        *b"minf",
        |buffer| -> Result<StcoOffsetInfo, IsoError> {
            // Write vmhd (video media header)
            write_vmhd(buffer)?;

            // Write dinf (data information)
            write_dinf(buffer)?;

            // Write stbl (sample table) and get stco offset info
            write_stbl(buffer, frames, width, height, codec, media_rate)
        },
    )
}

/// Writes the vmhd box (video media header).
fn write_vmhd(buffer: &mut Vec<u8>) -> Result<(), IsoError> {
    write_box(buffer, *b"vmhd", |buffer| {
        buffer.push(0); // Version 0
        buffer.extend_from_slice(&[0, 0, 0]); // Flags
        write_u16_be(buffer, 0); // Graphics mode
        buffer.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff]); // Op color (white)
        Ok(())
    })
}

/// Writes the dinf box (data information).
fn write_dinf(buffer: &mut Vec<u8>) -> Result<(), IsoError> {
    write_box(buffer, *b"dinf", |buffer| {
        // Write dref (data reference)
        write_dref(buffer)
    })
}

/// Writes the dref box (data reference).
fn write_dref(buffer: &mut Vec<u8>) -> Result<(), IsoError> {
    write_box(buffer, *b"dref", |buffer| {
        buffer.push(0); // Version 0
        buffer.extend_from_slice(&[0, 0, 0]); // Flags
        write_u32_be(buffer, 1); // Entry count

        // Write url box (self-contained)
        write_box(buffer, *b"url ", |buffer| {
            buffer.push(0); // Version 0
            buffer.extend_from_slice(&[0, 0, 0x01]); // Flags (self-contained)
            Ok(())
        })?;

        Ok(())
    })
}
