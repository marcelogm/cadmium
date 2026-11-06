use cadmium_engine::canvas::{Primer, Weave};

use super::*;

#[test]
fn every_primer_sits_at_its_own_row() {
    for (index, (primer, _)) in PRIMERS.iter().enumerate() {
        assert_eq!(primer_index(*primer), index);
        assert_eq!(primer_at(index as u32), *primer);
    }
}

#[test]
fn every_weave_sits_at_its_own_row() {
    for (index, (weave, _)) in WEAVES.iter().enumerate() {
        assert_eq!(weave_index(*weave), index);
        assert_eq!(weave_at(index as u32), *weave);
    }
}

#[test]
fn no_selection_falls_back_to_white_gesso_on_linen() {
    // GTK's "no selection" is u32::MAX.
    assert_eq!(primer_at(u32::MAX), Primer::WhiteGesso);
    assert_eq!(weave_at(u32::MAX), Weave::Linen);
}
