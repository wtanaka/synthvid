//! DC predictor tracking, one per JPEG component.

use crate::bitstream::BitstreamWriter;
use crate::dct::{DcCoefficient, DcPredictor};
use crate::huffman::encode_dc;

use super::JpegComponent;

/// DC predictors tracked for each JPEG component (luma, Cb, Cr).
pub(super) struct DcPredictors {
    /// DC predictor for the Y (luma) component.
    luma: DcPredictor,
    /// DC predictor for the Cb (blue chroma) component.
    cb: DcPredictor,
    /// DC predictor for the Cr (red chroma) component.
    cr: DcPredictor,
}

impl DcPredictors {
    /// Creates a new set of DC predictors initialized to zero.
    pub(super) const fn new() -> Self {
        Self {
            luma: DcPredictor::INITIAL,
            cb: DcPredictor::INITIAL,
            cr: DcPredictor::INITIAL,
        }
    }

    /// The slot for `component`: which predictor `encode_component_block`
    /// should advance, paired with the component identity itself so the
    /// two can never be picked independently and disagree -- unlike a
    /// separate `Luma`/`Cb`/`Cr` enum that repeats the same three-way
    /// split `JpegComponent` already makes. This is the only constructor
    /// of `ComponentSlot`: its fields are private to this module, so a
    /// struct literal written anywhere else -- even elsewhere in
    /// `jpeg.rs` -- cannot pair a component with a predictor that wasn't
    /// this match's own arm for it (e.g. `Y` paired with the `cb`
    /// predictor).
    pub(super) const fn slot(&mut self, component: JpegComponent) -> ComponentSlot<'_> {
        let predictor = match component {
            JpegComponent::Y => &mut self.luma,
            JpegComponent::Cb => &mut self.cb,
            JpegComponent::Cr => &mut self.cr,
        };
        ComponentSlot {
            component,
            predictor,
        }
    }
}

/// A block's component paired with its dedicated DC predictor.
pub(super) struct ComponentSlot<'a> {
    /// Which JPEG component this slot is for.
    component: JpegComponent,
    /// The predictor this component advances.
    predictor: &'a mut DcPredictor,
}

impl ComponentSlot<'_> {
    /// Encodes `dc`'s delta against this slot's predictor, writes it to
    /// `writer`, and advances the predictor by the actually-written delta
    /// -- as one operation, rather than three separate call-site steps
    /// (compute the delta, write it, advance the predictor) that a future
    /// edit could reorder, drop, repeat, or split across two different
    /// slots' predictors. Any of those would still produce a
    /// self-consistent, decodable bitstream, just one whose DC predictor
    /// has silently desynchronized from what a real decoder reconstructs
    /// -- nothing about the three steps as independent statements ties
    /// them together as a single unit the way this method does.
    pub(super) fn encode_dc(&mut self, dc: DcCoefficient, writer: &mut BitstreamWriter) {
        let component = self.component.huffman_class();
        let delta = dc.delta_from(*self.predictor);
        let encoded_delta = encode_dc(delta, writer, component);
        *self.predictor = self.predictor.advanced_by(encoded_delta);
    }
}
