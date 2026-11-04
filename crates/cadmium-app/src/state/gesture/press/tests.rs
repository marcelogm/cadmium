use super::*;
use crate::state::test_support::{
    painting_state, present_one_tray_frame, sample_at, squeeze_blob_on_tray,
};
use crate::tools::UiTool;

#[test]
fn the_eraser_wipes_the_tray_with_the_rag() {
    let mut state = painting_state();
    squeeze_blob_on_tray(&mut state, 0);
    let before = state.session.tray.wet_totals(&state.gpu).unwrap().volume;
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Tray, sample_at(280.0, 300.0, 1.0), true);
    state.motion(&[sample_at(320.0, 300.0, 1.0)]);
    state.release();
    present_one_tray_frame(&mut state);
    assert!(state.session.tray.wet_totals(&state.gpu).unwrap().volume < before);
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
