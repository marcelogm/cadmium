use cadmium_engine::brush::{brush_to_json, factory_brushes};
use cadmium_engine::pigments::MAX_SLOTS;

use super::*;
use crate::state::test_support::{holding_the_knife, painting_state};

#[test]
fn pigments_already_on_the_canvas_are_not_offered_again() {
    let state = painting_state();
    let snapshot = Snapshot::of(&state);
    assert!(!snapshot.slots.is_empty());
    for choice in &snapshot.addable {
        assert!(!snapshot.slots.iter().any(|slot| slot == choice.id));
    }
    let all = state.session.db.all().len();
    assert_eq!(snapshot.addable.len() + snapshot.slots.len(), all);
}

#[test]
fn each_pile_is_named_with_its_colour_index() {
    let state = painting_state();
    let snapshot = Snapshot::of(&state);
    assert_eq!(snapshot.tooltips.len(), snapshot.slots.len());
    assert_eq!(snapshot.tooltips.len(), snapshot.slot_names.len());
    for (slot, tooltip) in snapshot.slots.iter().zip(&snapshot.tooltips) {
        let pigment = state.session.db.get(slot).unwrap();
        assert_eq!(
            tooltip,
            &format!("{} ({})", pigment.name, pigment.colour_index)
        );
    }
}

#[test]
fn an_empty_mixture_shows_the_primer() {
    let state = painting_state();
    let db = &state.session.db;
    let spec = &state.session.canvas.spec;
    let primer = primer_srgb8(db, spec.primer);
    let empty = Mixture {
        volume_um: 0.0,
        fractions: [0.0; MAX_SLOTS],
    };
    assert_eq!(
        mixture_colour(db, &spec.pigment_slots, primer, &empty),
        primer
    );
}

#[test]
fn a_freshly_loaded_brush_holds_paint() {
    let snapshot = Snapshot::of(&painting_state());
    assert!(!snapshot.held_is_clean);
    assert!(snapshot.held_level > 0.0);
    assert!(!snapshot.tool.holds_knife());
}

#[test]
fn a_cleaned_brush_holds_no_paint() {
    let mut state = painting_state();
    state.clean_brush();
    let snapshot = Snapshot::of(&state);
    assert!(snapshot.held_is_clean);
    assert!(snapshot.held_level <= f32::EPSILON);
}

#[test]
fn the_knife_shows_what_the_knife_holds_not_the_brush() {
    let state = holding_the_knife();
    assert!(!state.brush_is_clean());
    let snapshot = Snapshot::of(&state);
    assert!(snapshot.tool.holds_knife());
    assert!(snapshot.held_is_clean);
    assert!(snapshot.held_level <= f32::EPSILON);
}

#[test]
fn the_rack_lists_every_brush_in_the_library() {
    let state = painting_state();
    let snapshot = Snapshot::of(&state);
    let count = state.brushes.all().count();
    assert_eq!(snapshot.brush_entries.len(), count);
    assert_eq!(snapshot.brush_keys.len(), count);
}

#[test]
fn a_factory_brush_is_not_a_user_brush() {
    let mut state = painting_state();
    state.wear_library_brush(0);
    let snapshot = Snapshot::of(&state);
    assert_eq!(snapshot.brush_index, Some(0));
    assert!(!snapshot.brush_is_user);
}

#[test]
fn an_imported_brush_is_a_user_brush() {
    let mut state = painting_state();
    let json = brush_to_json(&factory_brushes()[0]);
    let index = state.import_brush(&json).unwrap().index;
    let snapshot = Snapshot::of(&state);
    assert_eq!(snapshot.brush_index, Some(index));
    assert!(snapshot.brush_is_user);
}
