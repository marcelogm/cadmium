use super::*;
use crate::state::test_support::{
    PAINT_ROW_Y, finish_painting, paint_stroke, painting_state, present_one_frame, row_samples,
    sample_at, wet_volume,
};
use crate::tools::UiTool;

fn wipe_the_painted_row(state: &mut AppState) {
    state.motion(&[sample_at(290.0, PAINT_ROW_Y, 1.0)]);
    state.release();
    finish_painting(state, present_one_frame);
}

#[test]
fn the_eraser_wipes_the_painting_with_the_rag() {
    let mut state = painting_state();
    paint_stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    let before = wet_volume(&state);
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    assert!(state.press(sample_at(100.0, PAINT_ROW_Y, 1.0), true));
    wipe_the_painted_row(&mut state);
    assert!(wet_volume(&state) < before);
}

#[test]
fn the_move_tool_and_the_eyedropper_tip_do_not_touch_down() {
    for tool in [UiTool::Move, UiTool::Paint(Tool::Eyedropper)] {
        let mut state = painting_state();
        state.set_ui_tool(tool);
        assert!(
            !state.press(sample_at(100.0, 100.0, 0.8), false),
            "{tool:?}"
        );
        assert!(!state.is_stroking());
    }
}

#[test]
fn the_pencil_starts_a_pencil_stroke_on_the_painting() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Pencil));
    assert!(state.press(sample_at(100.0, 100.0, 1.0), false));
    assert!(state.is_stroking());
    assert_eq!(state.session.brush.tool, Tool::Pencil);
}

#[test]
fn the_eraser_end_wipes_whatever_the_tool() {
    for tool in [Tool::Brush, Tool::Pencil, Tool::Eyedropper] {
        let mut state = painting_state();
        state.set_ui_tool(UiTool::Paint(tool));
        assert!(state.press(sample_at(100.0, 100.0, 0.8), true), "{tool:?}");
        assert_eq!(state.session.brush.tool, Tool::Rag, "{tool:?}");
    }
}
