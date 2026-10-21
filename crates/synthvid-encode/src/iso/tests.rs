use super::*;
use synthvid_scene::{FrameRate, Height, Width};
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

#[test]
fn identity_matrix() {
    let matrix = TrackMatrix::identity();
    let expected: [i32; 9] = [
        0x0001_0000, // a
        0,           // b
        0,           // u
        0,           // c
        0x0001_0000, // d
        0,           // v
        0,           // x
        0,           // y
        0x4000_0000, // w
    ];
    assert_eq!(matrix.as_array(), expected);
}

#[test]
fn rotate_90_small_dims() {
    assert_eq!(check_rotate_90_small_dims(), Ok(()));
}

fn check_rotate_90_small_dims() -> Result<(), Fail> {
    let dims = Dimensions::new(Width::new(320).or_fail()?, Height::new(240).or_fail()?);
    let matrix = TrackMatrix::rotate_90(dims).or_fail()?;
    // b = 1.0 and c = -1.0 in 16.16; x = 240 * 65536 = 0x00F0_0000; w = 1.0 in 2.30.
    let expected: [i32; 9] = [
        0,
        0x0001_0000,
        0,
        -0x0001_0000,
        0,
        0,
        0x00F0_0000,
        0,
        0x4000_0000,
    ];
    ensure_eq!(matrix.as_array(), expected);
    Ok(())
}

#[test]
fn rotate_180_small_dims() {
    assert_eq!(check_rotate_180_small_dims(), Ok(()));
}

fn check_rotate_180_small_dims() -> Result<(), Fail> {
    let dims = Dimensions::new(Width::new(320).or_fail()?, Height::new(240).or_fail()?);
    let matrix = TrackMatrix::rotate_180(dims).or_fail()?;
    // a = d = -1.0 in 16.16; x = 320 * 65536 = 0x0140_0000; y = 240 * 65536 = 0x00F0_0000.
    let expected: [i32; 9] = [
        -0x0001_0000,
        0,
        0,
        0,
        -0x0001_0000,
        0,
        0x0140_0000,
        0x00F0_0000,
        0x4000_0000,
    ];
    ensure_eq!(matrix.as_array(), expected);
    Ok(())
}

#[test]
fn rotate_270_small_dims() {
    assert_eq!(check_rotate_270_small_dims(), Ok(()));
}

fn check_rotate_270_small_dims() -> Result<(), Fail> {
    let dims = Dimensions::new(Width::new(320).or_fail()?, Height::new(240).or_fail()?);
    let matrix = TrackMatrix::rotate_270(dims).or_fail()?;
    // b = -1.0 and c = 1.0 in 16.16; y = 320 * 65536 = 0x0140_0000; w = 1.0 in 2.30.
    let expected: [i32; 9] = [
        0,
        -0x0001_0000,
        0,
        0x0001_0000,
        0,
        0,
        0,
        0x0140_0000,
        0x4000_0000,
    ];
    ensure_eq!(matrix.as_array(), expected);
    Ok(())
}

#[test]
fn rotate_90_large_dims_overflow() {
    assert_eq!(check_rotate_90_large_dims_overflow(), Ok(()));
}

fn check_rotate_90_large_dims_overflow() -> Result<(), Fail> {
    let dims = Dimensions::new(
        Width::new(0x8000).or_fail()?,
        Height::new(0x8000).or_fail()?,
    );
    let result = TrackMatrix::rotate_90(dims);
    ensure_eq!(
        result,
        None,
        "32768 pixel tall track should overflow in 16.16"
    );
    Ok(())
}

#[test]
fn rotate_90_frozen_vector() {
    assert_eq!(check_rotate_90_frozen_vector(), Ok(()));
}

fn check_rotate_90_frozen_vector() -> Result<(), Fail> {
    // Hand-computed frozen vector for rotate_90 at 160x120 pixels
    // Expected 36-byte output (9 i32 values, each 4 bytes)
    let dims = Dimensions::new(Width::new(160).or_fail()?, Height::new(120).or_fail()?);
    let matrix = TrackMatrix::rotate_90(dims).or_fail()?;
    let array = matrix.as_array();

    // Manually computed expected values for 160x120:
    // a=0, b=1.0, u=0, c=-1.0, d=0, v=0, x=120*65536=0x00780000, y=0, w=1.0
    let expected_bytes: [u8; 36] = [
        0x00, 0x00, 0x00, 0x00, // a = 0
        0x00, 0x01, 0x00, 0x00, // b = 0x00010000 (1.0 in 16.16)
        0x00, 0x00, 0x00, 0x00, // u = 0
        0xff, 0xff, 0x00, 0x00, // c = -65536 = 0xffff0000 as i32 (-1.0 in 16.16)
        0x00, 0x00, 0x00, 0x00, // d = 0
        0x00, 0x00, 0x00, 0x00, // v = 0
        0x00, 0x78, 0x00, 0x00, // x = 120 * 65536 = 0x00780000
        0x00, 0x00, 0x00, 0x00, // y = 0
        0x40, 0x00, 0x00, 0x00, // w = 0x40000000 (1.0 in 2.30)
    ];

    // Verify via each i32's own total, sign-agnostic big-endian byte form.
    let mut actual_bytes = [0_u8; 36];
    for (slot, value) in actual_bytes.as_chunks_mut::<4>().0.iter_mut().zip(array) {
        slot.copy_from_slice(&value.to_be_bytes());
    }

    ensure_eq!(
        actual_bytes,
        expected_bytes,
        "rotate_90 frozen vector mismatch"
    );
    Ok(())
}

#[test]
fn matrix_byte_identity() {
    let matrix1 = TrackMatrix::identity();
    let matrix2 = TrackMatrix::identity();
    assert_eq!(matrix1, matrix2);
    assert_eq!(matrix1.as_array(), matrix2.as_array());
}

#[test]
fn rotate_90_byte_identity() {
    assert_eq!(check_rotate_90_byte_identity(), Ok(()));
}

fn check_rotate_90_byte_identity() -> Result<(), Fail> {
    let dims = Dimensions::new(Width::new(160).or_fail()?, Height::new(120).or_fail()?);
    let matrix1 = TrackMatrix::rotate_90(dims).or_fail()?;
    let matrix2 = TrackMatrix::rotate_90(dims).or_fail()?;
    ensure_eq!(matrix1, matrix2);
    ensure_eq!(matrix1.as_array(), matrix2.as_array());
    Ok(())
}

/// Four bytes of a box field, as stored.
struct Field([u8; 4]);

/// Where a field sits: this many bytes after the first occurrence of a tag.
struct FieldAt([u8; 4], usize);

/// An encoded file.
struct Encoded(Vec<u8>);

impl Encoded {
    /// Reads the big-endian bytes `offset` bytes after the first `tag`.
    fn field(&self, FieldAt(tag, offset): FieldAt) -> Result<Field, Fail> {
        let at = self
            .0
            .windows(4)
            .position(|window| window == tag.as_slice())
            .or_fail()?;
        let field: Vec<u8> = self
            .0
            .iter()
            .skip(at)
            .skip(offset)
            .take(4)
            .copied()
            .collect();
        field.try_into().map(Field).or_fail()
    }
}

/// The `tkhd` duration is in the movie timescale and equals the `mvhd`
/// duration. At 30000/1001 with 7 frames that is 7 * 1001 = 7007 units
/// of 1/30000 s; the frame count alone (7) would be wrong.
#[test]
fn tkhd_duration_equals_mvhd_duration() {
    assert_eq!(check_tkhd_duration_equals_mvhd_duration(), Ok(()));
}

fn check_tkhd_duration_equals_mvhd_duration() -> Result<(), Fail> {
    use core::num::NonZeroI64;
    use synthvid_scene::{Numerator, Ratio};

    let denom = NonZeroI64::new(1001).or_fail()?;
    let rate = FrameRate::new(Ratio::new(Numerator(30000), denom).or_fail()?).or_fail()?;
    // Each frame is 5x3 pixels of 3 bytes: 45 bytes.
    let frames: Vec<EncodedFrame> = (0..7_u8).map(|fill| EncodedFrame(vec![fill; 45])).collect();
    let bytes = Encoded(
        encode_iso(
            FrameRates {
                playback: rate,
                capture: None,
            },
            &frames,
            TrackParams {
                width: Width::new(5).or_fail()?,
                height: Height::new(3).or_fail()?,
                codec: IsoCodec::UncompressedRgb,
                track_matrix: TrackMatrix::identity(),
            },
        )
        .or_fail()?
        .0,
    );

    // Offsets count from the start of the tag. mvhd: 4 tag bytes,
    // version/flags, creation and modification times (16), then the
    // timescale and the duration (20).
    ensure_eq!(
        u32::from_be_bytes(bytes.field(FieldAt(*b"mvhd", 16))?.0),
        30000
    );
    ensure_eq!(
        u32::from_be_bytes(bytes.field(FieldAt(*b"mvhd", 20))?.0),
        7007
    );
    // tkhd: the same 16, then track ID and a reserved word (24) before
    // the duration.
    ensure_eq!(
        u32::from_be_bytes(bytes.field(FieldAt(*b"tkhd", 24))?.0),
        7007
    );
    Ok(())
}
