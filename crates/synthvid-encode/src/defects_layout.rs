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
    extract_4_bytes, is_avi_format, is_iso_format, parse_iso_box, AbsoluteOffset, BoxInfo,
    ContainerBytes, ISO_CONTAINERS,
};
use super::defects_frames::add_iso_frames;
use super::{BoxTag, DefectError};
use crate::avi::{AviCodec, RIFF_CHUNK_ALIGNMENT};
use core::fmt;
use std::collections::BTreeMap;
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
        occurrence: u32,
    },
    /// A frame, by its position among the frames. A frame's region starts at
    /// its data: in AVI the chunk header before it belongs to the region that
    /// precedes it.
    Frame(FrameIndex),
}

impl BoxStep {
    /// Creates a step naming the `occurrence`-th sibling that carries `tag`.
    #[must_use]
    pub const fn new(tag: BoxTag, occurrence: u32) -> Self {
        Self::Tagged { tag, occurrence }
    }

    /// Creates a step naming the first sibling that carries `tag`.
    #[must_use]
    pub const fn first(tag: BoxTag) -> Self {
        Self::new(tag, 0)
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
            Self::Tagged { tag, occurrence } if *occurrence != 0 => {
                write!(f, "/{tag}[{occurrence}]")
            }
            Self::Tagged { tag, .. } => write!(f, "/{tag}"),
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
        for step in core::iter::once(&self.first).chain(&self.rest) {
            write!(f, "{step}")?;
        }
        Ok(())
    }
}

/// A box and the offset of its first byte.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct Region {
    /// Offset of the box's first byte.
    pub(super) start: usize,
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
    /// The file's length in bytes.
    len: usize,
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
        let kind = if is_iso_format(input)? {
            walk_iso(input, 0, input.len(), None, &mut regions)?;
            add_iso_frames(input, &mut regions)?;
            ContainerKind::Iso
        } else if is_avi_format(input) {
            walk_avi_root(input, &mut regions)?;
            ContainerKind::Avi
        } else {
            return Err(DefectError::IncompatibleFileType);
        };
        Ok(Self {
            regions,
            len: input.len(),
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
    pub(super) fn span(&self, address: &BoxAddress) -> Result<(usize, usize), DefectError> {
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
        let end = index
            .checked_add(1)
            .and_then(|next| self.regions.get(next))
            .map_or(self.len, |next| next.start);
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
#[derive(Default)]
struct Occurrences {
    /// Siblings seen so far, by tag.
    tags: BTreeMap<[u8; 4], u32>,
    /// Frames seen so far.
    frames: u32,
}

impl Occurrences {
    /// Returns the step for the next sibling carrying `tag`.
    fn next(&mut self, tag: BoxTag) -> BoxStep {
        let seen = self.tags.entry(tag.get()).or_insert(0);
        let step = BoxStep::new(tag, *seen);
        *seen = seen.saturating_add(1);
        step
    }

    /// Returns the step for the next frame.
    const fn next_frame(&mut self) -> BoxStep {
        let step = BoxStep::frame(FrameIndex::new(self.frames));
        self.frames = self.frames.saturating_add(1);
        step
    }
}

/// Builds the address of a box from its parent's, or as a top-level box.
fn address_of(kind: ContainerKind, parent: Option<&BoxAddress>, step: BoxStep) -> BoxAddress {
    parent.map_or_else(
        || BoxAddress::top(kind, step),
        |parent| parent.clone().child(step),
    )
}

/// Lists the ISO boxes in `begin..end`, descending into containers.
fn walk_iso(
    input: ContainerBytes<'_>,
    begin: usize,
    end: usize,
    parent: Option<&BoxAddress>,
    out: &mut Vec<Region>,
) -> Result<(), DefectError> {
    let mut occurrences = Occurrences::default();
    let mut offset = begin;
    while offset < end {
        let info: BoxInfo = parse_iso_box(input, AbsoluteOffset::new(offset))?;
        let address = address_of(ContainerKind::Iso, parent, occurrences.next(info.tag));
        out.push(Region {
            start: info.box_start.get(),
            address: address.clone(),
        });
        if ISO_CONTAINERS.contains(&info.tag.get()) {
            walk_iso(
                input,
                info.payload_start.get(),
                info.payload_end.get(),
                Some(&address),
                out,
            )?;
        }
        offset = info.payload_end.get();
    }
    Ok(())
}

/// Lists an AVI file's boxes: the `RIFF` chunk, then its children.
fn walk_avi_root(input: ContainerBytes<'_>, out: &mut Vec<Region>) -> Result<(), DefectError> {
    let riff = BoxAddress::top(ContainerKind::Avi, BoxStep::first(BoxTag::new(*b"RIFF")));
    out.push(Region {
        start: 0,
        address: riff.clone(),
    });
    // The RIFF header is 12 bytes: "RIFF", a size, and the "AVI " form type.
    walk_avi(input, 12, input.len(), &riff, AviScope::Elsewhere, out)
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
        children: usize,
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
    start: usize,
    /// The tag; for a `LIST`, its subtype.
    tag: BoxTag,
    /// Whether the chunk holds other chunks.
    kind: ChunkKind,
    /// Offset one past the chunk's last byte, before any pad byte.
    end: usize,
}

/// Reads the AVI chunk header at `offset`.
///
/// A chunk's size counts only its payload, so a chunk may be shorter than its
/// 8-byte header: a 3-byte frame is a 3-byte chunk.
fn read_avi_chunk(input: ContainerBytes<'_>, offset: usize) -> Result<AviChunk, DefectError> {
    let at = |delta: usize| {
        offset
            .checked_add(delta)
            .map(AbsoluteOffset::new)
            .ok_or(DefectError::SizeOverflow)
    };
    let tag = extract_4_bytes(input, at(0)?).ok_or(DefectError::InvalidBoxStructure)?;
    let size = extract_4_bytes(input, at(4)?).ok_or(DefectError::InvalidBoxStructure)?;
    let size = usize::try_from(u32::from_le_bytes(size)).map_err(|_| DefectError::SizeOverflow)?;
    let end = at(8)?
        .get()
        .checked_add(size)
        .ok_or(DefectError::SizeOverflow)?;
    if end > input.len() {
        return Err(DefectError::InvalidBoxStructure);
    }
    if tag == *b"LIST" {
        let subtype = extract_4_bytes(input, at(8)?).ok_or(DefectError::InvalidBoxStructure)?;
        Ok(AviChunk {
            start: offset,
            tag: BoxTag::new(subtype),
            kind: ChunkKind::List {
                children: at(12)?.get(),
            },
            end,
        })
    } else {
        Ok(AviChunk {
            start: offset,
            tag: BoxTag::new(tag),
            kind: ChunkKind::Leaf,
            end,
        })
    }
}

/// Lists the AVI chunks in `begin..end`, descending into `LIST` chunks.
fn walk_avi(
    input: ContainerBytes<'_>,
    begin: usize,
    end: usize,
    parent: &BoxAddress,
    scope: AviScope,
    out: &mut Vec<Region>,
) -> Result<(), DefectError> {
    let mut occurrences = Occurrences::default();
    let mut offset = begin;
    while offset
        .checked_add(8)
        .is_some_and(|header_end| header_end <= end)
    {
        let chunk = read_avi_chunk(input, offset)?;
        // Under `movi`, a stream-0 frame is addressed by its number rather
        // than by the tag its codec gives it.
        // A frame's region begins at its data, as in ISO, so an offset into a
        // frame counts the same bytes in both formats. Its 8-byte chunk header
        // belongs to the region before it.
        let (step, start) = match (scope, AviCodec::of_frame_chunk(chunk.tag.get())) {
            (AviScope::Movi, Ok(_)) => (
                occurrences.next_frame(),
                chunk
                    .start
                    .checked_add(AVI_CHUNK_HEADER_LEN)
                    .ok_or(DefectError::SizeOverflow)?,
            ),
            _ => (occurrences.next(chunk.tag), chunk.start),
        };
        let address = address_of(ContainerKind::Avi, Some(parent), step);
        out.push(Region {
            start,
            address: address.clone(),
        });
        if let ChunkKind::List { children } = chunk.kind {
            let inner = match chunk.tag.get() {
                MOVI => AviScope::Movi,
                _ => AviScope::Elsewhere,
            };
            walk_avi(input, children, chunk.end, &address, inner, out)?;
        }
        // An odd-length chunk is followed by one pad byte, which belongs to
        // the chunk it follows.
        offset = chunk
            .end
            .checked_next_multiple_of(RIFF_CHUNK_ALIGNMENT)
            .ok_or(DefectError::SizeOverflow)?;
    }
    Ok(())
}
