//! Tests of the box walker, on byte strings built by hand.

use super::{
    child_path, payload, sibling_boxes, tags_in, u32_at, BigEndianWord, BoxPath, BoxTag,
    ByteOffset, IsoBytes,
};
use crate::{ensure, ensure_eq, Fail, OrFail as _};

/// A complete box, header included, as built by hand.
struct Boxed(Vec<u8>);

impl Boxed {
    /// The box as a file or scope, for the walker to read.
    fn iso(&self) -> IsoBytes<'_> {
        IsoBytes(&self.0)
    }
}

/// A box with the given tag and payload, its size word counting the header.
fn boxed(tag: BoxTag, body: IsoBytes<'_>) -> Result<Boxed, Fail> {
    let size = u32::try_from(body.0.len().checked_add(8).or_fail()?).or_fail()?;
    let mut out = size.to_be_bytes().to_vec();
    out.extend_from_slice(&tag.0);
    out.extend_from_slice(body.0);
    Ok(Boxed(out))
}

fn check_lists_top_level_boxes_in_file_order() -> Result<(), Fail> {
    let mut file = boxed(BoxTag(*b"ftyp"), IsoBytes(b"isom"))?.0;
    file.extend(boxed(BoxTag(*b"moov"), IsoBytes(&[]))?.0);
    file.extend(boxed(BoxTag(*b"mdat"), IsoBytes(b"xyz"))?.0);
    ensure_eq!(
        tags_in(IsoBytes(&file))?,
        vec![BoxTag(*b"ftyp"), BoxTag(*b"moov"), BoxTag(*b"mdat")]
    );
    Ok(())
}

#[test]
fn lists_top_level_boxes_in_file_order() {
    assert_eq!(check_lists_top_level_boxes_in_file_order(), Ok(()));
}

fn check_payload_containing_a_tag_is_not_a_box() -> Result<(), Fail> {
    // The moov payload holds the letters "mdat" and "moov"; neither is a box.
    let file = boxed(BoxTag(*b"moov"), IsoBytes(b"mdat moov"))?;
    ensure_eq!(tags_in(file.iso())?, vec![BoxTag(*b"moov")]);
    Ok(())
}

#[test]
fn payload_containing_a_tag_is_not_a_box() {
    assert_eq!(check_payload_containing_a_tag_is_not_a_box(), Ok(()));
}

fn check_refuses_boxes_that_do_not_tile_the_scope() -> Result<(), Fail> {
    // A size word of 20 claims more bytes than the 8 present.
    let truncated: [u8; 8] = [0, 0, 0, 20, b'm', b'o', b'o', b'v'];
    ensure!(sibling_boxes(IsoBytes(&truncated)).is_err());
    // A size word below the 8-byte header is malformed.
    let short: [u8; 8] = [0, 0, 0, 4, b'm', b'o', b'o', b'v'];
    ensure!(sibling_boxes(IsoBytes(&short)).is_err());
    // Trailing bytes too short for a header do not tile the scope either.
    let mut trailing = boxed(BoxTag(*b"free"), IsoBytes(&[]))?.0;
    trailing.push(0);
    ensure!(sibling_boxes(IsoBytes(&trailing)).is_err());
    ensure!(tags_in(IsoBytes(&trailing)).is_err());
    Ok(())
}

#[test]
fn refuses_boxes_that_do_not_tile_the_scope() {
    assert_eq!(check_refuses_boxes_that_do_not_tile_the_scope(), Ok(()));
}

/// An `mdhd` box: version and flags, creation and modification times, then a
/// timescale of 30000 (0x7530) at payload offset 12, which is box offset 20.
fn mdhd_box() -> Result<Boxed, Fail> {
    boxed(
        BoxTag(*b"mdhd"),
        IsoBytes(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x75, 0x30]),
    )
}

fn check_child_path_descends_through_each_tag() -> Result<(), Fail> {
    let mdhd = mdhd_box()?;
    let mdia = boxed(BoxTag(*b"mdia"), mdhd.iso())?;
    let trak = boxed(BoxTag(*b"trak"), mdia.iso())?;
    let moov = boxed(BoxTag(*b"moov"), trak.iso())?;
    let mut file = boxed(BoxTag(*b"ftyp"), IsoBytes(b"isom"))?.0;
    file.extend(moov.0);
    let found = child_path(
        IsoBytes(&file),
        BoxPath(&[
            BoxTag(*b"moov"),
            BoxTag(*b"trak"),
            BoxTag(*b"mdia"),
            BoxTag(*b"mdhd"),
        ]),
    )?;
    ensure_eq!(found.0 .0.len(), 24);
    ensure_eq!(u32_at(found.0, ByteOffset(20))?, BigEndianWord(30_000));
    ensure_eq!(payload(found)?.0.len(), 16);
    Ok(())
}

#[test]
fn child_path_descends_through_each_tag() {
    assert_eq!(check_child_path_descends_through_each_tag(), Ok(()));
}

fn check_absent_child_fails_the_path() -> Result<(), Fail> {
    let trak = boxed(BoxTag(*b"trak"), IsoBytes(&[]))?;
    let file = boxed(BoxTag(*b"moov"), trak.iso())?;
    ensure!(!tags_in(file.iso())?.contains(&BoxTag(*b"udta")));
    ensure!(child_path(file.iso(), BoxPath(&[BoxTag(*b"moov"), BoxTag(*b"udta")])).is_err());
    Ok(())
}

#[test]
fn absent_child_fails_the_path() {
    assert_eq!(check_absent_child_fails_the_path(), Ok(()));
}

fn check_u32_at_reads_big_endian_and_refuses_short_input() -> Result<(), Fail> {
    let bytes: [u8; 6] = [0, 0, 0, 0, 0x01, 0x02];
    ensure_eq!(
        u32_at(IsoBytes(&bytes), ByteOffset(0))?,
        BigEndianWord(0x0000_0000)
    );
    ensure_eq!(
        u32_at(IsoBytes(&[0, 0, 0x01, 0x02]), ByteOffset(0))?,
        BigEndianWord(0x0000_0102)
    );
    ensure!(u32_at(IsoBytes(&bytes), ByteOffset(4)).is_err());
    ensure!(u32_at(IsoBytes(&bytes), ByteOffset(9)).is_err());
    Ok(())
}

#[test]
fn u32_at_reads_big_endian_and_refuses_short_input() {
    assert_eq!(
        check_u32_at_reads_big_endian_and_refuses_short_input(),
        Ok(())
    );
}
