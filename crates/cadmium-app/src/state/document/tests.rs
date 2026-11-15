use super::*;
use crate::presets::starting_canvas_spec;
use crate::state::test_support::{
    PAINT_ROW_Y, TEST_BRUSH_DIAMETER_PX, painting_state, row_samples, stroke, stroke_on, wet_volume,
};
use crate::state::{ColourClick, EaselId};
use cadmium_engine::engine::PaintEngine;
use cadmium_engine::palette::ColourRef;
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

fn paint_on_the_sheet(state: &mut AppState) {
    state.begin_palette_edit();
    state.load_sheet_colour(ColourRef::Paint(1), ColourClick::Load);
    stroke_on(state, EaselId::Sheet, &row_samples(200.0));
    state.end_palette_edit();
}

fn sheet_volume(state: &AppState) -> f64 {
    state
        .sheet
        .session
        .canvas
        .wet_totals(&state.gpu)
        .unwrap()
        .volume
}

#[test]
fn a_new_painting_starts_with_a_blank_mixing_sheet() {
    let mut state = painting_state();
    paint_on_the_sheet(&mut state);
    state.start_painting(starting_canvas_spec()).unwrap();
    assert_eq!(sheet_volume(&state), 0.0);
    assert!(!state.can_undo(EaselId::Sheet));
    assert!(state.sheet.brush_is_clean());
    assert_eq!(state.sheet.selected_recipe(), None);
}

#[test]
fn an_opened_painting_brings_back_the_sheet_it_was_saved_with() {
    let mut saved = painting_state();
    paint_on_the_sheet(&mut saved);
    let snapshot = saved.capture_document().unwrap();
    let painted = sheet_volume(&saved);
    assert!(painted > 0.0);
    let mut opened = painting_state();
    opened
        .open_painting(snapshot, PathBuf::from("mixed.cadmium"))
        .unwrap();
    assert!((sheet_volume(&opened) - painted).abs() < 1e-6 * painted);
    assert!(!opened.can_undo(EaselId::Sheet));
    assert!(!opened.has_unsaved_edits());
    let resaved = opened.capture_document().unwrap();
    let mut reopened = painting_state();
    reopened
        .open_painting(resaved, PathBuf::from("mixed.cadmium"))
        .unwrap();
    assert!((sheet_volume(&reopened) - painted).abs() < 1e-6 * painted);
}
