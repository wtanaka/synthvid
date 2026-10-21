//! Addresses every byte of a container exactly once.
//!
//! A container is a tree of boxes (ISO) or chunks (AVI). Walking it in
//! document order lists every box's first byte. Each byte of the file then
//! belongs to the box that most recently started at or before it, so the boxes
//! partition the file: no byte has two owners and none has none.
//!
//! A truncation names a cut by its owner and an offset into the owner's
//! region, so every cut position has exactly one spelling. A frame is the
//! exception to "a box starts at its header": its region starts at its data,
//! in both formats, so a cut at a given offset within a frame's data removes
//! the same frame bytes whichever format holds it. In AVI the region runs on
//! past the data, through the pad byte and the next chunk's header.

use super::defects_box::{
    detect_format, extract_4_bytes, parse_iso_box, AbsoluteOffset, BoxInfo, ContainerBytes,
    ISO_CONTAINERS,
};
use super::defects_frames::add_iso_frames;
use super::defects_types::{ContainerFormat, FieldOffset, Occurrence};
use super::{BoxTag, DefectError};
use crate::avi::{AviCodec, RIFF_CHUNK_ALIGNMENT};
use crate::avi_helpers::ChunkId;
use core::fmt;
use core::iter::once;
use core::ops::Range;
extern crate alloc;
use alloc::collections::BTreeMap;
use synthvid_scene::FrameIndex;

/// The container format an address is written for.
///
/// ISO and AVI lay their boxes out differently, so an address is only
/// meaningful in the format it names.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ContainerKind {
    /// ISO base media format (MP4).
    Iso,
    /// AVI, a RIFF container. The `RIFF` chunk is the root box.
    Avi,
}

/// One step of an address: either a tagged box among its siblings, or a frame.
///
/// A frame is named by its number, not by the chunk tag its codec happens to
/// give it, so an address for a frame holds for every codec.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BoxStep {
    /// The box with this tag, and which of the siblings sharing it. The
    /// occurrence counts same-tag siblings under one parent, from zero, so two
    /// boxes with the same tag never share an address.
    Tagged {
        /// The box's tag.
        tag: BoxTag,
        /// Zero-based index among the siblings that carry the same tag.
        occurrence: Occurrence,
    },
    /// A frame, by its position among the frames. A frame's region starts at
    /// its data: in AVI the chunk header before it belongs to the region that
    /// precedes it.
    Frame(FrameIndex),
}

impl BoxStep {
    /// Creates a step naming the `occurrence`-th sibling that carries `tag`.
    #[must_use]
    pub const fn new(tag: BoxTag, occurrence: Occurrence) -> Self {
        Self::Tagged { tag, occurrence }
    }

    /// Creates a step naming the first sibling that carries `tag`.
    #[must_use]
    pub const fn first(tag: BoxTag) -> Self {
        Self::new(tag, Occurrence(0))
    }

    /// Creates a step naming a frame.
    #[must_use]
    pub const fn frame(index: FrameIndex) -> Self {
        Self::Frame(index)
    }
}

impl fmt::Display for BoxStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tagged { tag, occurrence } if occurrence.0 != 0 => {
                write!(f, "/{tag}[{}]", occurrence.0)
            }
            Self::Tagged {
                tag,
                occurrence: _occurrence,
            } => write!(f, "/{tag}"),
            Self::Frame(index) => write!(f, "/frame[{}]", index.get()),
        }
    }
}

/// The address of one box in a container: a format and a non-empty path of
/// steps from the root.
///
/// For ISO the path starts at a top-level box, such as `moov`. For AVI the
/// root is the `RIFF` chunk, so every path starts with it.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BoxAddress {
    /// The format the address is written for.
    kind: ContainerKind,
    /// The first step, from the root.
    first: BoxStep,
    /// The steps after the first.
    rest: Vec<BoxStep>,
}

impl BoxAddress {
    /// Creates the address of a top-level box.
    #[must_use]
    pub const fn top(kind: ContainerKind, first: BoxStep) -> Self {
        Self {
            kind,
            first,
            rest: Vec::new(),
        }
    }

    /// Returns the address of a child of this box.
    #[must_use]
    pub fn child(mut self, step: BoxStep) -> Self {
        self.rest.push(step);
        self
    }

    /// The format this address is written for.
    #[must_use]
    pub const fn kind(&self) -> ContainerKind {
        self.kind
    }
}

impl fmt::Display for BoxAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for step in once(&self.first).chain(&self.rest) {
            write!(f, "{step}")?;
        }
        Ok(())
    }
}

/// A box and the offset of its first byte.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct Region {
    /// Offset of the box's first byte.
    pub(super) start: AbsoluteOffset,
    /// The box's address.
    pub(super) address: BoxAddress,
}

/// A container's boxes in document order, with the file's length.
///
/// Each box owns the bytes from its start up to the next box's start, and the
/// last owns the rest of the file.
pub(super) struct Layout {
    /// The boxes in document order.
    regions: Vec<Region>,
    /// The offset one past the file's last byte, which is its length.
    len: AbsoluteOffset,
    /// The format the file is in.
    kind: ContainerKind,
}

impl Layout {
    /// Lists the boxes of an ISO or AVI container.
    ///
    /// # Errors
    ///
    /// Returns an error when the bytes are neither format, or a box header is
    /// malformed.
    pub(super) fn of(input: ContainerBytes<'_>) -> Result<Self, DefectError> {
        let mut regions = Vec::new();
        let kind = match detect_format(input)? {
            ContainerFormat::Iso => {
                let whole = AbsoluteOffset::new(0)..AbsoluteOffset::new(input.as_slice().len());
                walk_iso(input, whole, None, &mut regions)?;
                add_iso_frames(input, &mut regions)?;
                ContainerKind::Iso
            }
            ContainerFormat::Avi => {
                walk_avi_root(input, &mut regions)?;
                ContainerKind::Avi
            }
            ContainerFormat::Neither => return Err(DefectError::IncompatibleFileType),
        };
        Ok(Self {
            regions,
            len: AbsoluteOffset::new(input.as_slice().len()),
            kind,
        })
    }

    /// Finds the byte range a box owns: its start and the offset where its
    /// ownership ends.
    ///
    /// # Errors
    ///
    /// Returns an error when the address is for the other format, or names a
    /// box the file does not contain.
    pub(super) fn span(
        &self,
        address: &BoxAddress,
    ) -> Result<(AbsoluteOffset, AbsoluteOffset), DefectError> {
        if address.kind() != self.kind {
            return Err(DefectError::IncompatibleFileType);
        }
        let index = self
            .regions
            .iter()
            .position(|region| region.address == *address)
            .ok_or(DefectError::AddressNotFound)?;
        let start = self
            .regions
            .get(index)
            .ok_or(DefectError::AddressNotFound)?
            .start;
        let end = 'end: {
            let Some(next) = index.checked_add(1).and_then(|next| self.regions.get(next)) else {
                break 'end self.len;
            };
            next.start
        };
        Ok((start, end))
    }

    /// The boxes in document order.
    #[cfg(test)]
    pub(super) fn regions(&self) -> &[Region] {
        &self.regions
    }
}

/// Counts same-tag siblings so each gets its own occurrence, and frames so
/// each gets its own index.
struct Occurrences {
    /// Siblings seen so far, by tag.
    tags: BTreeMap<BoxTag, Occurrence>,
    /// The index the next frame will take.
    frames: FrameIndex,
}

impl Occurrences {
    /// Returns the step for the next sibling carrying `tag`.
    fn next(&mut self, tag: BoxTag) -> Result<BoxStep, DefectError> {
        let seen = self.tags.entry(tag).or_insert(Occurrence(0));
        let step = BoxStep::new(tag, *seen);
        seen.0 = seen.0.checked_add(1).ok_or(DefectError::SizeOverflow)?;
        Ok(step)
    }

    /// Returns the step for the next frame.
    fn next_frame(&mut self) -> Result<BoxStep, DefectError> {
        let step = BoxStep::frame(self.frames);
        self.frames = FrameIndex::new(
            self.frames
                .get()
                .checked_add(1)
                .ok_or(DefectError::SizeOverflow)?,
        );
        Ok(step)
    }
}

/// Builds the address of a box from its parent's, or as a top-level box.
fn address_of(kind: ContainerKind, parent: Option<&BoxAddress>, step: BoxStep) -> BoxAddress {
    let Some(parent_address) = parent else {
        return BoxAddress::top(kind, step);
    };
    parent_address.clone().child(step)
}

/// Lists the ISO boxes in `begin..end`, descending into containers.
fn walk_iso(
    input: ContainerBytes<'_>,
    span: Range<AbsoluteOffset>,
    parent: Option<&BoxAddress>,
    out: &mut Vec<Region>,
) -> Result<(), DefectError> {
    let mut occurrences = Occurrences {
        tags: BTreeMap::new(),
        frames: FrameIndex::new(0),
    };
    let mut offset = span.start;
    while offset < span.end {
        let info: BoxInfo = parse_iso_box(input, offset)?;
        let address = address_of(ContainerKind::Iso, parent, occurrences.next(info.tag)?);
        out.push(Region {
            start: info.box_start.as_absolute(),
            address: address.clone(),
        });
        if ISO_CONTAINERS.contains(&info.tag.get()) {
            let children = info.payload_start.as_absolute()..info.payload_end.as_absolute();
            walk_iso(input, children, Some(&address), out)?;
        }
        offset = info.payload_end.as_absolute();
    }
    Ok(())
}

/// Lists an AVI file's boxes: the `RIFF` chunk, then its children.
fn walk_avi_root(input: ContainerBytes<'_>, out: &mut Vec<Region>) -> Result<(), DefectError> {
    let riff = BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")));
    out.push(Region {
        start: AbsoluteOffset::new(0),
        address: riff.clone(),
    });
    // The RIFF header is 12 bytes: "RIFF", a size, and the "AVI " form type.
    let children = AbsoluteOffset::new(12)..AbsoluteOffset::new(input.as_slice().len());
    walk_avi(input, children, &riff, AviScope::Elsewhere, out)
}

/// The length of an AVI chunk header: a tag and a size.
const AVI_CHUNK_HEADER_LEN: usize = 8;

/// The list type of the `movi` list, which holds the frames.
const MOVI: [u8; 4] = *b"movi";

/// What an AVI chunk holds.
enum ChunkKind {
    /// Payload bytes only.
    Leaf,
    /// A `LIST`: other chunks, from this offset.
    List {
        /// Offset of the first byte of the chunk's children.
        children: AbsoluteOffset,
    },
}

/// Where in the file a walk of AVI chunks is, as far as addressing a frame
/// needs to know.
#[derive(Clone, Copy)]
enum AviScope {
    /// Directly inside the `movi` list, whose stream-0 chunks are frames.
    Movi,
    /// Anywhere else.
    Elsewhere,
}

/// An AVI chunk's extent and tag.
struct AviChunk {
    /// Offset of the chunk's first byte.
    start: AbsoluteOffset,
    /// The tag; for a `LIST`, its subtype.
    tag: BoxTag,
    /// Whether the chunk holds other chunks.
    kind: ChunkKind,
    /// Offset one past the chunk's last byte, before any pad byte.
    end: AbsoluteOffset,
}

/// Reads the AVI chunk header at `offset`.
///
/// A chunk's size counts only its payload, so a chunk may be shorter than its
/// 8-byte header: a 3-byte frame is a 3-byte chunk.
fn read_avi_chunk(
    input: ContainerBytes<'_>,
    offset: AbsoluteOffset,
) -> Result<AviChunk, DefectError> {
    let at = |delta: FieldOffset| {
        offset
            .get()
            .checked_add(delta.0)
            .map(AbsoluteOffset::new)
            .ok_or(DefectError::SizeOverflow)
    };
    let tag =
        extract_4_bytes(input, at(FieldOffset(0))?).ok_or(DefectError::InvalidBoxStructure)?;
    let size_field =
        extract_4_bytes(input, at(FieldOffset(4))?).ok_or(DefectError::InvalidBoxStructure)?;
    let size =
        usize::try_from(u32::from_le_bytes(size_field.0)).map_err(DefectError::size_overflow)?;
    let end = AbsoluteOffset::new(
        at(FieldOffset(8))?
            .get()
            .checked_add(size)
            .ok_or(DefectError::SizeOverflow)?,
    );
    if end.get() > input.as_slice().len() {
        return Err(DefectError::InvalidBoxStructure);
    }
    if tag.0 == *b"LIST" {
        let subtype =
            extract_4_bytes(input, at(FieldOffset(8))?).ok_or(DefectError::InvalidBoxStructure)?;
        Ok(AviChunk {
            start: offset,
            tag: BoxTag::new(subtype.0),
            kind: ChunkKind::List {
                children: at(FieldOffset(12))?,
            },
            end,
        })
    } else {
        Ok(AviChunk {
            start: offset,
            tag: BoxTag::new(tag.0),
            kind: ChunkKind::Leaf,
            end,
        })
    }
}

/// Lists the AVI chunks in `begin..end`, descending into `LIST` chunks.
fn walk_avi(
    input: ContainerBytes<'_>,
    span: Range<AbsoluteOffset>,
    parent: &BoxAddress,
    scope: AviScope,
    out: &mut Vec<Region>,
) -> Result<(), DefectError> {
    let mut occurrences = Occurrences {
        tags: BTreeMap::new(),
        frames: FrameIndex::new(0),
    };
    let mut offset = span.start;
    while offset
        .get()
        .checked_add(8)
        .is_some_and(|header_end| header_end <= span.end.get())
    {
        let chunk = read_avi_chunk(input, offset)?;
        // Under `movi`, a stream-0 frame is addressed by its number rather
        // than by the tag its codec gives it.
        // A frame's region begins at its data, as in ISO, so an offset into a
        // frame counts the same bytes in both formats. Its 8-byte chunk header
        // belongs to the region before it.
        let (step, start) = match (
            scope,
            AviCodec::of_frame_chunk(ChunkId::new(chunk.tag.get())),
        ) {
            (AviScope::Movi, Ok(_)) => (
                occurrences.next_frame()?,
                AbsoluteOffset::new(
                    chunk
                        .start
                        .get()
                        .checked_add(AVI_CHUNK_HEADER_LEN)
                        .ok_or(DefectError::SizeOverflow)?,
                ),
            ),
            (AviScope::Movi, Err(_)) | (AviScope::Elsewhere, Ok(_) | Err(_)) => {
                (occurrences.next(chunk.tag)?, chunk.start)
            }
        };
        let address = address_of(ContainerKind::Avi, Some(parent), step);
        out.push(Region {
            start,
            address: address.clone(),
        });
        match chunk.kind {
            ChunkKind::List { children } => {
                let inner = match chunk.tag.get() {
                    MOVI => AviScope::Movi,
                    _ => AviScope::Elsewhere,
                };
                walk_avi(input, children..chunk.end, &address, inner, out)?;
            }
            ChunkKind::Leaf => {}
        }
        // An odd-length chunk is followed by one pad byte, which belongs to
        // the chunk it follows.
        offset = AbsoluteOffset::new(
            chunk
                .end
                .get()
                .checked_next_multiple_of(RIFF_CHUNK_ALIGNMENT)
                .ok_or(DefectError::SizeOverflow)?,
        );
    }
    Ok(())
}
