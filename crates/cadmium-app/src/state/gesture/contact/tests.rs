use cadmium_engine::stroke::Tool;

use crate::state::EaselId;
use crate::state::test_support::{painting_state, sample_at};
use crate::tools::UiTool;

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
