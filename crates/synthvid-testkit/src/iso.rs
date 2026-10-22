//! Reads an ISO base media file as a tree of boxes.
//!
//! Every box starts with a 4-byte big-endian size, which counts the header, and
//! a 4-byte tag. A box is found by following those headers from the start of
//! its parent's payload, never by searching the bytes for a tag. Payload bytes
//! are skipped by size, so a payload that happens to contain a tag's letters
//! is not mistaken for a box.

use crate::{Fail, OrFail as _};

/// The bytes of a file, or of a scope within one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsoBytes<'bytes>(pub &'bytes [u8]);

/// A four-byte box tag, as written in a box header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxTag(pub [u8; 4]);

/// A path of box tags, each looked for among the children of the box before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxPath<'bytes>(pub &'bytes [BoxTag]);

/// A whole box: its header, then its payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxView<'bytes>(pub IsoBytes<'bytes>);

/// A position, in bytes, from the start of an [`IsoBytes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteOffset(pub usize);

/// A big-endian `u32` read from a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BigEndianWord(pub u32);

/// A child box: its tag, and the whole box with its header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Child<'bytes> {
    /// The tag written in the box header.
    pub tag: BoxTag,
    /// The whole box, header included.
    pub boxed: BoxView<'bytes>,
}

/// The whole boxes that make up `scope`, each with its tag, header included.
///
/// # Errors
///
/// Returns a [`Fail`] if the boxes do not tile `scope` exactly.
pub fn sibling_boxes(scope: IsoBytes<'_>) -> Result<Vec<Child<'_>>, Fail> {
    let mut boxes = Vec::new();
    let mut rest = scope.0;
    while !rest.is_empty() {
        let size_word: [u8; 4] = rest.get(..4).or_fail()?.try_into().or_fail()?;
        let size = usize::try_from(u32::from_be_bytes(size_word)).or_fail()?;
        if size < 8 {
            return Err(Fail::here(format_args!(
                "box size {size} is smaller than its header"
            )));
        }
        let tag: [u8; 4] = rest.get(4..8).or_fail()?.try_into().or_fail()?;
        let whole = rest.get(..size).or_fail()?;
        boxes.push(Child {
            tag: BoxTag(tag),
            boxed: BoxView(IsoBytes(whole)),
        });
        rest = rest.get(size..).or_fail()?;
    }
    Ok(boxes)
}

/// The tags of the whole boxes in `scope`, in file order. A scope whose boxes
/// do not tile it fails, so an absence check cannot pass on a malformed scope.
///
/// # Errors
///
/// Returns a [`Fail`] when the boxes in `scope` do not tile it.
pub fn tags_in(scope: IsoBytes<'_>) -> Result<Vec<BoxTag>, Fail> {
    Ok(sibling_boxes(scope)?
        .iter()
        .map(|child| child.tag)
        .collect())
}

/// The first child of `scope` tagged `tag`, header included.
///
/// # Errors
///
/// Returns a [`Fail`] when no child is tagged `tag`, or when the boxes in
/// `scope` do not tile it.
fn named_child(scope: IsoBytes<'_>, tag: BoxTag) -> Result<BoxView<'_>, Fail> {
    sibling_boxes(scope)?
        .into_iter()
        .find(|child| child.tag == tag)
        .map(|child| child.boxed)
        .or_fail()
}

/// The box reached from the start of `file` by descending through each tag of
/// `path` in turn, header included. Each tag is looked for only among the
/// children of the box before it.
///
/// # Errors
///
/// Returns a [`Fail`] when a tag of `path` is not found among the children of
/// the box before it, or when a box along the way is malformed.
pub fn child_path<'bytes>(
    file: IsoBytes<'bytes>,
    path: BoxPath<'_>,
) -> Result<BoxView<'bytes>, Fail> {
    let mut scope = file;
    let mut found = BoxView(file);
    for tag in path.0 {
        found = named_child(scope, *tag)?;
        scope = payload(found)?;
    }
    Ok(found)
}

/// The bytes of a box after its 8-byte header.
///
/// # Errors
///
/// Returns a [`Fail`] when `boxed` is shorter than a header.
pub fn payload(boxed: BoxView<'_>) -> Result<IsoBytes<'_>, Fail> {
    Ok(IsoBytes(boxed.0 .0.get(8..).or_fail()?))
}

/// The big-endian `u32` at `offset` bytes into `bytes`.
///
/// # Errors
///
/// Returns a [`Fail`] when fewer than four bytes lie at `offset`.
pub fn u32_at(bytes: IsoBytes<'_>, offset: ByteOffset) -> Result<BigEndianWord, Fail> {
    let word: [u8; 4] = bytes
        .0
        .get(offset.0..)
        .or_fail()?
        .get(..4)
        .or_fail()?
        .try_into()
        .or_fail()?;
    Ok(BigEndianWord(u32::from_be_bytes(word)))
}

#[cfg(test)]
mod tests;
