//! The primers and canvas weaves the dialog offers, in their rows' order.

use cadmium_engine::canvas::{Primer, Weave};

pub(super) const PRIMERS: [(Primer, &str); 2] = [
    (Primer::WhiteGesso, "White gesso"),
    (Primer::SiennaImprimatura, "Sienna imprimatura"),
];

pub(super) const WEAVES: [(Weave, &str); 3] = [
    (Weave::Linen, "Linen"),
    (Weave::Cotton, "Cotton"),
    (Weave::Smooth, "Smooth"),
];

/// The primer at a combo row's `selected` index; white gesso when nothing is selected.
pub(super) fn primer_at(selected: u32) -> Primer {
    PRIMERS
        .get(selected as usize)
        .map_or(Primer::WhiteGesso, |(primer, _)| *primer)
}

/// The row of `primer`; the first row if it isn't listed.
pub(super) fn primer_index(primer: Primer) -> usize {
    PRIMERS
        .iter()
        .position(|(candidate, _)| *candidate == primer)
        .unwrap_or_default()
}

/// The weave at a combo row's `selected` index; linen when nothing is selected.
pub(super) fn weave_at(selected: u32) -> Weave {
    WEAVES
        .get(selected as usize)
        .map_or(Weave::Linen, |(weave, _)| *weave)
}

/// The row of `weave`; the first row if it isn't listed.
pub(super) fn weave_index(weave: Weave) -> usize {
    WEAVES
        .iter()
        .position(|(candidate, _)| *candidate == weave)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
