//! Tests for AVI container writing.

#[cfg(test)]
use super::*;
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

/// An encoded AVI file.
struct Avi(ContainerFile);

/// A byte offset into an encoded file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Offset(usize);

/// A length in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Len(usize);

/// A four-byte chunk tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tag([u8; 4]);

/// The bytes of one encoded frame.
struct FrameBytes(Vec<u8>);

/// The bytes from `begin` up to `end`.
#[derive(Clone, Copy, Debug)]
struct Span {
    /// The first byte of the span.
    begin: Offset,
    /// The byte just past the span.
    end: Offset,
}

impl Offset {
    /// The offset `len` bytes further on.
    fn after(self, len: Len) -> Result<Self, Fail> {
        self.0.checked_add(len.0).map(Self).or_fail()
    }
}

impl Avi {
    /// The offset just past the last byte.
    const fn end(&self) -> Offset {
        Offset(self.0 .0.len())
    }

    /// The four bytes at `at`.
    fn tag_at(&self, at: Offset) -> Result<Tag, Fail> {
        self.0
             .0
            .get(at.0..)
            .and_then(<[u8]>::first_chunk::<4>)
            .map(|tag| Tag(*tag))
            .or_fail()
    }

    /// The little-endian 32-bit word at `at`.
    fn word_at(&self, at: Offset) -> Result<Len, Fail> {
        let Tag(word) = self.tag_at(at)?;
        usize::try_from(u32::from_le_bytes(word)).map(Len).or_fail()
    }

    /// The offset of the first occurrence of `tag`.
    fn find(&self, tag: Tag) -> Result<Offset, Fail> {
        self.0
             .0
            .windows(4)
            .position(|window| window == tag.0)
            .map(Offset)
            .or_fail()
    }

    /// Fails unless the `len` bytes at `at` are `expected`.
    fn ensure_holds(&self, at: Offset, len: Len, expected: &FrameBytes) -> Result<(), Fail> {
        let end = at.after(len)?;
        ensure_eq!(self.0 .0.get(at.0..end.0), Some(expected.0.as_slice()));
        Ok(())
    }
}

/// The 320×240 frame size most tests encode.
fn qvga() -> Result<(Width, Height), Fail> {
    Ok((Width::new(320).or_fail()?, Height::new(240).or_fail()?))
}

#[test]
fn encode_uncompressed_rgb() {
    assert_eq!(check_encode_uncompressed_rgb(), Ok(()));
}

fn check_encode_uncompressed_rgb() -> Result<(), Fail> {
    let (width, height) = qvga()?;
    let frame_rate = FrameRate::from_fps(30).or_fail()?;

    // A dummy RGB frame: 320 × 240 pixels × 3 bytes.
    let frame = EncodedFrame(vec![0_u8; 230_400]);

    let avi_data = encode_avi(
        frame_rate,
        &[frame],
        width,
        height,
        AviCodec::UncompressedRgb,
    )
    .or_fail()?;
    // Check RIFF header
    ensure_eq!(avi_data.as_bytes().get(0..4), Some(b"RIFF".as_slice()));
    // Check AVI identifier
    ensure_eq!(avi_data.as_bytes().get(8..12), Some(b"AVI ".as_slice()));
    Ok(())
}

#[test]
fn encode_motion_jpeg() {
    assert_eq!(check_encode_motion_jpeg(), Ok(()));
}

fn check_encode_motion_jpeg() -> Result<(), Fail> {
    let (width, height) = qvga()?;
    let frame_rate = FrameRate::from_fps(30).or_fail()?;

    // Create dummy JPEG frames (just minimal JPEG headers)
    let jpeg_frame = EncodedFrame(vec![0xFF, 0xD8, 0xFF, 0xD9]); // Minimal JPEG SOI/EOI
    let frames = vec![jpeg_frame.clone(), jpeg_frame];

    let avi_data =
        encode_avi(frame_rate, &frames, width, height, AviCodec::MotionJpeg).or_fail()?;
    // Check RIFF header
    ensure_eq!(avi_data.as_bytes().get(0..4), Some(b"RIFF".as_slice()));
    // Check AVI identifier
    ensure_eq!(avi_data.as_bytes().get(8..12), Some(b"AVI ".as_slice()));
    Ok(())
}

#[test]
fn encode_avi_no_frames() {
    assert_eq!(check_encode_avi_no_frames(), Ok(()));
}

fn check_encode_avi_no_frames() -> Result<(), Fail> {
    let (width, height) = qvga()?;
    let frame_rate = FrameRate::from_fps(30).or_fail()?;

    let result = encode_avi(frame_rate, &[], width, height, AviCodec::UncompressedRgb);
    ensure_eq!(result, Err(AviError::NoFrames));
    Ok(())
}

#[test]
fn encode_avi_frame_rate_with_denominator() {
    assert_eq!(check_encode_avi_frame_rate_with_denominator(), Ok(()));
}

fn check_encode_avi_frame_rate_with_denominator() -> Result<(), Fail> {
    let (width, height) = qvga()?;
    // 30000/1001 frame rate (NTSC)
    let frame_rate =
        FrameRate::from_fraction(30_000, NonZeroU32::new(1001).or_fail()?).or_fail()?;

    // 320 × 240 pixels × 3 bytes.
    let frame = EncodedFrame(vec![0_u8; 230_400]);
    let file = encode_avi(
        frame_rate,
        &[frame],
        width,
        height,
        AviCodec::UncompressedRgb,
    )
    .or_fail()?;
    ensure_eq!(file.as_bytes().get(0..4), Some(b"RIFF".as_slice()));
    Ok(())
}

#[test]
fn encode_avi_uncompressed_rgb_has_db_chunk_id() {
    assert_eq!(check_encode_avi_uncompressed_rgb_has_db_chunk_id(), Ok(()));
}

fn check_encode_avi_uncompressed_rgb_has_db_chunk_id() -> Result<(), Fail> {
    let (width, height) = qvga()?;
    let frame_rate = FrameRate::from_fps(30).or_fail()?;
    // 320 × 240 pixels × 3 bytes.
    let frame = EncodedFrame(vec![0_u8; 230_400]);
    let avi_data = Avi(encode_avi(
        frame_rate,
        &[frame],
        width,
        height,
        AviCodec::UncompressedRgb,
    )
    .or_fail()?);

    // movi LIST starts after RIFF(12) + hdrl LIST + avih(56+8) + strl LIST + strh(56+8) + strf(40+8)
    // Find "movi" in the output
    let movi_pos = avi_data.find(Tag(*b"movi"))?;
    // After "movi" (4 bytes) comes the first chunk: "00db", an uncompressed DIB.
    let list_type_pos = movi_pos.after(Len(4))?;
    ensure_eq!(avi_data.tag_at(list_type_pos)?, Tag(*b"00db"));
    Ok(())
}

#[test]
fn encode_avi_motion_jpeg_has_dc_chunk_id() {
    assert_eq!(check_encode_avi_motion_jpeg_has_dc_chunk_id(), Ok(()));
}

fn check_encode_avi_motion_jpeg_has_dc_chunk_id() -> Result<(), Fail> {
    let (width, height) = qvga()?;
    let frame_rate = FrameRate::from_fps(30).or_fail()?;
    let jpeg_frame = EncodedFrame(vec![0xFF, 0xD8, 0xFF, 0xD9]);
    let avi_data = Avi(encode_avi(
        frame_rate,
        &[jpeg_frame.clone(), jpeg_frame],
        width,
        height,
        AviCodec::MotionJpeg,
    )
    .or_fail()?);

    let list_type_pos = avi_data.find(Tag(*b"movi"))?.after(Len(4))?;
    // First chunk in movi should be "00dc", a compressed frame
    ensure_eq!(avi_data.tag_at(list_type_pos)?, Tag(*b"00dc"));
    Ok(())
}

#[test]
fn every_codec_is_listed() {
    for codec in AviCodec::ALL {
        // Adding a codec breaks this match, which is the cue to list it in
        // `AviCodec::ALL`.
        match codec {
            AviCodec::MotionJpeg | AviCodec::UncompressedRgb => {}
        }
        assert_eq!(AviCodec::of_frame_chunk(codec.frame_chunk_id()), Ok(codec));
    }
    assert_eq!(AviCodec::ALL.len(), 2);
}

#[test]
fn encode_avi_idx1_entries_match_codec() {
    assert_eq!(check_encode_avi_idx1_entries_match_codec(), Ok(()));
}

fn check_encode_avi_idx1_entries_match_codec() -> Result<(), Fail> {
    // Uncompressed RGB: idx1 should use 00db
    let (width, height) = qvga()?;
    let frame_rate = FrameRate::from_fps(30).or_fail()?;
    // 320 × 240 pixels × 3 bytes.
    let frame = EncodedFrame(vec![0_u8; 230_400]);
    let avi_data = Avi(encode_avi(
        frame_rate,
        &[frame],
        width,
        height,
        AviCodec::UncompressedRgb,
    )
    .or_fail()?);

    // Find idx1 chunk
    let idx1_pos = avi_data.find(Tag(*b"idx1"))?;
    // idx1 data starts after "idx1" + size (8 bytes)
    let idx1_data = idx1_pos.after(Len(8))?;
    // First 4 bytes of each index entry should be 00db
    ensure_eq!(avi_data.tag_at(idx1_data)?, Tag(*b"00db"));

    // Motion-JPEG: idx1 should use 00dc
    let jpeg_frame = EncodedFrame(vec![0xFF, 0xD8, 0xFF, 0xD9]);
    let avi_data_mjpeg = Avi(encode_avi(
        frame_rate,
        &[jpeg_frame.clone(), jpeg_frame],
        width,
        height,
        AviCodec::MotionJpeg,
    )
    .or_fail()?);

    let idx1_pos_mjpeg = avi_data_mjpeg.find(Tag(*b"idx1"))?;
    let idx1_data_mjpeg = idx1_pos_mjpeg.after(Len(8))?;
    ensure_eq!(avi_data_mjpeg.tag_at(idx1_data_mjpeg)?, Tag(*b"00dc"));
    Ok(())
}

#[test]
fn encode_avi_rejects_mismatched_uncompressed_rgb_size() {
    assert_eq!(
        check_encode_avi_rejects_mismatched_uncompressed_rgb_size(),
        Ok(())
    );
}

fn check_encode_avi_rejects_mismatched_uncompressed_rgb_size() -> Result<(), Fail> {
    let width = Width::new(32).or_fail()?;
    let height = Height::new(16).or_fail()?;
    let frame_rate = FrameRate::from_fps(25).or_fail()?;
    let result = encode_avi(
        frame_rate,
        &[EncodedFrame(vec![0_u8; 10])],
        width,
        height,
        AviCodec::UncompressedRgb,
    );
    ensure_eq!(result, Err(AviError::InvalidDimensions));
    Ok(())
}

#[test]
fn encode_avi_accepts_correctly_sized_uncompressed_rgb() {
    assert_eq!(
        check_encode_avi_accepts_correctly_sized_uncompressed_rgb(),
        Ok(())
    );
}

fn check_encode_avi_accepts_correctly_sized_uncompressed_rgb() -> Result<(), Fail> {
    let width = Width::new(32).or_fail()?;
    let height = Height::new(16).or_fail()?;
    let frame_rate = FrameRate::from_fps(25).or_fail()?;
    let file = encode_avi(
        frame_rate,
        // 32 × 16 pixels × 3 bytes.
        &[EncodedFrame(vec![0_u8; 1_536])],
        width,
        height,
        AviCodec::UncompressedRgb,
    )
    .or_fail()?;
    ensure_eq!(file.as_bytes().get(0..4), Some(b"RIFF".as_slice()));
    Ok(())
}

#[test]
fn encode_avi_rejects_empty_motion_jpeg_frame() {
    assert_eq!(check_encode_avi_rejects_empty_motion_jpeg_frame(), Ok(()));
}

fn check_encode_avi_rejects_empty_motion_jpeg_frame() -> Result<(), Fail> {
    let (width, height) = qvga()?;
    let frame_rate = FrameRate::from_fps(30).or_fail()?;
    let result = encode_avi(
        frame_rate,
        &[EncodedFrame(vec![])],
        width,
        height,
        AviCodec::MotionJpeg,
    );
    ensure_eq!(result, Err(AviError::InvalidDimensions));
    Ok(())
}

/// A chunk found by [`riff_chunks`]: its tag, the offset of its payload,
/// and the length its size field declares.
#[derive(Debug, PartialEq, Eq)]
struct WalkedChunk {
    /// The chunk's four-byte tag.
    tag: Tag,
    /// Where the chunk's payload begins.
    payload: Offset,
    /// The payload length the size field declares.
    size: Len,
}

impl WalkedChunk {
    /// The offset just past the declared payload.
    fn payload_end(&self) -> Result<Offset, Fail> {
        self.payload.after(self.size)
    }
}

/// Walks the chunks in `span` the way a plain RIFF reader does, without
/// consulting idx1: each chunk is followed by one pad byte when its size is
/// odd. Fails unless the last chunk ends exactly at the end of `span`.
fn riff_chunks(data: &Avi, span: Span) -> Result<Vec<WalkedChunk>, Fail> {
    let mut chunks = Vec::new();
    let mut offset = span.begin;
    while offset < span.end {
        let tag = data.tag_at(offset)?;
        let size = data.word_at(offset.after(Len(4))?)?;
        let payload = offset.after(Len(8))?;
        chunks.push(WalkedChunk { tag, payload, size });
        offset = payload.after(Len(size.0.checked_next_multiple_of(2).or_fail()?))?;
    }
    ensure_eq!(offset, span.end, "chunks overrun their parent");
    Ok(chunks)
}

/// An MJPEG AVI whose first frame is 9 bytes (1 mod 4) and is followed by
/// a 6-byte frame, so a reader that pads to anything but 2 bytes misplaces
/// the second chunk and everything after it.
fn odd_mjpeg_avi() -> Result<(Avi, [FrameBytes; 2]), Fail> {
    let frames = [
        EncodedFrame(vec![0xFF, 0xD8, 0x11, 0x22, 0x33, 0x44, 0x55, 0xFF, 0xD9]),
        EncodedFrame(vec![0xFF, 0xD8, 0x66, 0x77, 0xFF, 0xD9]),
    ];
    let avi = encode_avi(
        FrameRate::from_fps(30).or_fail()?,
        &frames,
        Width::new(16).or_fail()?,
        Height::new(16).or_fail()?,
        AviCodec::MotionJpeg,
    )
    .or_fail()?;
    Ok((
        Avi(avi),
        frames.map(|EncodedFrame(bytes)| FrameBytes(bytes)),
    ))
}

#[test]
fn encode_avi_odd_mjpeg_frame_walks_as_plain_riff() {
    assert_eq!(
        check_encode_avi_odd_mjpeg_frame_walks_as_plain_riff(),
        Ok(())
    );
}

fn check_encode_avi_odd_mjpeg_frame_walks_as_plain_riff() -> Result<(), Fail> {
    let (avi, frames) = odd_mjpeg_avi()?;
    let riff = riff_chunks(
        &avi,
        Span {
            begin: Offset(0),
            end: avi.end(),
        },
    )?;
    ensure_eq!(riff.len(), 1);
    let whole = riff.first().or_fail()?;
    ensure_eq!(whole.tag, Tag(*b"RIFF"));
    ensure_eq!(whole.payload_end()?, avi.end());

    let top = riff_chunks(
        &avi,
        Span {
            begin: Offset(12),
            end: avi.end(),
        },
    )?;
    let tags: Vec<_> = top.iter().map(|chunk| chunk.tag).collect();
    ensure_eq!(tags, [Tag(*b"LIST"), Tag(*b"LIST"), Tag(*b"idx1")]);
    let movi = top.get(1).or_fail()?;
    ensure_eq!(avi.tag_at(movi.payload)?, Tag(*b"movi"));

    let movi_data = movi.payload.after(Len(4))?;
    let walked = riff_chunks(
        &avi,
        Span {
            begin: movi_data,
            end: movi.payload_end()?,
        },
    )?;
    ensure_eq!(walked.len(), 2);
    for (chunk, frame) in walked.iter().zip(&frames) {
        ensure_eq!(chunk.tag, Tag(*b"00dc"));
        avi.ensure_holds(chunk.payload, chunk.size, frame)?;
    }
    // Exactly one zero pad byte follows the 9-byte frame.
    let first = walked.first().or_fail()?;
    let pad = first.payload.after(Len(9))?;
    ensure_eq!(avi.0 .0.get(pad.0), Some(&0));
    // The 9-byte frame, its pad byte, then the next chunk's 8-byte header.
    let second_payload = pad.after(Len(1))?.after(Len(8))?;
    ensure_eq!(walked.get(1).or_fail()?.payload, second_payload);
    Ok(())
}

#[test]
fn encode_avi_odd_mjpeg_idx1_matches_walked_chunks() {
    assert_eq!(
        check_encode_avi_odd_mjpeg_idx1_matches_walked_chunks(),
        Ok(())
    );
}

fn check_encode_avi_odd_mjpeg_idx1_matches_walked_chunks() -> Result<(), Fail> {
    let (avi, _) = odd_mjpeg_avi()?;
    let top = riff_chunks(
        &avi,
        Span {
            begin: Offset(12),
            end: avi.end(),
        },
    )?;
    let movi = top.get(1).or_fail()?;
    let movi_tag = movi.payload;
    let walked = riff_chunks(
        &avi,
        Span {
            begin: movi_tag.after(Len(4))?,
            end: movi.payload_end()?,
        },
    )?;
    let idx1 = top.get(2).or_fail()?;
    ensure_eq!(Some(idx1.size.0), walked.len().checked_mul(16));
    let mut entry = idx1.payload;
    for chunk in &walked {
        ensure_eq!(avi.tag_at(entry)?, chunk.tag);
        // Offsets count from the "movi" list type, so the first frame is at 4.
        let from_movi = chunk
            .payload
            .0
            .checked_sub(8)
            .and_then(|header| header.checked_sub(movi_tag.0));
        ensure_eq!(Some(avi.word_at(entry.after(Len(8))?)?.0), from_movi);
        ensure_eq!(avi.word_at(entry.after(Len(12))?)?, chunk.size);
        entry = entry.after(Len(16))?;
    }
    Ok(())
}
