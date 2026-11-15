use std::path::PathBuf;

use cadmium_engine::stroke::Tool;

use crate::state::test_support::{PAINT_ROW_Y, painting_state, row_samples, sample_at, stroke};
use crate::state::{AppState, EaselId};
use crate::tools::UiTool;

fn saved_painting() -> AppState {
    let mut state = painting_state();
    let mark = state.edit_mark();
    state.saved_to(PathBuf::from("saved.cadmium"), mark);
    assert!(!state.has_unsaved_edits());
    state
}

#[test]
fn a_stroke_lasts_until_the_pen_lifts() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    assert!(!state.is_stroking());
    state.press(EaselId::Painting, sample_at(100.0, 100.0, 0.9), false);
    assert!(state.is_stroking());
    state.release(EaselId::Painting);
    assert!(!state.is_stroking());
}

#[test]
fn a_stroke_on_a_saved_painting_leaves_unsaved_edits() {
    let mut state = saved_painting();
    stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    assert!(state.has_unsaved_edits());
}

#[test]
fn pressing_again_while_a_stroke_is_down_counts_the_stroke_it_ends() {
    let mut state = saved_painting();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    assert!(state.press(EaselId::Painting, sample_at(100.0, 100.0, 0.9), false));
    state.motion(EaselId::Painting, &[sample_at(140.0, 100.0, 0.9)]);
    assert!(!state.has_unsaved_edits());
    assert!(state.press(EaselId::Painting, sample_at(200.0, 100.0, 0.9), false));
    assert!(state.has_unsaved_edits());
}
