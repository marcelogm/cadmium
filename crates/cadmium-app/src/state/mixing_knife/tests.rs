use cadmium_engine::stroke::Tool;
use cadmium_engine::surface::Surface;

use crate::state::AppState;
use crate::state::test_support::{
    PAINT_ROW_Y, TEST_BRUSH_DIAMETER_PX, brush_totals, finish_painting, painting_state,
    present_one_tray_frame, row_samples, sample_at, squeeze_blob_on_tray,
};
use crate::tools::panel::PanelTool;
use crate::tools::{TrayTool, UiTool};

const UPRIGHT_PRESSURE: f32 = 0.8;
const KNIFE_SIZE_PX: f32 = 120.0;
const BRUSH_SIZE_PX: f32 = 30.0;

fn face_volume(state: &AppState) -> f64 {
    state.session.engine.face_totals(&state.gpu).unwrap().volume
}

/// Squeezes a blob on the tray and passes the mixing knife upright across it.
fn scrape_the_tray(state: &mut AppState) {
    squeeze_blob_on_tray(state, 0);
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.set_tray_tool(TrayTool::MixingKnife);
    assert!(state.press(
        Surface::Tray,
        sample_at(240.0, 300.0, UPRIGHT_PRESSURE),
        false
    ));
    let samples: Vec<_> = (1..=16)
        .map(|step| sample_at(240.0 + f64::from(step) * 8.0, 300.0, UPRIGHT_PRESSURE))
        .collect();
    state.motion(&samples);
    state.release();
    finish_painting(state, present_one_tray_frame);
}

#[test]
fn a_mixing_knife_pass_on_the_tray_scrapes_paint_onto_its_face() {
    let mut state = painting_state();
    assert_eq!(face_volume(&state), 0.0);
    assert!(state.mixing_knife_is_clean());
    scrape_the_tray(&mut state);
    assert!(face_volume(&state) > 0.0);
    assert_eq!(state.session.brush.tool, Tool::Brush);
    assert!(!state.mixing_knife_is_clean());
    assert!(state.mixing_knife_mixture().fractions[0] > 0.0);
    assert!(state.mixing_knife_level() > 0.0);
    let recipe = state.mixing_knife_recipe();
    assert_eq!(recipe.len(), 1, "{recipe:?}");
    assert_eq!(recipe[0].0, 0);
}

#[test]
fn the_mixing_knife_strokes_at_its_own_size_and_hands_the_brush_its_own_back() {
    let mut state = painting_state();
    assert_eq!(state.mixing_knife_diameter_px(), 80.0);
    state.set_tray_tool(TrayTool::MixingKnife);
    state.press(
        Surface::Tray,
        sample_at(240.0, 300.0, UPRIGHT_PRESSURE),
        false,
    );
    assert_eq!(state.session.brush.geometry.max_diameter_px, 80.0);
    state.release();
    assert_eq!(
        state.session.brush.geometry.max_diameter_px,
        TEST_BRUSH_DIAMETER_PX
    );
}

#[test]
fn the_size_slider_sizes_the_focused_tool_only() {
    let mut state = painting_state();
    state.set_tray_tool(TrayTool::MixingKnife);
    state.set_focused_size(KNIFE_SIZE_PX);
    assert_eq!(state.focused_size(), KNIFE_SIZE_PX);
    assert_eq!(state.mixing_knife_diameter_px(), KNIFE_SIZE_PX);
    assert_eq!(
        state.session.brush.geometry.max_diameter_px,
        TEST_BRUSH_DIAMETER_PX
    );
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.set_focused_size(BRUSH_SIZE_PX);
    assert_eq!(state.focused_size(), BRUSH_SIZE_PX);
    assert_eq!(state.session.brush.geometry.max_diameter_px, BRUSH_SIZE_PX);
    assert_eq!(state.mixing_knife_diameter_px(), KNIFE_SIZE_PX);
}

#[test]
fn resizing_the_mixing_knife_mid_stroke_keeps_the_brush_size() {
    let mut state = painting_state();
    state.set_tray_tool(TrayTool::MixingKnife);
    state.press(
        Surface::Tray,
        sample_at(240.0, 300.0, UPRIGHT_PRESSURE),
        false,
    );
    state.set_focused_size(KNIFE_SIZE_PX);
    assert!(!state.is_stroking());
    assert_eq!(state.mixing_knife_diameter_px(), KNIFE_SIZE_PX);
    assert_eq!(
        state.session.brush.geometry.max_diameter_px,
        TEST_BRUSH_DIAMETER_PX
    );
}

#[test]
fn cleaning_with_the_mixing_knife_focused_cleans_its_face_and_not_the_brush() {
    let mut state = painting_state();
    scrape_the_tray(&mut state);
    let brush = brush_totals(&state);
    assert!(brush.volume > 0.0);
    assert_eq!(state.focused_tool(), PanelTool::Paint(Tool::MixingKnife));
    state.clean_brush();
    finish_painting(&mut state, present_one_tray_frame);
    assert_eq!(face_volume(&state), 0.0);
    assert!(state.mixing_knife_is_clean());
    assert!(state.mixing_knife_recipe().is_empty());
    assert_eq!(brush_totals(&state), brush);
    assert_eq!(state.session.brush.tool, Tool::Brush);
}

#[test]
fn cleaning_the_brush_leaves_the_face_alone() {
    let mut state = painting_state();
    scrape_the_tray(&mut state);
    let face = face_volume(&state);
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.clean_brush();
    assert_eq!(face_volume(&state), face);
    assert_eq!(brush_totals(&state).volume, 0.0);
}

#[test]
fn the_face_keeps_its_mix_between_passes_even_when_cleaning_each_stroke() {
    let mut state = painting_state();
    state.clean_each_stroke = true;
    scrape_the_tray(&mut state);
    let face = face_volume(&state);
    state.press(
        Surface::Tray,
        sample_at(500.0, 100.0, UPRIGHT_PRESSURE),
        false,
    );
    assert_eq!(face_volume(&state), face);
    state.release();
}

#[test]
fn the_mixing_knife_never_strokes_the_painting() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::MixingKnife));
    assert_eq!(state.ui_tool, UiTool::Paint(Tool::Brush));
    assert_eq!(state.session.brush.tool, Tool::Brush);
    let samples = row_samples(PAINT_ROW_Y);
    state.begin_stroke(Tool::MixingKnife, samples[0]);
    assert!(!state.is_stroking());
    assert_eq!(state.session.brush.tool, Tool::Brush);
}
