//! JPEG encoding configuration types.

/// Quality parameter for JPEG encoding (1-100).
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Quality(u8);

impl Quality {
    /// Creates a new Quality parameter.
    ///
    /// # Arguments
    /// * `value` - Quality value (1-100)
    ///
    /// # Returns
    /// `Some(Quality)` if value is in range, `None` otherwise.
    #[must_use]
    pub const fn new(value: u8) -> Option<Self> {
        if value >= 1 && value <= 100 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Gets the quality value.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// A JPEG chroma sampling factor in one dimension: `One` (no subsampling in
/// that dimension) or `Two` (2x subsampling).
///
/// This crate only ever produces these two values -- not because the JPEG
/// spec forbids 3 or 4, but because [`ChromaSampling`] (the only place a
/// `SamplingFactor` is ever built) defines only 4:4:4 and 4:2:0 -- so the
/// type is this closed set rather than a `u8` a caller could hand any
/// value at all, which `write_sof_component`'s `(h << 4) | v`
/// nibble-packing and `encode_scan`'s block-grid multipliers both depend
/// on staying small.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SamplingFactor {
    /// No subsampling in this dimension.
    One,
    /// 2x subsampling in this dimension.
    Two,
}

impl SamplingFactor {
    /// The raw JPEG sampling-factor value (1 or 2).
    #[must_use]
    pub const fn get(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }
}

/// The chroma subsampling factor for JPEG encoding, applied equally in
/// both dimensions.
///
/// Stores one [`SamplingFactor`], not a separate horizontal and vertical
/// field: this encoder only ever subsamples uniformly (`(One, One)` for
/// 4:4:4, `(Two, Two)` for 4:2:0), so a horizontal/vertical pair would
/// just be the same fact twice -- and a mixed pair like `(Two, One)`
/// (4:2:2) or `(One, Two)` (4:4:0), which nothing downstream
/// (`Plane::downsample_2x2`, `encode_scan`'s MCU grid) actually
/// implements, would then be representable with no compiler error.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SubsampleFactor(SamplingFactor);

impl SubsampleFactor {
    /// The 1:1 factor, used by every component that is not subsampled.
    ///
    /// Named so that a marker writer states which factor it means rather than
    /// passing a bare literal.
    pub const ONE_TO_ONE: Self = Self(SamplingFactor::One);

    /// Creates a new subsampling factor.
    ///
    /// Private: only [`ChromaSampling::subsample_factor`] constructs one,
    /// from the sampling mode it already knows.
    const fn new(factor: SamplingFactor) -> Self {
        Self(factor)
    }

    /// Returns the horizontal sampling factor (identical to
    /// [`Self::vertical`], since this encoder never subsamples
    /// non-uniformly).
    #[must_use]
    pub const fn horizontal(self) -> SamplingFactor {
        self.0
    }

    /// Returns the vertical sampling factor (identical to
    /// [`Self::horizontal`]; see there).
    #[must_use]
    pub const fn vertical(self) -> SamplingFactor {
        self.0
    }
}

/// Chroma sampling mode for JPEG encoding.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ChromaSampling {
    /// 4:4:4 sampling (no chroma subsampling).
    Yuv444,
    /// 4:2:0 sampling (chroma subsampling).
    Yuv420,
}

impl ChromaSampling {
    /// Returns the (horizontal, vertical) chroma subsampling factors: `(1, 1)`
    /// for 4:4:4 (no subsampling), `(2, 2)` for 4:2:0. `write_sof0`'s
    /// luma sampling-factor byte, `Plane::downsample_2x2`'s
    /// downsampling ratio, and `encode_scan`'s MCU block-grid
    /// multiplier must all agree with this -- deriving them from one
    /// place instead of three independent literals is the point.
    #[must_use]
    pub const fn subsample_factor(self) -> SubsampleFactor {
        match self {
            Self::Yuv444 => SubsampleFactor::new(SamplingFactor::One),
            Self::Yuv420 => SubsampleFactor::new(SamplingFactor::Two),
        }
    }
}
