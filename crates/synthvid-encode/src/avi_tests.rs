//! Tests for AVI container writing.

#[cfg(test)]
use super::*;
use core::num::NonZeroU16;
use synthvid_scene::Ratio;

fn make_width(w: u16) -> Width {
    Width::from_nonzero(NonZeroU16::new(w).unwrap())
}

fn make_height(h: u16) -> Height {
    Height::from_nonzero(NonZeroU16::new(h).unwrap())
}

fn make_frame_rate(num: i64, den: i64) -> FrameRate {
    let ratio = Ratio::new(num, core::num::NonZeroI64::new(den).unwrap()).unwrap();
    FrameRate::from_ratio(ratio).unwrap()
}

#[test]
fn test_encode_uncompressed_rgb() {
    let width = make_width(320);
    let height = make_height(240);
    let frame_rate = make_frame_rate(30, 1);

    // Create a dummy RGB frame (320*240*3 bytes for uncompressed RGB)
    let frame_size = 320 * 240 * 3;
    let frame = vec![0u8; frame_size];

    let result = encode_avi(
        frame_rate,
        &[frame],
        width,
        height,
        AviCodec::UncompressedRgb,
    );
    assert!(result.is_ok());

    let avi_data = result.unwrap();
    // Check RIFF header
    assert_eq!(&avi_data[0..4], b"RIFF");
    // Check AVI identifier
    assert_eq!(&avi_data[8..12], b"AVI ");
}

#[test]
fn test_encode_motion_jpeg() {
    let width = make_width(320);
    let height = make_height(240);
    let frame_rate = make_frame_rate(30, 1);

    // Create dummy JPEG frames (just minimal JPEG headers)
    let jpeg_frame = vec![0xFF, 0xD8, 0xFF, 0xD9]; // Minimal JPEG SOI/EOI
    let frames = vec![jpeg_frame.clone(), jpeg_frame];

    let result = encode_avi(frame_rate, &frames, width, height, AviCodec::MotionJpeg);
    assert!(result.is_ok());

    let avi_data = result.unwrap();
    // Check RIFF header
    assert_eq!(&avi_data[0..4], b"RIFF");
    // Check AVI identifier
    assert_eq!(&avi_data[8..12], b"AVI ");
}

#[test]
fn test_encode_avi_no_frames() {
    let width = make_width(320);
    let height = make_height(240);
    let frame_rate = make_frame_rate(30, 1);

    let result = encode_avi(frame_rate, &[], width, height, AviCodec::UncompressedRgb);
    assert_eq!(result, Err(AviError::NoFrames));
}

#[test]
fn test_encode_avi_frame_rate_with_denominator() {
    let width = make_width(320);
    let height = make_height(240);
    // 30000/1001 frame rate (NTSC)
    let frame_rate = make_frame_rate(30000, 1001);

    let frame = vec![0u8; 320 * 240 * 3];
    let result = encode_avi(
        frame_rate,
        &[frame],
        width,
        height,
        AviCodec::UncompressedRgb,
    );
    assert!(result.is_ok());
}

#[test]
fn test_encode_avi_uncompressed_rgb_has_db_chunk_id() {
    let width = make_width(320);
    let height = make_height(240);
    let frame_rate = make_frame_rate(30, 1);
    let frame = vec![0u8; 320 * 240 * 3];
    let avi_data = encode_avi(
        frame_rate,
        &[frame],
        width,
        height,
        AviCodec::UncompressedRgb,
    )
    .expect("encoding should succeed");

    // movi LIST starts after RIFF(12) + hdrl LIST + avih(56+8) + strl LIST + strh(56+8) + strf(40+8)
    // Find "movi" in the output
    let movi_pos = avi_data
        .windows(4)
        .position(|w| w == b"movi")
        .expect("movi LIST");
    // After "movi" (4 bytes) comes the first chunk: "00db", an uncompressed DIB.
    let list_type_pos = movi_pos + 4;
    assert_eq!(&avi_data[list_type_pos..list_type_pos + 4], b"00db");
}

#[test]
fn test_encode_avi_motion_jpeg_has_dc_chunk_id() {
    let width = make_width(320);
    let height = make_height(240);
    let frame_rate = make_frame_rate(30, 1);
    let jpeg_frame = vec![0xFF, 0xD8, 0xFF, 0xD9];
    let avi_data = encode_avi(
        frame_rate,
        &[jpeg_frame.clone(), jpeg_frame],
        width,
        height,
        AviCodec::MotionJpeg,
    )
    .expect("encoding should succeed");

    let list_type_pos = avi_data.windows(4).position(|w| w == b"movi").unwrap() + 4;
    // First chunk in movi should be "00dc", a compressed frame
    assert_eq!(&avi_data[list_type_pos..list_type_pos + 4], b"00dc");
}

#[test]
fn test_every_codec_is_listed() {
    for codec in AviCodec::ALL {
        // Adding a codec breaks this match, which is the cue to list it in
        // `AviCodec::ALL`.
        match codec {
            AviCodec::MotionJpeg | AviCodec::UncompressedRgb => {}
        }
        assert_eq!(AviCodec::of_frame_chunk(codec.frame_chunk_tag()), Ok(codec));
    }
    assert_eq!(AviCodec::ALL.len(), 2);
}

#[test]
fn test_encode_avi_idx1_entries_match_codec() {
    // Uncompressed RGB: idx1 should use 00db
    let width = make_width(320);
    let height = make_height(240);
    let frame_rate = make_frame_rate(30, 1);
    let frame = vec![0u8; 320 * 240 * 3];
    let avi_data = encode_avi(
        frame_rate,
        &[frame],
        width,
        height,
        AviCodec::UncompressedRgb,
    )
    .expect("encoding should succeed");

    // Find idx1 chunk
    let idx1_pos = avi_data
        .windows(4)
        .position(|w| w == b"idx1")
        .expect("idx1 chunk");
    // idx1 data starts after "idx1" + size (8 bytes)
    let idx1_data = &avi_data[idx1_pos + 8..];
    // First 4 bytes of each index entry should be 00db
    assert_eq!(&idx1_data[0..4], b"00db");

    // Motion-JPEG: idx1 should use 00dc
    let jpeg_frame = vec![0xFF, 0xD8, 0xFF, 0xD9];
    let avi_data_mjpeg = encode_avi(
        frame_rate,
        &[jpeg_frame.clone(), jpeg_frame],
        width,
        height,
        AviCodec::MotionJpeg,
    )
    .expect("encoding should succeed");

    let idx1_pos_mjpeg = avi_data_mjpeg
        .windows(4)
        .position(|w| w == b"idx1")
        .expect("idx1 chunk");
    let idx1_data_mjpeg = &avi_data_mjpeg[idx1_pos_mjpeg + 8..];
    assert_eq!(&idx1_data_mjpeg[0..4], b"00dc");
}

#[test]
fn test_encode_avi_rejects_mismatched_uncompressed_rgb_size() {
    let width = make_width(32);
    let height = make_height(16);
    let frame_rate = make_frame_rate(25, 1);
    let result = encode_avi(
        frame_rate,
        &[vec![0u8; 10]],
        width,
        height,
        AviCodec::UncompressedRgb,
    );
    assert_eq!(result, Err(AviError::InvalidDimensions));
}

#[test]
fn test_encode_avi_accepts_correctly_sized_uncompressed_rgb() {
    let width = make_width(32);
    let height = make_height(16);
    let frame_rate = make_frame_rate(25, 1);
    let result = encode_avi(
        frame_rate,
        &[vec![0u8; 32 * 16 * 3]],
        width,
        height,
        AviCodec::UncompressedRgb,
    );
    assert!(result.is_ok());
}

#[test]
fn test_encode_avi_rejects_empty_motion_jpeg_frame() {
    let width = make_width(320);
    let height = make_height(240);
    let frame_rate = make_frame_rate(30, 1);
    let result = encode_avi(frame_rate, &[vec![]], width, height, AviCodec::MotionJpeg);
    assert_eq!(result, Err(AviError::InvalidDimensions));
}

/// A chunk found by [`riff_chunks`]: its tag, the offset of its payload,
/// and the length its size field declares.
#[derive(Debug, PartialEq, Eq)]
struct WalkedChunk {
    tag: [u8; 4],
    payload: usize,
    size: usize,
}

/// Walks the chunks in `begin..end` the way a plain RIFF reader does,
/// without consulting idx1: each chunk is followed by one pad byte when
/// its size is odd. Panics unless the last chunk ends exactly at `end`.
fn riff_chunks(data: &[u8], begin: usize, end: usize) -> Vec<WalkedChunk> {
    let mut chunks = Vec::new();
    let mut offset = begin;
    while offset < end {
        let (tag, rest) = data[offset..].split_first_chunk::<4>().expect("tag");
        let (size, _) = rest.split_first_chunk::<4>().expect("size");
        let size = usize::try_from(u32::from_le_bytes(*size)).expect("size fits");
        let payload = offset.checked_add(8).expect("payload offset");
        chunks.push(WalkedChunk {
            tag: *tag,
            payload,
            size,
        });
        offset = payload
            .checked_add(size.next_multiple_of(2))
            .expect("next chunk offset");
    }
    assert_eq!(offset, end, "chunks overrun their parent");
    chunks
}

/// An MJPEG AVI whose first frame is 9 bytes (1 mod 4) and is followed by
/// a 6-byte frame, so a reader that pads to anything but 2 bytes misplaces
/// the second chunk and everything after it.
fn odd_mjpeg_avi() -> (Vec<u8>, [Vec<u8>; 2]) {
    let frames = [
        vec![0xFF, 0xD8, 0x11, 0x22, 0x33, 0x44, 0x55, 0xFF, 0xD9],
        vec![0xFF, 0xD8, 0x66, 0x77, 0xFF, 0xD9],
    ];
    let avi = encode_avi(
        make_frame_rate(30, 1),
        &frames,
        make_width(16),
        make_height(16),
        AviCodec::MotionJpeg,
    )
    .expect("encoding should succeed");
    (avi, frames)
}

#[test]
fn test_encode_avi_odd_mjpeg_frame_walks_as_plain_riff() {
    let (avi, frames) = odd_mjpeg_avi();
    let riff = riff_chunks(&avi, 0, avi.len());
    assert_eq!(riff.len(), 1);
    assert_eq!(riff[0].tag, *b"RIFF");
    assert_eq!(riff[0].payload + riff[0].size, avi.len());

    let top = riff_chunks(&avi, 12, avi.len());
    let tags: Vec<_> = top.iter().map(|c| c.tag).collect();
    assert_eq!(tags, [*b"LIST", *b"LIST", *b"idx1"]);
    let movi = &top[1];
    assert_eq!(&avi[movi.payload..movi.payload + 4], b"movi");

    let movi_data = movi.payload + 4;
    let walked = riff_chunks(&avi, movi_data, movi.payload + movi.size);
    assert_eq!(walked.len(), 2);
    for (chunk, frame) in walked.iter().zip(&frames) {
        assert_eq!(chunk.tag, *b"00dc");
        assert_eq!(&avi[chunk.payload..chunk.payload + chunk.size], &frame[..]);
    }
    // Exactly one zero pad byte follows the 9-byte frame.
    assert_eq!(avi[walked[0].payload + 9], 0);
    assert_eq!(walked[1].payload, walked[0].payload + 9 + 1 + 8);
}

#[test]
fn test_encode_avi_odd_mjpeg_idx1_matches_walked_chunks() {
    let (avi, _) = odd_mjpeg_avi();
    let top = riff_chunks(&avi, 12, avi.len());
    let movi_tag = top[1].payload;
    let walked = riff_chunks(&avi, movi_tag + 4, top[1].payload + top[1].size);
    let idx1 = &top[2];
    assert_eq!(idx1.size, 16 * walked.len());
    for (i, chunk) in walked.iter().enumerate() {
        let entry = &avi[idx1.payload + 16 * i..idx1.payload + 16 * (i + 1)];
        let field = |at: usize| {
            usize::try_from(u32::from_le_bytes(entry[at..at + 4].try_into().unwrap())).unwrap()
        };
        assert_eq!(entry[0..4], chunk.tag);
        // Offsets count from the "movi" list type, so the first frame is at 4.
        assert_eq!(field(8), chunk.payload - 8 - movi_tag);
        assert_eq!(field(12), chunk.size);
    }
}
