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
    assert!(state.press(Surface::Painting, sample_at(100.0, PAINT_ROW_Y, 1.0), true));
    wipe_the_painted_row(&mut state);
    assert!(wet_volume(&state) < before);
}

#[test]
fn the_move_tool_and_the_eyedropper_do_not_touch_down() {
    let mut state = painting_state();
    for tool in [UiTool::Move, UiTool::Paint(Tool::Eyedropper)] {
        state.set_ui_tool(tool);
        assert!(!state.press(Surface::Painting, sample_at(100.0, 100.0, 1.0), false));
        assert!(!state.is_stroking());
    }
}

#[test]
fn the_pencil_starts_a_pencil_stroke_on_the_painting() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Pencil));
    assert!(state.press(Surface::Painting, sample_at(100.0, 100.0, 1.0), false));
    assert!(state.contact_on(Surface::Painting));
    assert_eq!(state.session.brush.tool, Tool::Pencil);
}

#[test]
fn the_eraser_end_wipes_the_painting_while_the_pencil_is_chosen() {
    let mut state = painting_state();
    paint_stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    let before = wet_volume(&state);
    state.set_ui_tool(UiTool::Paint(Tool::Pencil));
    assert!(state.press(Surface::Painting, sample_at(100.0, PAINT_ROW_Y, 1.0), true));
    assert_eq!(state.session.brush.tool, Tool::Rag);
    wipe_the_painted_row(&mut state);
    assert!(wet_volume(&state) < before);
}
