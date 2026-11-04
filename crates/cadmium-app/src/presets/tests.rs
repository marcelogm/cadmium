use cadmium_engine::pigments::{MAX_SLOTS, PigmentDb};

use super::*;

#[test]
fn starting_painting_is_valid_and_fills_every_slot_with_the_primaries() {
    let spec = starting_canvas_spec();
    assert!(spec.validate(&PigmentDb::builtin()).is_ok());
    assert_eq!(spec.pigment_slots.len(), MAX_SLOTS);
    assert_eq!(spec.pigment_slots, PRIMARIES);
}

#[test]
fn every_preset_is_a_valid_palette() {
    let db = PigmentDb::builtin();
    for preset in &PIGMENT_PRESETS {
        let spec = CanvasSpec {
            pigment_slots: preset.pigments.iter().map(|id| id.to_string()).collect(),
            ..starting_canvas_spec()
        };
        assert!(spec.validate(&db).is_ok(), "{}", preset.name);
    }
}

fn slots(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| id.to_string()).collect()
}

#[test]
fn first_brush_carries_ultramarine_when_the_palette_has_it() {
    let spec = starting_canvas_spec();
    let recipe = starting_recipe(&spec.pigment_slots);
    assert_eq!(spec.pigment_slots[recipe[0].0], "ultramarine_blue");
}

#[test]
fn first_brush_skips_the_whites_when_there_is_no_ultramarine() {
    assert_eq!(starting_recipe(&slots(ZORN)), [(1, 1.0)]);
    assert_eq!(
        starting_recipe(&slots(&["zinc_white", "titanium_white", "ivory_black"])),
        [(2, 1.0)]
    );
}

#[test]
fn an_all_white_palette_starts_with_the_first_white() {
    assert_eq!(starting_recipe(&slots(&["titanium_white"])), [(0, 1.0)]);
}
