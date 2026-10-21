//! Typed field values and positions for the ISO box writers.
//!
//! Each type holds one quantity that a box writer takes, so that no writer
//! signature names a bare integer or a bare four-byte tag.

/// A 32-bit unsigned field, written big-endian.
#[derive(Clone, Copy)]
pub(crate) struct U32Be(pub(crate) u32);

/// A 16-bit unsigned field, written big-endian.
#[derive(Clone, Copy)]
pub(crate) struct U16Be(pub(crate) u16);

/// A 32-bit signed field, written big-endian.
#[derive(Clone, Copy)]
pub(crate) struct I32Be(pub(crate) i32);

/// The four-character type of a box.
#[derive(Clone, Copy)]
pub(crate) struct BoxType(pub(crate) [u8; 4]);

/// The size of a box in bytes, including its header.
#[derive(Clone, Copy)]
pub(crate) struct BoxSize(pub(crate) u32);

/// A byte position within a container buffer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ByteOffset(pub(crate) usize);
