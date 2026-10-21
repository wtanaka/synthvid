//! Newtypes the defect modules pass between one another. Each names one
//! quantity, so a value of one kind cannot be handed where another belongs.

use super::defects_box::ContainerBytes;
use super::defects_reorder::ExtractedBox;
use super::BoxPath;

/// The bytes of a container: a file being edited, or a box extracted from one.
pub(super) struct FileBytes(pub(super) Vec<u8>);

impl FileBytes {
    /// A copy of a container's bytes, ready to be edited.
    pub(super) fn copy_of(input: ContainerBytes<'_>) -> Self {
        Self(input.as_slice().to_vec())
    }

    /// Views these bytes as a container.
    pub(super) fn as_container(&self) -> ContainerBytes<'_> {
        ContainerBytes::new(&self.0)
    }
}

/// A 32-bit value to write into a field of a container.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FieldValue(pub(super) u32);

/// A byte distance from the start of a box's payload. Distinct from an
/// absolute file offset, so the two cannot be confused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FieldOffset(pub(super) usize);

/// Four bytes read literally from a container, before they are read as a
/// number or kept as a tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RawWord(pub(super) [u8; 4]);

/// The format a container is in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ContainerFormat {
    /// ISO base media format (MP4).
    Iso,
    /// AVI, a RIFF container.
    Avi,
    /// Neither format.
    Neither,
}

/// The two box paths a reorder defect names, in the order the defect gives them.
#[derive(Clone, Copy)]
pub(super) struct PathPair<'paths> {
    /// The first path.
    pub(super) first: &'paths BoxPath,
    /// The second path.
    pub(super) second: &'paths BoxPath,
}

/// The two boxes a reorder defect moves, in the order the defect names them.
pub(super) struct BoxPair {
    /// The box the first path names.
    pub(super) first: ExtractedBox,
    /// The box the second path names.
    pub(super) second: ExtractedBox,
}

/// Which sibling with a given tag a box is, counting from zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Occurrence(pub u32);
