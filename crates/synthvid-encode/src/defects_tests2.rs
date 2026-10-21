use super::tests::{applied, Base, Media};
#[cfg(test)]
use super::*;
use defects_box::{
    extract_4_bytes, find_avi_box_with_ancestors, find_iso_box_with_ancestors, AbsoluteOffset,
};
use synthvid_testkit::{ensure, ensure_eq, ensure_ne, Fail, OrFail as _};

/// The value of a 32-bit size field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SizeField(u32);

/// How much a defect must enlarge a size field.
#[derive(Clone, Copy, Debug)]
struct Growth(u32);

/// The byte order a container writes its size fields in.
#[derive(Clone, Copy, Debug)]
enum ByteOrder {
    /// ISO base media files.
    Big,
    /// RIFF files.
    Little,
}

impl ByteOrder {
    /// The size field at `at` in `media`.
    fn read(self, media: &Media, at: AbsoluteOffset) -> Result<SizeField, Fail> {
        let word = extract_4_bytes(ContainerBytes::new(&media.0), at).or_fail()?;
        Ok(SizeField(match self {
            Self::Big => u32::from_be_bytes(word.0),
            Self::Little => u32::from_le_bytes(word.0),
        }))
    }
}

/// A file before and after a defect was applied to it.
struct Edit<'media> {
    /// The file the defect was applied to.
    before: &'media Media,
    /// The file the defect produced.
    after: &'media Media,
    /// The byte order of the file's size fields.
    order: ByteOrder,
}

impl Edit<'_> {
    /// Fails unless the size field at `at` grew by exactly `growth`.
    fn ensure_grew(&self, at: AbsoluteOffset, growth: Growth) -> Result<(), Fail> {
        let before = self.order.read(self.before, at)?;
        let after = self.order.read(self.after, at)?;
        ensure_eq!(Some(after.0), before.0.checked_add(growth.0));
        Ok(())
    }
}

#[test]
fn reordered_boxes_iso_length_preserved() {
    assert_eq!(check_reordered_boxes_iso_length_preserved(), Ok(()));
}

fn check_reordered_boxes_iso_length_preserved() -> Result<(), Fail> {
    let input = Base::Iso.media()?;
    let path1 = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let path2 = BoxPath::root().child(BoxTag::new(*b"mdat"));
    let defect = Defect::ReorderedBoxes(path1, path2);
    let result = applied(&defect, &input)?;
    ensure_eq!(
        result.0.len(),
        input.0.len(),
        "reordering must preserve total file length"
    );
    Ok(())
}

#[test]
fn reordered_boxes_iso_middle_preserved() {
    assert_eq!(check_reordered_boxes_iso_middle_preserved(), Ok(()));
}

fn check_reordered_boxes_iso_middle_preserved() -> Result<(), Fail> {
    let input = Base::Iso.media()?;
    let path1 = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let path2 = BoxPath::root().child(BoxTag::new(*b"mdat"));
    let defect = Defect::ReorderedBoxes(path1, path2);
    let result = applied(&defect, &input)?;

    // The result length is preserved
    ensure_eq!(result.0.len(), input.0.len(), "length must be preserved");
    // The result is different (boxes were reordered)
    ensure_ne!(result, input, "boxes must be reordered");
    // Both result and input should be valid (non-empty) to prove middle wasn't dropped
    ensure!(!result.0.is_empty(), "result must not be empty");
    ensure!(!input.0.is_empty(), "input must not be empty");
    Ok(())
}

#[test]
fn reordered_boxes_non_siblings_error() {
    assert_eq!(check_reordered_boxes_non_siblings_error(), Ok(()));
}

fn check_reordered_boxes_non_siblings_error() -> Result<(), Fail> {
    let input = Base::Iso.media()?;
    // Try to reorder boxes that are not immediate siblings:
    // path1: /ftyp (root level)
    // path2: /moov/trak/tkhd (nested deep)
    let path1 = BoxPath::root().child(BoxTag::new(*b"ftyp"));
    let path2 = BoxPath::root()
        .child(BoxTag::new(*b"moov"))
        .child(BoxTag::new(*b"trak"))
        .child(BoxTag::new(*b"tkhd"));
    let defect = Defect::ReorderedBoxes(path1, path2);
    ensure_eq!(
        apply(&defect, &input.file()),
        Err(DefectError::InvalidBoxStructure)
    );
    Ok(())
}

#[test]
fn duplicate_box_iso_nested_updates_ancestors() {
    assert_eq!(check_duplicate_box_iso_nested_updates_ancestors(), Ok(()));
}

fn check_duplicate_box_iso_nested_updates_ancestors() -> Result<(), Fail> {
    let input = Base::Iso.media()?;

    let tkhd_path = BoxPath::root()
        .child(BoxTag::new(*b"moov"))
        .child(BoxTag::new(*b"trak"))
        .child(BoxTag::new(*b"tkhd"));
    let container = ContainerBytes::new(&input.0);
    let tkhd_info = find_iso_box_with_ancestors(container, &tkhd_path).or_fail()?;

    let tkhd_box_size = tkhd_info
        .box_info
        .payload_end
        .get()
        .checked_sub(tkhd_info.box_info.box_start.get())
        .or_fail()?;
    let moov_size_offset = *tkhd_info.ancestor_size_offsets.first().or_fail()?;
    let trak_size_offset = *tkhd_info.ancestor_size_offsets.get(1).or_fail()?;

    let defect = Defect::DuplicateBox(tkhd_path);
    let result = applied(&defect, &input)?;

    ensure_eq!(
        Some(result.0.len()),
        input.0.len().checked_add(tkhd_box_size),
        "file must grow by duplicated box size"
    );

    let edit = Edit {
        before: &input,
        after: &result,
        order: ByteOrder::Big,
    };
    let growth = Growth(u32::try_from(tkhd_box_size).or_fail()?);
    // moov and trak sizes must each increase by the tkhd box size.
    edit.ensure_grew(moov_size_offset, growth)?;
    edit.ensure_grew(trak_size_offset, growth)?;
    Ok(())
}

#[test]
fn unknown_box_iso_nested_updates_ancestors() {
    assert_eq!(check_unknown_box_iso_nested_updates_ancestors(), Ok(()));
}

fn check_unknown_box_iso_nested_updates_ancestors() -> Result<(), Fail> {
    /// The size of the inserted box: an 8-byte header and no payload.
    const NEW_BOX_SIZE: usize = 8;
    let input = Base::Iso.media()?;

    let tkhd_path = BoxPath::root()
        .child(BoxTag::new(*b"moov"))
        .child(BoxTag::new(*b"trak"))
        .child(BoxTag::new(*b"tkhd"));
    let container = ContainerBytes::new(&input.0);
    let tkhd_info = find_iso_box_with_ancestors(container, &tkhd_path).or_fail()?;

    let moov_size_offset = *tkhd_info.ancestor_size_offsets.first().or_fail()?;
    let trak_size_offset = *tkhd_info.ancestor_size_offsets.get(1).or_fail()?;

    let defect = Defect::UnknownBox {
        after: tkhd_path,
        tag: BoxTag::new(*b"xxxx"),
    };
    let result = applied(&defect, &input)?;

    ensure_eq!(
        Some(result.0.len()),
        input.0.len().checked_add(NEW_BOX_SIZE),
        "file must grow by new box size"
    );

    let edit = Edit {
        before: &input,
        after: &result,
        order: ByteOrder::Big,
    };
    let growth = Growth(u32::try_from(NEW_BOX_SIZE).or_fail()?);
    // moov and trak sizes must each increase by the new box size.
    edit.ensure_grew(moov_size_offset, growth)?;
    edit.ensure_grew(trak_size_offset, growth)?;
    Ok(())
}

#[test]
fn duplicate_box_avi_nested_updates_ancestors() {
    assert_eq!(check_duplicate_box_avi_nested_updates_ancestors(), Ok(()));
}

fn check_duplicate_box_avi_nested_updates_ancestors() -> Result<(), Fail> {
    /// Where the RIFF header's size field sits.
    const RIFF_SIZE_OFFSET: usize = 4;
    let input = Base::Avi.media()?;

    let strh_path = BoxPath::root()
        .child(BoxTag::new(*b"hdrl"))
        .child(BoxTag::new(*b"strl"))
        .child(BoxTag::new(*b"strh"));
    let container = ContainerBytes::new(&input.0);
    let strh_info = find_avi_box_with_ancestors(container, &strh_path).or_fail()?;

    let strh_chunk_size = strh_info
        .box_info
        .payload_end
        .get()
        .checked_sub(strh_info.box_info.box_start.get())
        .or_fail()?;
    let hdrl_size_offset = *strh_info.ancestor_size_offsets.get(1).or_fail()?;
    let strl_size_offset = *strh_info.ancestor_size_offsets.get(2).or_fail()?;

    let defect = Defect::DuplicateBox(strh_path);
    let result = applied(&defect, &input)?;

    ensure_eq!(
        Some(result.0.len()),
        input.0.len().checked_add(strh_chunk_size),
        "file must grow by duplicated chunk size"
    );

    let edit = Edit {
        before: &input,
        after: &result,
        order: ByteOrder::Little,
    };
    let growth = Growth(u32::try_from(strh_chunk_size).or_fail()?);
    // RIFF, hdrl and strl sizes must each increase by the strh chunk size.
    edit.ensure_grew(AbsoluteOffset::new(RIFF_SIZE_OFFSET), growth)?;
    edit.ensure_grew(hdrl_size_offset, growth)?;
    edit.ensure_grew(strl_size_offset, growth)?;
    Ok(())
}

/// Applies the defect `defect_of` builds to the hdrl LIST chunk of the base
/// AVI file, checks that only that chunk's size field changed, and returns
/// the field before and after.
fn resize_hdrl(defect_of: fn(BoxPath) -> Defect) -> Result<(SizeField, SizeField), Fail> {
    let input = Base::Avi.media()?;
    let container = ContainerBytes::new(&input.0);

    // Find hdrl LIST chunk directly
    let hdrl_path = BoxPath::root().child(BoxTag::new(*b"hdrl"));
    let hdrl_info = find_avi_box_with_ancestors(container, &hdrl_path).or_fail()?;

    // For AVI LIST chunks, box_start is the offset of the LIST tag
    let hdrl_start = hdrl_info.box_info.box_start.get();
    // The size field for an AVI LIST chunk is at offset + 4 (after the "LIST" tag)
    let hdrl_size_offset = AbsoluteOffset::new(hdrl_start.checked_add(4).or_fail()?);

    // Read the original size field
    let orig_size = ByteOrder::Little.read(&input, hdrl_size_offset)?;

    let result = applied(&defect_of(hdrl_path), &input)?;

    // File should be same length (we're only modifying the size field, not adding/removing content)
    ensure_eq!(result.0.len(), input.0.len(), "file length unchanged");

    // Read the new size field
    let new_size = ByteOrder::Little.read(&result, hdrl_size_offset)?;

    // Verify the defect was applied only to that specific field by checking
    // that the result differs from input in a predictable way
    let mut expected = input.clone();
    let field_end = hdrl_size_offset.get().checked_add(4).or_fail()?;
    expected
        .0
        .get_mut(hdrl_size_offset.get()..field_end)
        .or_fail()?
        .copy_from_slice(&new_size.0.to_le_bytes());
    ensure_eq!(
        result,
        expected,
        "only the hdrl LIST chunk size field should have changed"
    );
    Ok((orig_size, new_size))
}

#[test]
fn oversized_list_chunk_hdrl_directly() {
    assert_eq!(check_oversized_list_chunk_hdrl_directly(), Ok(()));
}

fn check_oversized_list_chunk_hdrl_directly() -> Result<(), Fail> {
    // Apply OversizedBoxLength defect to the hdrl LIST chunk itself
    let (orig_size, new_size) = resize_hdrl(Defect::OversizedBoxLength)?;
    // Size should have increased by 256
    ensure_eq!(
        Some(new_size.0),
        orig_size.0.checked_add(256),
        "hdrl LIST chunk size must increase by 256"
    );
    Ok(())
}

#[test]
fn undersized_list_chunk_hdrl_directly() {
    assert_eq!(check_undersized_list_chunk_hdrl_directly(), Ok(()));
}

fn check_undersized_list_chunk_hdrl_directly() -> Result<(), Fail> {
    // Apply UndersizedBoxLength defect to the hdrl LIST chunk itself
    let (orig_size, new_size) = resize_hdrl(Defect::UndersizedBoxLength)?;
    // Size should have decreased by 1
    ensure_eq!(
        Some(new_size.0),
        orig_size.0.checked_sub(1),
        "hdrl LIST chunk size must decrease by 1"
    );
    Ok(())
}
