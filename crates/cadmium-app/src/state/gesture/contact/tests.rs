use cadmium_engine::stroke::Tool;

use super::*;
use crate::state::test_support::{painting_state, sample_at};
use crate::tools::UiTool;

#[test]
fn a_contact_is_on_the_painting_from_the_press_to_the_release() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    assert!(!state.contact_on(Surface::Painting));
    state.press(Surface::Painting, sample_at(100.0, 100.0, 0.9), false);
    assert!(state.contact_on(Surface::Painting));
    state.release();
    assert!(!state.contact_on(Surface::Painting));
}

#[test]
fn a_stroke_lasts_until_the_pen_lifts() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Painting, sample_at(100.0, 100.0, 0.9), false);
    assert!(state.is_stroking());
    state.release();
    assert!(!state.is_stroking());
}
