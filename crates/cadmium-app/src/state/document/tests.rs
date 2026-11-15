use super::*;
use crate::state::EaselId;
use crate::state::test_support::{
    PAINT_ROW_Y, TEST_BRUSH_DIAMETER_PX, painting_state, row_samples, stroke, wet_volume,
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
        .painting
        .session
        .canvas
        .spec
        .pigment_slots
        .iter()
        .position(|id| id == PIGMENT)
        .unwrap();
    current.painting.session.brush.recipe = vec![(slot, 1.0)];
    let spec = &SimParams::specs()[0];
    let tuned = spec.min + (spec.max - spec.min) * TUNED_PARAM_VALUE_SHARE;
    current.set_param(spec.key, tuned);
    current.edits.record();

    let mut saved = painting_state();
    saved.painting.session.canvas.spec.pigment_slots.reverse();
    let reversed_palette = saved.painting.session.canvas.spec.pigment_slots.clone();
    let snapshot = saved.capture_document().unwrap();
    let path = PathBuf::from("opened.cadmium");
    current.open_painting(snapshot, path.clone()).unwrap();

    let opened = &current.painting.session;
    assert_eq!(opened.canvas.spec.pigment_slots, reversed_palette);
    let new_slot = reversed_palette
        .iter()
        .position(|id| id == PIGMENT)
        .unwrap();
    assert_eq!(opened.brush.recipe, [(new_slot, 1.0)]);
    assert_eq!(
        opened.brush.geometry.max_diameter_px,
        TEST_BRUSH_DIAMETER_PX
    );
    assert_eq!(opened.engine.params().get(spec.key), tuned);
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
    state.saved_to(PathBuf::from("old.cadmium"), mark);
    let blank = state.painting.session.canvas.spec.clone();
    state.start_painting(blank).unwrap();
    assert_eq!(state.file, None);
    assert!(!state.can_undo(EaselId::Painting));
    assert!(!state.has_unsaved_edits());
    assert_eq!(wet_volume(&state), 0.0);
    assert_eq!(state.painting.session.engine.params().get(spec.key), tuned);
    assert!(!state.brush_is_clean());
}
