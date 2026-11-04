use super::*;
use cadmium_engine::canvas::CanvasSpec;

use crate::presets::{PIGMENT_PRESETS, ZORN, starting_canvas_spec};

#[test]
fn ninth_pigment_is_refused() {
    let db = PigmentDb::builtin();
    let mut selection = PigmentSelection::new(&db, &starting_canvas_spec().pigment_slots);
    assert!(!selection.set("zinc_white", true));
    assert!(!selection.contains("zinc_white"));
    assert_eq!(selection.count(), MAX_SLOTS);
}

#[test]
fn rechoosing_a_pigment_in_a_full_selection_is_accepted() {
    let db = PigmentDb::builtin();
    let mut selection = PigmentSelection::new(&db, &starting_canvas_spec().pigment_slots);
    assert!(selection.set("titanium_white", true));
    assert_eq!(selection.count(), MAX_SLOTS);
}

#[test]
fn dropping_a_pigment_frees_a_slot() {
    let db = PigmentDb::builtin();
    let mut selection = PigmentSelection::new(&db, &starting_canvas_spec().pigment_slots);
    assert!(selection.set("phthalo_blue", false));
    assert!(selection.set("zinc_white", true));
    assert_eq!(selection.count(), MAX_SLOTS);
}

#[test]
fn unknown_pigment_cannot_be_chosen() {
    let db = PigmentDb::builtin();
    let mut selection = PigmentSelection::new(&db, &[]);
    assert!(!selection.set("mars_violet", true));
    assert_eq!(selection.count(), 0);
}

#[test]
fn slots_follow_the_pigment_database_order() {
    let db = PigmentDb::builtin();
    let mut selection = PigmentSelection::new(&db, &[]);
    selection.set("ivory_black", true);
    selection.set("titanium_white", true);
    assert_eq!(selection.slots(), ["titanium_white", "ivory_black"]);
}

#[test]
fn every_preset_yields_a_valid_canvas_spec() {
    let db = PigmentDb::builtin();
    for preset in &PIGMENT_PRESETS {
        let mut selection = PigmentSelection::new(&db, &[]);
        selection.apply_preset(preset.pigments);
        assert_eq!(selection.count(), preset.pigments.len(), "{}", preset.name);
        let spec = CanvasSpec {
            pigment_slots: selection.slots(),
            ..starting_canvas_spec()
        };
        assert!(spec.validate(&db).is_ok(), "{}", preset.name);
    }
}

#[test]
fn applying_a_preset_replaces_the_selection() {
    let db = PigmentDb::builtin();
    let mut selection = PigmentSelection::new(&db, &starting_canvas_spec().pigment_slots);
    selection.apply_preset(ZORN);
    assert_eq!(
        selection.slots(),
        [
            "titanium_white",
            "yellow_ochre",
            "cadmium_red",
            "ivory_black"
        ]
    );
}

#[test]
fn counter_reads_n_of_eight() {
    assert_eq!(counter_text(6), "6 de 8");
}
