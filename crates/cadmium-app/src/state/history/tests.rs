use std::path::PathBuf;

use cadmium_engine::stroke::Tool;
use cadmium_engine::surface::Surface;

use super::*;
use crate::state::test_support::{PAINT_ROW_Y, painting_state, row_samples, sample_at, stroke};
use crate::tools::TrayTool;

#[test]
fn drying_a_canvas_without_wet_paint_does_nothing() {
    let mut state = painting_state();
    assert_eq!(state.dry(), DryOutcome::NothingWet);
    assert!(!state.has_unsaved_edits());
    assert!(!state.can_undo());
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
fn undo_lifts_a_hold_that_is_still_down() {
    let mut state = painting_state();
    state.set_tray_tool(TrayTool::Tube);
    state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
    state.advance_hold(0.1);
    assert!(state.undo());
    assert!(!state.holds_on(Surface::Tray));
    assert_eq!(
        state.session.tray.wet_totals(&state.gpu).unwrap().volume,
        0.0
    );
}
