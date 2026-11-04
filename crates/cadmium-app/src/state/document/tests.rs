use super::*;
use crate::presets::starting_canvas_spec;
use crate::state::test_support::{
    PAINT_ROW_Y, TEST_BRUSH_DIAMETER_PX, painting_state, row_samples, squeeze_blob_on_tray, stroke,
    wet_volume,
};
use cadmium_engine::engine::PaintEngine;
use cadmium_engine::params::SimParams;
use cadmium_engine::stroke::Tool;

#[test]
fn opened_painting_keeps_the_brush_and_lab_and_remembers_its_file() {
    const PIGMENT: &str = "ultramarine_blue";
    const TUNED_PARAM_VALUE_SHARE: f32 = 0.25;
    let mut current = painting_state();
    let slot = current
        .session
        .canvas
        .spec
        .pigment_slots
        .iter()
        .position(|id| id == PIGMENT)
        .unwrap();
    current.session.brush.recipe = vec![(slot, 1.0)];
    let spec = &SimParams::specs()[0];
    let tuned = spec.min + (spec.max - spec.min) * TUNED_PARAM_VALUE_SHARE;
    current.set_param(spec.key, tuned);
    current.edits.record();

    let mut saved = painting_state();
    saved.session.canvas.spec.pigment_slots.reverse();
    let reversed_palette = saved.session.canvas.spec.pigment_slots.clone();
    let snapshot = saved.capture_document().unwrap();
    let path = PathBuf::from("opened.cadmium");
    current.open_painting(snapshot, path.clone()).unwrap();

    assert_eq!(current.session.canvas.spec.pigment_slots, reversed_palette);
    let new_slot = reversed_palette
        .iter()
        .position(|id| id == PIGMENT)
        .unwrap();
    assert_eq!(current.session.brush.recipe, [(new_slot, 1.0)]);
    assert_eq!(
        current.session.brush.geometry.max_diameter_px,
        TEST_BRUSH_DIAMETER_PX
    );
    assert_eq!(current.session.engine.params().get(spec.key), tuned);
    assert_eq!(current.file, Some(path));
    assert!(!current.has_unsaved_edits());
}

#[test]
fn a_new_painting_keeps_the_lab_and_starts_untitled_without_undo() {
    const TUNED_PARAM_VALUE_SHARE: f32 = 0.75;
    let mut state = painting_state();
    let spec = &SimParams::specs()[0];
    let tuned = spec.min + (spec.max - spec.min) * TUNED_PARAM_VALUE_SHARE;
    state.set_param(spec.key, tuned);
    stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    let mark = state.edit_mark();
    state.saved_to(PathBuf::from("antiga.cadmium"), mark);
    let blank = state.session.canvas.spec.clone();
    state.start_painting(blank).unwrap();
    assert_eq!(state.file, None);
    assert!(!state.can_undo());
    assert!(!state.has_unsaved_edits());
    assert_eq!(wet_volume(&state), 0.0);
    assert_eq!(state.session.engine.params().get(spec.key), tuned);
    assert!(!state.brush_is_clean());
}

#[test]
fn the_tube_falls_back_to_the_first_pigment_on_a_smaller_palette() {
    let mut state = painting_state();
    let last = state.slot_count() - 1;
    state.set_tube_slot(last);
    let mut spec = state.session.canvas.spec.clone();
    spec.pigment_slots.truncate(2);
    state.start_painting(spec).unwrap();
    assert_eq!(state.tube_slot, 0);
}

#[test]
fn a_new_painting_starts_with_a_clean_tray() {
    let mut state = painting_state();
    squeeze_blob_on_tray(&mut state, 0);
    state.start_painting(starting_canvas_spec()).unwrap();
    assert_eq!(
        state.session.tray.wet_totals(&state.gpu).unwrap().volume,
        0.0
    );
}
