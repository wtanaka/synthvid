//! The rectangle a frame covers, as the object-state arithmetic sees it.
//!
//! A box's extent and the frame's extent are both [`Bounds`]. Wrapping the frame
//! in its own type stops the two from being passed in each other's place, which
//! the object-state helpers would otherwise accept without complaint.

use synthvid_scene::Bounds;

/// The rectangle covered by a frame, in scene units.
#[derive(Copy, Clone, Debug)]
pub(crate) struct FrameBounds(pub(crate) Bounds);
