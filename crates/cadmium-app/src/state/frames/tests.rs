use crate::state::test_support::{
    PAINT_ROW_Y, RAG_ROW_Y, painting_state, present_one_frame, present_one_tray_frame, row_samples,
    sample_at, stroke, wet_volume,
};
use crate::tools::UiTool;
use cadmium_engine::stroke::Tool;
use cadmium_engine::surface::Surface;

#[test]
fn a_stroke_still_queued_when_the_eraser_comes_down_is_painted_with_its_own_brush() {
    let mut state = painting_state();
    stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    stroke(&mut state, Tool::Rag, &row_samples(RAG_ROW_Y));
    present_one_frame(&mut state);
    assert!(wet_volume(&state) > 0.0);
    assert_eq!(state.session.brush.tool, Tool::Brush);
}

#[test]
fn an_eraser_stroke_on_the_tray_puts_the_rag_down_after_the_tray_frame() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), true);
    state.motion(&[sample_at(340.0, 300.0, 0.9)]);
    assert_eq!(state.session.brush.tool, Tool::Rag);
    state.release();
    present_one_tray_frame(&mut state);
    assert_eq!(state.session.brush.tool, Tool::Brush);
}

#[test]
fn tray_frames_report_the_pen_samples_they_drew() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Blender));
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    state.motion(&[sample_at(320.0, 300.0, 0.9), sample_at(340.0, 300.0, 0.9)]);
    present_one_tray_frame(&mut state);
    assert_eq!(state.pen.samples_last_frame, 3);
    state.release();
}
