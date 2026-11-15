use std::path::PathBuf;

use cadmium_engine::stroke::Tool;

use super::*;
use crate::state::ColourClick;
use crate::state::EaselId;
use crate::state::test_support::{
    PAINT_ROW_Y, finish_painting, paint_stroke, painting_state, present_one_frame, row_samples,
    sample_at, stroke, wet_volume,
};
use cadmium_engine::palette::ColourRef;

#[test]
fn drying_a_canvas_without_wet_paint_does_nothing() {
    let mut state = painting_state();
    assert_eq!(state.dry(), DryOutcome::NothingWet);
    assert!(!state.has_unsaved_edits());
    assert!(!state.can_undo(EaselId::Painting));
}

#[test]
fn drying_after_a_pencil_sketch_does_nothing() {
    let mut state = painting_state();
    stroke(&mut state, Tool::Pencil, &row_samples(PAINT_ROW_Y));
    let mark = state.edit_mark();
    state.saved_to(PathBuf::from("sketch.cadmium"), mark);
    assert_eq!(state.dry(), DryOutcome::NothingWet);
    assert!(!state.has_unsaved_edits());
}

#[test]
fn drying_wet_paint_can_be_undone() {
    let mut state = painting_state();
    stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    let mark = state.edit_mark();
    state.saved_to(PathBuf::from("dry.cadmium"), mark);
    assert_eq!(state.dry(), DryOutcome::Dried { undoable: true });
    assert!(state.has_unsaved_edits());
}

#[test]
fn undo_lifts_a_stroke_that_is_still_down() {
    let mut state = painting_state();
    state.press(EaselId::Painting, sample_at(280.0, 100.0, 1.0), false);
    state.motion(EaselId::Painting, &[sample_at(320.0, 100.0, 1.0)]);
    assert!(state.undo(EaselId::Painting));
    assert!(!state.is_stroking());
    finish_painting(&mut state, present_one_frame);
    assert_eq!(wet_volume(&state), 0.0);
}

#[test]
fn undoing_a_stroke_shows_the_brush_as_full_as_before_it() {
    let mut state = painting_state();
    state.click_colour(ColourRef::Paint(0), ColourClick::Load);
    let full = state.reserve_level();
    paint_stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    assert!(state.reserve_level() < full);
    assert!(state.undo(EaselId::Painting));
    assert_eq!(state.reserve_level(), full);
}

#[test]
fn redoing_a_stroke_shows_the_brush_as_drained_as_after_it() {
    let mut state = painting_state();
    state.click_colour(ColourRef::Paint(0), ColourClick::Load);
    paint_stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    let drained = state.reserve_level();
    assert!(state.undo(EaselId::Painting));
    assert!(state.reserve_level() > drained);
    assert!(state.redo(EaselId::Painting));
    assert_eq!(state.reserve_level(), drained);
}

fn saved_after_a_stroke() -> AppState {
    let mut state = painting_state();
    paint_stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    let mark = state.edit_mark();
    state.saved_to(PathBuf::from("history.cadmium"), mark);
    state
}

#[test]
fn undoing_on_a_saved_painting_leaves_unsaved_edits() {
    let mut state = saved_after_a_stroke();
    assert!(state.undo(EaselId::Painting));
    assert!(state.has_unsaved_edits());
}

#[test]
fn redoing_on_a_saved_painting_leaves_unsaved_edits() {
    let mut state = saved_after_a_stroke();
    assert!(state.undo(EaselId::Painting));
    let mark = state.edit_mark();
    state.saved_to(PathBuf::from("history.cadmium"), mark);
    assert!(state.redo(EaselId::Painting));
    assert!(state.has_unsaved_edits());
}

#[test]
fn undoing_with_nothing_to_undo_leaves_a_saved_painting_saved() {
    let mut state = painting_state();
    let mark = state.edit_mark();
    state.saved_to(PathBuf::from("history.cadmium"), mark);
    assert!(!state.undo(EaselId::Painting));
    assert!(!state.has_unsaved_edits());
}
