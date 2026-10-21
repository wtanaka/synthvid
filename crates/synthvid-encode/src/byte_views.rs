//! Borrowed views that carry a byte buffer, a frame list or one frame's payload
//! through the container writers, so that no writer signature names a bare
//! `Vec<u8>` or `[u8]`.

use crate::media::EncodedFrame;

/// A byte buffer that a container writer appends to.
pub(crate) struct ByteSink<'sink>(pub(crate) &'sink mut Vec<u8>);

/// The encoded frames of one track, in presentation order.
#[derive(Clone, Copy)]
pub(crate) struct FrameList<'frames>(pub(crate) &'frames [EncodedFrame]);

/// The encoded payload of one frame.
#[derive(Clone, Copy)]
pub(crate) struct FramePayload<'payload>(pub(crate) &'payload [u8]);
