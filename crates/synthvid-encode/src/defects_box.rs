//! Internal box/chunk parsing helpers for ISO and AVI formats.

use super::{BoxPath, BoxTag, DefectError};

/// The absolute file offset of a box's own header (where its 4-byte
/// size field begins). Distinguished from `PayloadStart`/`PayloadEnd`
/// so a box's own header position can never be silently used where a
/// payload-relative field offset was needed, or vice versa.
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub(super) struct BoxStart(usize);

impl BoxStart {
    /// Creates a box start offset from a byte position.
    pub(super) const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Gets the box start offset value.
    pub(super) const fn get(self) -> usize {
        self.0
    }

    /// The absolute offset of the box's own 8-byte header (size+tag).
    pub(super) const fn as_absolute(self) -> AbsoluteOffset {
        AbsoluteOffset(self.0)
    }

    /// The absolute offset of an AVI/RIFF `LIST` chunk's (or the RIFF
    /// header's) own declared size field, which lives 4 bytes after
    /// the chunk's own start (past its 4-byte tag/"RIFF" literal),
    /// unlike an ISO box where the size field IS the box's own start.
    pub(super) fn avi_size_field(self) -> Result<AbsoluteOffset, DefectError> {
        self.0
            .checked_add(4)
            .map(AbsoluteOffset)
            .ok_or(DefectError::SizeOverflow)
    }
}

/// The length, in bytes, of a box's own header+payload span (its total
/// on-disk size). Distinguished from an offset type so a length and a
/// position can never be added to the wrong operand or swapped by
/// mistake in the arithmetic that computes byte ranges.
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub(super) struct BoxLen(usize);

impl BoxLen {
    /// Creates a box length from a byte count.
    pub(super) const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Gets the box length value.
    pub(super) const fn get(self) -> usize {
        self.0
    }
}

/// The absolute file offset where a box's PAYLOAD begins (i.e. 8 bytes
/// after `BoxStart`, past the box's own size+tag header).
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub(super) struct PayloadStart(usize);

impl PayloadStart {
    /// Creates a payload start offset.
    pub(super) const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Gets the payload start offset value.
    pub(super) const fn get(self) -> usize {
        self.0
    }

    /// The absolute offset of a field `delta` bytes into this payload.
    pub(super) fn field_at(self, delta: usize) -> Result<AbsoluteOffset, DefectError> {
        self.0
            .checked_add(delta)
            .map(AbsoluteOffset)
            .ok_or(DefectError::SizeOverflow)
    }

    /// Convert to an absolute offset for direct file access.
    pub(super) const fn as_absolute(self) -> AbsoluteOffset {
        AbsoluteOffset(self.0)
    }
}

/// The absolute file offset one past the end of a box's payload.
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub(super) struct PayloadEnd(usize);

impl PayloadEnd {
    /// Creates a payload end offset.
    pub(super) const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Gets the payload end offset value.
    pub(super) const fn get(self) -> usize {
        self.0
    }

    /// Convert to an absolute offset for direct file access.
    pub(super) const fn as_absolute(self) -> AbsoluteOffset {
        AbsoluteOffset(self.0)
    }
}

/// A single absolute file offset meant to be read from or written to
/// directly (e.g. via `extract_4_bytes`/`write_u32`). Reached only
/// through `PayloadStart::field_at`/`BoxStart::as_absolute`/etc, never
/// constructed from an arbitrary arithmetic expression, so a caller
/// cannot accidentally pass a payload-relative delta where an absolute
/// offset was required.
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub(super) struct AbsoluteOffset(usize);

impl AbsoluteOffset {
    /// Creates an absolute offset.
    pub(super) const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Gets the absolute offset value.
    pub(super) const fn get(self) -> usize {
        self.0
    }
}

/// Byte order for writing a multi-byte integer field.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum Endianness {
    /// Big-endian (ISO/MP4 box fields).
    Big,
    /// Little-endian (AVI/RIFF chunk fields).
    Little,
}

/// The raw bytes of a container file being inspected or mutated.
/// Distinguishes "the whole file" from any other byte slice this
/// module happens to be holding (an extracted box's own bytes, a
/// truncated prefix, etc.) -- those remain plain `&[u8]`/`Vec<u8>`
/// since they are not "the container" in the same sense.
#[derive(Debug, Clone, Copy)]
pub(super) struct ContainerBytes<'a>(&'a [u8]);

impl<'a> ContainerBytes<'a> {
    /// Creates a container bytes wrapper.
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }

    /// Gets the byte slice.
    pub(super) const fn as_slice(self) -> &'a [u8] {
        self.0
    }

    /// Gets the length of the container.
    pub(super) const fn len(self) -> usize {
        self.0.len()
    }
}

/// Box header information: box start, payload start, payload end.
pub(super) struct BoxInfo {
    /// Box start byte offset (the position of the box's own header)
    pub(super) box_start: BoxStart,
    /// Payload start byte offset
    pub(super) payload_start: PayloadStart,
    /// Payload end byte offset
    pub(super) payload_end: PayloadEnd,
    /// Box tag
    pub(super) tag: BoxTag,
}

/// Extended box info with ancestor size field positions (ISO format).
pub(super) struct IsoBoxWithAncestors {
    /// The target box
    pub(super) box_info: BoxInfo,
    /// Offsets in the file where each ancestor's size field lives (excluding the RIFF header).
    /// Ordered from root to immediate parent.
    pub(super) ancestor_size_offsets: Vec<AbsoluteOffset>,
}

/// Extended box info with ancestor size field positions (AVI format).
pub(super) struct AviBoxWithAncestors {
    /// The target box
    pub(super) box_info: BoxInfo,
    /// Offsets in the file where each ancestor's size field lives.
    /// First element (if present) is always the RIFF header size offset (4).
    /// Remaining offsets are LIST chunk size field offsets.
    /// Ordered from root (RIFF) down to immediate parent.
    pub(super) ancestor_size_offsets: Vec<AbsoluteOffset>,
}

/// Helper to extract 4 bytes safely from input at offset.
#[must_use]
pub(super) fn extract_4_bytes(
    input: ContainerBytes<'_>,
    offset: AbsoluteOffset,
) -> Option<[u8; 4]> {
    let off = offset.get();
    let buf = input.as_slice();
    buf.get(off..off.checked_add(4)?)
        .and_then(|slice| <&[u8; 4]>::try_from(slice).ok())
        .copied()
}

/// Detects if input is ISO format (big-endian size, tag).
pub(super) fn is_iso_format(input: ContainerBytes<'_>) -> Result<bool, DefectError> {
    if input.len() < 8 {
        return Ok(false);
    }
    let Some(size_bytes) = extract_4_bytes(input, AbsoluteOffset::new(0)) else {
        return Ok(false);
    };
    let size = u32::from_be_bytes(size_bytes);
    let size_usize = usize::try_from(size).map_err(|_| DefectError::InvalidBoxStructure)?;
    Ok(size >= 8 && size_usize <= input.len())
}

/// Detects if input is AVI/RIFF format (little-endian "RIFF" tag).
pub(super) fn is_avi_format(input: ContainerBytes<'_>) -> bool {
    if input.len() < 12 {
        return false;
    }
    let buf = input.as_slice();
    // Check for "RIFF" at offset 0
    if buf.get(0..4) != Some(b"RIFF") {
        return false;
    }
    // Check for "AVI " at offset 8
    buf.get(8..12) == Some(b"AVI ")
}

/// Helper to write a u32 to buffer at given offset, in the specified endianness.
/// The buffer indexing requires a raw usize; offset is converted at this boundary.
pub(super) fn write_u32(
    buffer: &mut [u8],
    offset: AbsoluteOffset,
    value: u32,
    endianness: Endianness,
) -> Result<(), DefectError> {
    // buffer indexing requires a raw usize; this is the single point where
    // the offset re-enters raw-usize space
    let off = offset.get();
    let end = off.checked_add(4).ok_or(DefectError::SizeOverflow)?;
    let bytes = match endianness {
        Endianness::Big => value.to_be_bytes(),
        Endianness::Little => value.to_le_bytes(),
    };
    buffer
        .get_mut(off..end)
        .map(|slice| slice.copy_from_slice(&bytes))
        .ok_or(DefectError::InvalidBoxStructure)
}

/// Parses an ISO box at the given offset, returns `(tag, payload_start, payload_end)`.
pub(super) fn parse_iso_box(
    input: ContainerBytes<'_>,
    offset: AbsoluteOffset,
) -> Result<BoxInfo, DefectError> {
    let off = offset.get();
    let header_end = off.checked_add(8).ok_or(DefectError::SizeOverflow)?;
    if header_end > input.len() {
        return Err(DefectError::InvalidBoxStructure);
    }

    let size_bytes = extract_4_bytes(input, offset).ok_or(DefectError::InvalidBoxStructure)?;
    let size = u32::from_be_bytes(size_bytes);
    let size_usize = usize::try_from(size).map_err(|_| DefectError::InvalidBoxStructure)?;

    if size_usize < 8 {
        return Err(DefectError::InvalidBoxStructure);
    }

    let box_end = off
        .checked_add(size_usize)
        .ok_or(DefectError::SizeOverflow)?;
    if box_end > input.len() {
        return Err(DefectError::InvalidBoxStructure);
    }

    let tag_bytes = extract_4_bytes(
        input,
        AbsoluteOffset::new(off.checked_add(4).ok_or(DefectError::SizeOverflow)?),
    )
    .ok_or(DefectError::SizeOverflow)?;

    Ok(BoxInfo {
        box_start: BoxStart::new(off),
        payload_start: PayloadStart::new(off.checked_add(8).ok_or(DefectError::SizeOverflow)?),
        payload_end: PayloadEnd::new(box_end),
        tag: BoxTag::new(tag_bytes),
    })
}

/// Parses an AVI/RIFF chunk at the given offset, returns `(tag, payload_start, payload_end)`.
/// For LIST chunks, the tag includes the subtype (first 4 bytes of payload).
pub(super) fn parse_avi_chunk(
    input: ContainerBytes<'_>,
    offset: AbsoluteOffset,
) -> Result<BoxInfo, DefectError> {
    let off = offset.get();
    let header_end = off.checked_add(8).ok_or(DefectError::SizeOverflow)?;
    if header_end > input.len() {
        return Err(DefectError::InvalidBoxStructure);
    }

    let tag_bytes = extract_4_bytes(input, offset).ok_or(DefectError::InvalidBoxStructure)?;
    let size_bytes = extract_4_bytes(
        input,
        AbsoluteOffset::new(off.checked_add(4).ok_or(DefectError::SizeOverflow)?),
    )
    .ok_or(DefectError::SizeOverflow)?;
    let size = u32::from_le_bytes(size_bytes);
    let size_usize = usize::try_from(size).map_err(|_| DefectError::InvalidBoxStructure)?;

    if size_usize < 8 {
        return Err(DefectError::InvalidBoxStructure);
    }

    let chunk_end = off
        .checked_add(size_usize.checked_add(8).ok_or(DefectError::SizeOverflow)?)
        .ok_or(DefectError::SizeOverflow)?;
    if chunk_end > input.len() {
        return Err(DefectError::InvalidBoxStructure);
    }

    // For LIST chunks, read the subtype as the tag instead of the literal "LIST" string
    let (actual_tag, payload_start) = if tag_bytes == *b"LIST" {
        // Verify we can read the subtype (4 bytes at offset+8)
        let subtype_offset = off.checked_add(8).ok_or(DefectError::SizeOverflow)?;
        let subtype_bytes = extract_4_bytes(input, AbsoluteOffset::new(subtype_offset))
            .ok_or(DefectError::SizeOverflow)?;
        let payload_start_list = off.checked_add(12).ok_or(DefectError::SizeOverflow)?;
        (
            BoxTag::new(subtype_bytes),
            PayloadStart::new(payload_start_list),
        )
    } else {
        let payload_start_regular = off.checked_add(8).ok_or(DefectError::SizeOverflow)?;
        (
            BoxTag::new(tag_bytes),
            PayloadStart::new(payload_start_regular),
        )
    };

    let payload_end = PayloadEnd::new(chunk_end);

    Ok(BoxInfo {
        box_start: BoxStart::new(off),
        payload_start,
        payload_end,
        tag: actual_tag,
    })
}

/// Finds a box by path in an ISO container, returns its byte range.
pub(super) fn find_iso_box(
    input: ContainerBytes<'_>,
    path: &BoxPath,
) -> Result<BoxInfo, DefectError> {
    let is_iso = is_iso_format(input)?;
    if !is_iso {
        return Err(DefectError::IncompatibleFileType);
    }

    let path_tags = path.tags();
    if path_tags.is_empty() {
        return Ok(BoxInfo {
            box_start: BoxStart::new(0),
            payload_start: PayloadStart::new(0),
            payload_end: PayloadEnd::new(input.len()),
            tag: BoxTag::new(*b"root"),
        });
    }

    // Start with the entire file as the container
    let mut container_start = AbsoluteOffset::new(0);
    let mut container_end = input.len();

    for (depth, target_tag) in path_tags.iter().enumerate() {
        let is_last = depth.checked_add(1) == Some(path_tags.len());

        // Search for target_tag within the current container
        let mut offset = container_start;
        let mut found = false;

        while offset.get() < container_end {
            let box_info = parse_iso_box(input, offset)?;

            if box_info.tag == *target_tag {
                if is_last {
                    return Ok(box_info);
                }
                // Found an intermediate box, descend into it
                container_start = box_info.payload_start.as_absolute();
                container_end = box_info.payload_end.get();
                found = true;
                break;
            }

            offset = box_info.payload_end.as_absolute();
        }

        if !found {
            return Err(DefectError::BoxNotFound(path.clone()));
        }
    }

    Err(DefectError::BoxNotFound(path.clone()))
}

/// Finds a box by path in an AVI/RIFF container, returns its byte range.
pub(super) fn find_avi_box(
    input: ContainerBytes<'_>,
    path: &BoxPath,
) -> Result<BoxInfo, DefectError> {
    let is_avi = is_avi_format(input);
    if !is_avi {
        return Err(DefectError::IncompatibleFileType);
    }

    let path_tags = path.tags();
    if path_tags.is_empty() {
        return Ok(BoxInfo {
            box_start: BoxStart::new(0),          // RIFF header starts at 0
            payload_start: PayloadStart::new(12), // After RIFF header
            payload_end: PayloadEnd::new(input.len()),
            tag: BoxTag::new(*b"root"),
        });
    }

    // Start after RIFF header
    let mut offset = AbsoluteOffset::new(12);

    for (depth, target_tag) in path_tags.iter().enumerate() {
        let is_last = depth.checked_add(1) == Some(path_tags.len());

        let mut found_box: Option<BoxInfo> = None;

        while offset.get() < input.len() {
            if offset
                .get()
                .checked_add(8)
                .ok_or(DefectError::SizeOverflow)?
                > input.len()
            {
                break;
            }

            let chunk = parse_avi_chunk(input, offset)?;

            if is_last && chunk.tag == *target_tag {
                return Ok(chunk);
            }

            if !is_last && chunk.tag == *target_tag {
                // For non-last tags, look inside this chunk
                offset = chunk.payload_start.as_absolute();
                found_box = Some(chunk);
                break;
            }

            // Move to next chunk at this level
            // AVI chunks are padded to 4-byte boundaries
            let chunk_total = chunk
                .payload_end
                .get()
                .checked_sub(chunk.payload_start.get())
                .ok_or(DefectError::SizeOverflow)?;
            // Round up chunk_total to the next even number
            // Round up chunk_total to the next even number. Since chunk_total is from
            // checked_sub on two usize values from within the input buffer, it's always safe.
            let padded_chunk_size = chunk_total
                .checked_add(1)
                .and_then(|x| {
                    // Safe: x is always positive (result of checked_add(1)), so division by 2 is valid
                    let div_result = x >> 1; // Equivalent to x / 2
                    div_result.checked_mul(2)
                })
                .ok_or(DefectError::SizeOverflow)?;
            offset = AbsoluteOffset::new(
                chunk
                    .payload_start
                    .get()
                    .checked_add(padded_chunk_size)
                    .ok_or(DefectError::SizeOverflow)?,
            );
        }

        if !is_last && found_box.is_none() {
            return Err(DefectError::BoxNotFound(path.clone()));
        }
    }

    Err(DefectError::BoxNotFound(path.clone()))
}

/// Finds a box by path in an ISO container and returns its box info plus ancestor size field offsets.
pub(super) fn find_iso_box_with_ancestors(
    input: ContainerBytes<'_>,
    path: &BoxPath,
) -> Result<IsoBoxWithAncestors, DefectError> {
    let is_iso = is_iso_format(input)?;
    if !is_iso {
        return Err(DefectError::IncompatibleFileType);
    }

    let path_tags = path.tags();
    if path_tags.is_empty() {
        return Ok(IsoBoxWithAncestors {
            box_info: BoxInfo {
                box_start: BoxStart::new(0),
                payload_start: PayloadStart::new(0),
                payload_end: PayloadEnd::new(input.len()),
                tag: BoxTag::new(*b"root"),
            },
            ancestor_size_offsets: Vec::new(),
        });
    }

    // Start with the entire file as the container
    let mut container_start = AbsoluteOffset::new(0);
    let mut container_end = input.len();
    let mut ancestor_size_offsets = Vec::new();

    for (depth, target_tag) in path_tags.iter().enumerate() {
        let is_last = depth.checked_add(1) == Some(path_tags.len());

        // Search for target_tag within the current container
        let mut offset = container_start;
        let mut found = false;

        while offset.get() < container_end {
            let box_info = parse_iso_box(input, offset)?;

            if box_info.tag == *target_tag {
                if is_last {
                    return Ok(IsoBoxWithAncestors {
                        box_info,
                        ancestor_size_offsets,
                    });
                }
                // Found an intermediate box, descend into it
                // Record this box's size field offset for later patching
                ancestor_size_offsets.push(offset);
                container_start = box_info.payload_start.as_absolute();
                container_end = box_info.payload_end.get();
                found = true;
                break;
            }

            offset = box_info.payload_end.as_absolute();
        }

        if !found {
            return Err(DefectError::BoxNotFound(path.clone()));
        }
    }

    Err(DefectError::BoxNotFound(path.clone()))
}

/// Finds a box by path in an AVI/RIFF container and returns its box info plus ancestor size field offsets.
pub(super) fn find_avi_box_with_ancestors(
    input: ContainerBytes<'_>,
    path: &BoxPath,
) -> Result<AviBoxWithAncestors, DefectError> {
    let is_avi = is_avi_format(input);
    if !is_avi {
        return Err(DefectError::IncompatibleFileType);
    }

    let path_tags = path.tags();
    if path_tags.is_empty() {
        return Ok(AviBoxWithAncestors {
            box_info: BoxInfo {
                box_start: BoxStart::new(0),          // RIFF header starts at 0
                payload_start: PayloadStart::new(12), // After RIFF header
                payload_end: PayloadEnd::new(input.len()),
                tag: BoxTag::new(*b"root"),
            },
            ancestor_size_offsets: vec![AbsoluteOffset::new(4)], // RIFF header size offset
        });
    }

    // RIFF header size is always at offset 4
    let mut ancestor_size_offsets = vec![AbsoluteOffset::new(4)];

    // Start after RIFF header
    let mut offset = AbsoluteOffset::new(12);

    for (depth, target_tag) in path_tags.iter().enumerate() {
        let is_last = depth.checked_add(1) == Some(path_tags.len());

        let mut found_box: Option<BoxInfo> = None;

        while offset.get() < input.len() {
            if offset
                .get()
                .checked_add(8)
                .ok_or(DefectError::SizeOverflow)?
                > input.len()
            {
                break;
            }

            let chunk = parse_avi_chunk(input, offset)?;

            if is_last && chunk.tag == *target_tag {
                return Ok(AviBoxWithAncestors {
                    box_info: chunk,
                    ancestor_size_offsets,
                });
            }

            if !is_last && chunk.tag == *target_tag {
                // For non-last tags, look inside this chunk (only if it's a LIST chunk)
                let tag_bytes =
                    extract_4_bytes(input, offset).ok_or(DefectError::InvalidBoxStructure)?;
                if tag_bytes == *b"LIST" {
                    // Record this LIST chunk's size field offset (offset + 4)
                    ancestor_size_offsets.push(AbsoluteOffset::new(
                        offset
                            .get()
                            .checked_add(4)
                            .ok_or(DefectError::SizeOverflow)?,
                    ));
                }
                offset = chunk.payload_start.as_absolute();
                found_box = Some(chunk);
                break;
            }

            // Move to next chunk at this level
            // AVI chunks are padded to 4-byte boundaries
            let chunk_total = chunk
                .payload_end
                .get()
                .checked_sub(chunk.payload_start.get())
                .ok_or(DefectError::SizeOverflow)?;
            // Round up chunk_total to the next even number. Since chunk_total is from
            // checked_sub on two usize values from within the input buffer, it's always safe.
            let padded_chunk_size = chunk_total
                .checked_add(1)
                .and_then(|x| {
                    // Safe: x is always positive (result of checked_add(1)), so division by 2 is valid
                    let div_result = x >> 1; // Equivalent to x / 2
                    div_result.checked_mul(2)
                })
                .ok_or(DefectError::SizeOverflow)?;
            offset = AbsoluteOffset::new(
                chunk
                    .payload_start
                    .get()
                    .checked_add(padded_chunk_size)
                    .ok_or(DefectError::SizeOverflow)?,
            );
        }

        if !is_last && found_box.is_none() {
            return Err(DefectError::BoxNotFound(path.clone()));
        }
    }

    Err(DefectError::BoxNotFound(path.clone()))
}
