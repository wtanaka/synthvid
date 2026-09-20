//! Frame-to-YCbCr plane conversion and per-sampling-mode active plane
//! selection.

use crate::color::rgb_to_ycbcr;
use synthvid_scene::{Frame, Rgb8};

use super::plane::{Plane, PlaneDimensions};

/// The three planes a frame converts into: luma, blue-difference chroma,
/// red-difference chroma. A named struct, rather than a positional
/// `(Plane, Plane, Plane)` tuple, so a caller can't accidentally transpose
/// two same-typed planes -- e.g. handing the Cb plane where Cr belongs --
/// with no compiler error, the same reasoning `BlockCoord` and
/// `PlaneDimensions` already apply to bundling `u32`/`Width`/`Height` pairs.
///
/// Its fields are private to this module, and [`Self::from_frame`] is its
/// only constructor: a struct literal built anywhere in `jpeg.rs` (e.g.
/// `YCbCrPlanes { luma, cb: red_plane, cr: blue_plane }`, swapping the two
/// chroma planes), or a positional call passing three same-typed `Plane`s
/// in the wrong order, can no longer be written outside this file.
pub(super) struct YCbCrPlanes {
    /// Luma (Y) plane.
    luma: Plane,
    /// Blue-difference chroma (Cb) plane.
    cb: Plane,
    /// Red-difference chroma (Cr) plane.
    cr: Plane,
}

impl YCbCrPlanes {
    /// Converts a frame's RGB pixel data to YCbCr planes.
    ///
    /// Builds three planes: luma, blue chroma, red chroma. Each plane is
    /// pre-filled with appropriate padding values (0 for luma, 128 for
    /// chroma) and overwritten for valid pixels.
    ///
    /// The only constructor of `YCbCrPlanes`. All three planes share one
    /// `dims` derived from `frame` itself (not three independently-sized
    /// `Plane`s a caller could pass with disagreeing dimensions), and each
    /// converted pixel's `y()`/`cb()`/`cr()` value is written to
    /// `self.luma`/`self.cb`/`self.cr` by field name, in this one place --
    /// unlike a previous version of this function, which built the three
    /// planes separately in `jpeg.rs` and returned a `(&mut Plane, &mut
    /// Plane, &mut Plane)` tuple for the caller to destructure, an
    /// unnamed triple of same-typed values with exactly the swap risk
    /// this type otherwise exists to rule out.
    ///
    /// Takes `&Frame`, not a separate pixel-data slice and
    /// `PlaneDimensions`: `Frame` itself guarantees its data is exactly
    /// `width * height * 3` bytes (see `lib.rs`'s
    /// `test_frame_rejects_mismatched_data_length`), so deriving both the
    /// pixel bytes and the dimensions from the same `frame` value here
    /// means a caller can no longer pass a `frame_data` slice and a
    /// `dims` that disagree.
    pub(super) fn from_frame(frame: &Frame) -> Self {
        let dims = PlaneDimensions::new(frame.width(), frame.height());
        let mut planes = Self {
            luma: Plane::filled(dims, super::JpegComponent::Y.huffman_class()),
            cb: Plane::filled(dims, super::JpegComponent::Cb.huffman_class()),
            cr: Plane::filled(dims, super::JpegComponent::Cr.huffman_class()),
        };

        let mut pixels = planes
            .luma
            .data_mut()
            .iter_mut()
            .zip(planes.cb.data_mut().iter_mut())
            .zip(planes.cr.data_mut().iter_mut());

        let (chunks, _remainder) = frame.data().as_chunks::<3>();
        for chunk in chunks {
            let Some(((luma, blue), red)) = pixels.next() else {
                break;
            };
            let [r, g, b] = *chunk;
            let ycbcr = rgb_to_ycbcr(Rgb8::new(r, g, b));
            *luma = ycbcr.y();
            *blue = ycbcr.cb();
            *red = ycbcr.cr();
        }

        planes
    }

    /// A full-resolution view of all three planes, for 4:4:4 encoding.
    /// The only way to build an `ActivePlanes` that borrows every plane at
    /// its original resolution: the pairing (which field borrows which
    /// plane) is fixed here, once, by field name, rather than
    /// reconstructed at each call site via a struct literal that could
    /// name the wrong plane for a field.
    #[must_use]
    pub(super) const fn full_resolution(&self) -> ActivePlanes<'_> {
        ActivePlanes {
            luma: &self.luma,
            cb: &self.cb,
            cr: &self.cr,
        }
    }

    /// Downsamples both chroma planes by 2x2, for 4:2:0 encoding, bundled
    /// with a borrow of this `YCbCrPlanes`'s own luma plane into one
    /// self-contained [`DownsampledChroma`] -- see
    /// [`DownsampledChroma::active`].
    #[must_use]
    pub(super) fn downsample_chroma(&self) -> DownsampledChroma<'_> {
        DownsampledChroma {
            luma: &self.luma,
            cb: self.cb.downsample_2x2(),
            cr: self.cr.downsample_2x2(),
        }
    }
}

/// The luma plane at its original resolution, borrowed, paired with
/// already-downsampled chroma planes, owned -- for 4:2:0 encoding. Bundles
/// everything [`Self::active`] needs into one self-contained value, rather
/// than [`ActivePlanes`] being built from this plane's luma and a
/// separately-passed chroma argument that could come from a different
/// `YCbCrPlanes` entirely (nothing would tie the two together). Its
/// fields are private to this module for the same reason
/// [`YCbCrPlanes`]'s are.
pub(super) struct DownsampledChroma<'a> {
    /// Original-resolution luma plane, borrowed from the `YCbCrPlanes`
    /// this was downsampled from.
    luma: &'a Plane,
    /// Downsampled Cb plane.
    cb: Plane,
    /// Downsampled Cr plane.
    cr: Plane,
}

impl DownsampledChroma<'_> {
    /// The active planes for 4:2:0 encoding: the original-resolution luma
    /// plane and this value's own downsampled chroma planes. Takes no
    /// arguments beyond `self` -- unlike a method that also accepted a
    /// separate chroma value, there is nothing here a caller could pass
    /// that came from a different `YCbCrPlanes`.
    #[must_use]
    pub(super) const fn active(&self) -> ActivePlanes<'_> {
        ActivePlanes {
            luma: self.luma,
            cb: &self.cb,
            cr: &self.cr,
        }
    }
}

/// The three planes currently active for block extraction, borrowed
/// rather than owned so this can point at either the original
/// full-resolution [`YCbCrPlanes`] (4:4:4) or a mix of the original luma
/// plane and freshly downsampled chroma planes (4:2:0) without copying
/// luma either way.
///
/// Its fields are private to this module: the only ways to build one are
/// [`YCbCrPlanes::full_resolution`] and [`DownsampledChroma::active`],
/// both of which fix the luma/Cb/Cr pairing by field name in one place,
/// rather than a struct literal at each of `encode_scan`'s two sampling
/// arms that could name the wrong plane for a field (e.g. binding the
/// downsampled Cr plane to the `cb` field).
pub(super) struct ActivePlanes<'a> {
    /// Luma (Y) plane currently in use.
    luma: &'a Plane,
    /// Blue-difference chroma (Cb) plane currently in use.
    cb: &'a Plane,
    /// Red-difference chroma (Cr) plane currently in use.
    cr: &'a Plane,
}

impl ActivePlanes<'_> {
    /// The plane for `component`. Deriving the plane from the same
    /// `JpegComponent` value a caller also uses to pick a DC predictor
    /// (see `DcPredictors::slot`) means the two are never chosen
    /// independently -- there is no way to extract a block from one
    /// component's plane while advancing a different component's
    /// predictor, the way there would be if a caller picked a plane field
    /// and a `JpegComponent` as two separate, unrelated expressions.
    pub(super) const fn plane(&self, component: super::JpegComponent) -> &Plane {
        match component {
            super::JpegComponent::Y => self.luma,
            super::JpegComponent::Cb => self.cb,
            super::JpegComponent::Cr => self.cr,
        }
    }
}
