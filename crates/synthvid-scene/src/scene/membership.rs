//! The answer to whether a frame lies inside a span.

/// Whether a frame index lies inside a frame span.
///
/// Returned by `FrameSpan::contains` in place of a bare `bool`, so the two
/// answers carry names at every call site.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Membership {
    /// The frame lies in `[start, end)`.
    Inside,
    /// The frame lies before the span's start or at or after its end.
    Outside,
}
