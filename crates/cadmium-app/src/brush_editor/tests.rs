use cadmium_engine::brush::{BrushSpec, factory_brushes};
use cadmium_engine::canvas::CanvasError;
use cadmium_engine::gpu::GpuLost;
use cadmium_studio::{DemoStroke, SLOW_MOTION, StudioError};

use super::{
    CUSTOM, ToolbarAction, apply, failure_toast, stroke_index, stroke_labels, summary_rows,
};
use crate::state::AppState;
use crate::state::test_support::painting_state;

fn open_state() -> AppState {
    let mut state = painting_state();
    state.open_studio();
    assert!(state.studio_open());
    state
}

#[test]
fn play_and_pause_drive_the_player() {
    let mut state = open_state();
    apply(&mut state, ToolbarAction::Play(false));
    assert!(!state.studio().unwrap().player.is_playing());
    apply(&mut state, ToolbarAction::Play(true));
    assert!(state.studio().unwrap().player.is_playing());
}

#[test]
fn slow_motion_sets_the_slow_speed_and_back() {
    let mut state = open_state();
    apply(&mut state, ToolbarAction::SlowMotion(true));
    assert_eq!(state.studio().unwrap().player.speed(), SLOW_MOTION);
    apply(&mut state, ToolbarAction::SlowMotion(false));
    assert_eq!(state.studio().unwrap().player.speed(), 1.0);
}

#[test]
fn the_stroke_dropdown_index_picks_that_demo_stroke() {
    let mut state = open_state();
    for (index, stroke) in DemoStroke::ALL.into_iter().enumerate().rev() {
        apply(&mut state, ToolbarAction::Stroke(index));
        assert_eq!(state.studio().unwrap().player.stroke(), stroke);
    }
}

#[test]
fn an_unknown_stroke_index_keeps_the_stroke() {
    let mut state = open_state();
    let before = state.studio().unwrap().player.stroke();
    apply(&mut state, ToolbarAction::Stroke(DemoStroke::ALL.len()));
    assert_eq!(state.studio().unwrap().player.stroke(), before);
}

#[test]
fn reset_view_restores_the_reset_camera() {
    let mut state = open_state();
    let reset = state.studio().unwrap().camera;
    let camera = &mut state.studio_mut().unwrap().camera;
    camera.orbit(40.0, -25.0);
    camera.zoom(2.0);
    assert_ne!(*camera, reset);
    apply(&mut state, ToolbarAction::ResetView);
    assert_eq!(state.studio().unwrap().camera, reset);
}

#[test]
fn actions_on_a_closed_studio_do_nothing() {
    let mut state = painting_state();
    apply(&mut state, ToolbarAction::Play(false));
    apply(&mut state, ToolbarAction::SlowMotion(true));
    apply(&mut state, ToolbarAction::Stroke(1));
    apply(&mut state, ToolbarAction::ResetView);
    assert!(!state.studio_open());
}

#[test]
fn the_dropdown_lists_every_demo_stroke_in_order() {
    let labels: Vec<_> = DemoStroke::ALL.into_iter().map(DemoStroke::label).collect();
    assert_eq!(stroke_labels(), labels);
    for (index, stroke) in DemoStroke::ALL.into_iter().enumerate() {
        assert_eq!(stroke_index(stroke), index);
    }
}

#[test]
fn the_summary_names_a_factory_brush() {
    let brush = factory_brushes().into_iter().next().unwrap();
    let rows = summary_rows(&brush);
    let head = cadmium_studio::head_name(&brush.head).unwrap();
    let hair = cadmium_studio::hair_name(&brush.hair).unwrap();
    assert_eq!(rows[0], ("Name", brush.name.clone()));
    assert_eq!(rows[1], ("Head", head.to_owned()));
    assert_eq!(rows[2], ("Hair", hair.to_owned()));
    assert_eq!(rows[3], ("Size", format!("{:.0} px", brush.size_px)));
}

#[test]
fn the_summary_calls_changed_parts_custom() {
    let mut brush: BrushSpec = factory_brushes().into_iter().next().unwrap();
    brush.head.thickness += 0.123;
    brush.hair.stiffness += 0.123;
    brush.size_px = 23.6;
    let rows = summary_rows(&brush);
    assert_eq!(rows[1].1, CUSTOM);
    assert_eq!(rows[2].1, CUSTOM);
    assert_eq!(rows[3].1, "24 px");
}

#[test]
fn a_studio_error_is_toasted() {
    let error = StudioError::Canvas(CanvasError::OutOfVideoMemory);
    let toast = failure_toast(Some(&error), false).unwrap();
    assert_eq!(toast, format!("Can't show the brush studio: {error}"));
}

#[test]
fn a_lost_gpu_is_left_to_the_gpu_lost_dialog() {
    let canvas = StudioError::Canvas(CanvasError::OutOfVideoMemory);
    assert_eq!(failure_toast(Some(&canvas), true), None);
    assert_eq!(failure_toast(Some(&StudioError::Gpu(GpuLost)), false), None);
    assert_eq!(failure_toast(None, false), None);
}
