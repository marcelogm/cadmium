use super::*;
use crate::state::test_support::{
    ULTRAMARINE_SLOT, WHITE_SLOT, brush_totals, painting_state, settle_both_surfaces,
    squeeze_blob_on_tray,
};
use crate::tools::TrayTool;

const YELLOW_SLOT: usize = 1;

#[test]
fn a_pile_click_with_the_tube_only_chooses_the_tube() {
    let mut state = painting_state();
    state.set_tray_tool(TrayTool::Tube);
    let before = brush_totals(&state);
    let recipe = state.session.brush.recipe.clone();
    state.click_pile(ULTRAMARINE_SLOT, PigmentClick::Mix);
    assert_eq!(state.tube_slot, ULTRAMARINE_SLOT);
    assert_eq!(state.session.brush.recipe, recipe);
    assert_eq!(brush_totals(&state), before);
}

#[test]
fn a_pile_click_with_the_brush_loads_the_brush_and_leaves_the_tube() {
    let mut state = painting_state();
    state.click_pile(YELLOW_SLOT, PigmentClick::Choose);
    assert_eq!(state.session.brush.recipe, [(YELLOW_SLOT, 1.0)]);
    assert!(brush_totals(&state).pigment[YELLOW_SLOT] > 0.0);
    assert_eq!(state.tube_slot, WHITE_SLOT);
}

#[test]
fn adding_a_pigment_while_holding_the_tube_makes_it_the_tube_and_leaves_the_brush_unloaded() {
    let mut state = painting_state();
    let mut spec = state.session.canvas.spec.clone();
    spec.pigment_slots.truncate(ULTRAMARINE_SLOT + 1);
    state.start_painting(spec).unwrap();
    state.set_tray_tool(TrayTool::Tube);
    let before = brush_totals(&state);
    let recipe = state.session.brush.recipe.clone();
    let slot = state.add_pile("burnt_umber").unwrap();
    assert_eq!(slot, ULTRAMARINE_SLOT + 1);
    assert_eq!(state.tube_slot, slot);
    assert_eq!(state.session.brush.recipe, recipe);
    assert_eq!(brush_totals(&state), before);
}

#[test]
fn adding_a_pigment_with_the_brush_loads_it_clean() {
    let mut state = painting_state();
    let mut spec = state.session.canvas.spec.clone();
    spec.pigment_slots.truncate(ULTRAMARINE_SLOT + 1);
    state.start_painting(spec).unwrap();
    let slot = state.add_pile("burnt_umber").unwrap();
    assert_eq!(state.session.brush.recipe, [(slot, 1.0)]);
    assert_eq!(brush_totals(&state).pigment[ULTRAMARINE_SLOT], 0.0);
    assert_eq!(state.tube_slot, WHITE_SLOT);
}

#[test]
fn clearing_the_tray_gives_only_the_tray_pending_work() {
    let mut state = painting_state();
    squeeze_blob_on_tray(&mut state, 1);
    settle_both_surfaces(&mut state);
    state.clear_tray();
    assert!(state.session.tray_has_pending_work());
    assert!(!state.session.has_pending_work());
}

#[test]
fn choosing_a_tube_does_not_change_the_brush() {
    let mut state = painting_state();
    let recipe = state.session.brush.recipe.clone();
    state.set_tube_slot(1);
    assert_eq!(state.session.brush.recipe, recipe);
    assert_eq!(state.tube_slot, 1);
}

#[test]
fn a_tube_beyond_the_palette_is_ignored() {
    let mut state = painting_state();
    state.set_tube_slot(state.slot_count());
    assert_eq!(state.tube_slot, 0);
}

#[test]
fn clearing_the_tray_is_an_edit() {
    let mut state = painting_state();
    squeeze_blob_on_tray(&mut state, 0);
    state.clear_tray();
    assert_eq!(
        state.session.tray.wet_totals(&state.gpu).unwrap().volume,
        0.0
    );
    assert!(state.has_unsaved_edits());
}

#[test]
fn clearing_a_clean_tray_is_not_an_edit() {
    let mut state = painting_state();
    state.clear_tray();
    assert!(!state.has_unsaved_edits());
}
