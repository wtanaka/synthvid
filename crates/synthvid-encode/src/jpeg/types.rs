//! Newtypes the JPEG frame coder is built from: a component's marker ID byte
//! and the pixel extent of one MCU in one dimension.

/// A component's marker ID byte (1 = Y, 2 = Cb, 3 = Cr), as SOF0 and SOS
/// both write it.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct ComponentId(pub(super) u8);

/// The pixel width or height of one MCU in one dimension: `8` without
/// subsampling, `16` at 2x.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct McuExtent(pub(super) u32);
