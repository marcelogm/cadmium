use crate::state::AppState;
use crate::state::test_support::{
    PAINT_ROW_Y, RAG_ROW_Y, ScriptedClock, painting_state, present_one_frame,
    present_one_tray_frame, row_samples, sample_at, scripted_painting_state, stroke, wet_volume,
};
use crate::tools::UiTool;
use cadmium_engine::engine::BRISTLE_SETTLE_MS;
use cadmium_engine::stroke::Tool;
use cadmium_engine::surface::Surface;

const FRAME_INTERVAL_MS: f64 = 16.0;
const CLOCK_START_MS: f64 = 73_000.0;
const SETTLE_FRAME_LIMIT: u32 = 200;

fn frames_until_settled(state: &mut AppState, clock: &ScriptedClock) -> u32 {
    let mut frames = 0;
    while state.session.bristles_settling(state.now_ms()) {
        clock.advance(FRAME_INTERVAL_MS);
        present_one_frame(state);
        frames += 1;
        assert!(frames < SETTLE_FRAME_LIMIT, "the bristles never settled");
    }
    frames
}

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

#[test]
fn frames_keep_coming_while_the_bristles_settle_after_a_stroke() {
    let clock = ScriptedClock::starting_at(CLOCK_START_MS);
    let mut state = scripted_painting_state(&clock);
    stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    present_one_frame(&mut state);
    assert!(!state.session.has_pending_work());
    let settle_frames = f64::from(BRISTLE_SETTLE_MS) / FRAME_INTERVAL_MS;
    let frames = f64::from(frames_until_settled(&mut state, &clock));
    assert!(
        frames >= settle_frames,
        "{frames} frames, {settle_frames} to settle"
    );
    assert!(
        frames <= settle_frames + 2.0,
        "{frames} frames, {settle_frames} to settle"
    );
}

#[test]
fn a_rag_stroke_after_the_brush_asks_for_no_settling_frames() {
    let clock = ScriptedClock::starting_at(CLOCK_START_MS);
    let mut state = scripted_painting_state(&clock);
    stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    stroke(&mut state, Tool::Rag, &row_samples(RAG_ROW_Y));
    present_one_frame(&mut state);
    assert!(!state.session.bristles_settling(state.now_ms()));
}
