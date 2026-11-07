use cadmium_engine::canvas::StateTexture;
use cadmium_engine::stroke::Tool;
use cadmium_engine::surface::Surface;

use crate::state::AppState;
use crate::state::test_support::{
    PAINT_ROW_Y, TEST_BRUSH_DIAMETER_PX, brush_totals, finish_painting, painting_state,
    present_one_frame, present_one_tray_frame, row_samples, sample_at, squeeze_blob_on_tray,
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

#[test]
fn mid_stroke_the_brush_still_has_its_own_size() {
    let mut state = painting_state();
    state.set_tray_tool(TrayTool::MixingKnife);
    state.press(
        Surface::Tray,
        sample_at(240.0, 300.0, UPRIGHT_PRESSURE),
        false,
    );
    assert_eq!(state.session.brush.geometry.max_diameter_px, 80.0);
    assert_eq!(state.stroke_diameter_px(Tool::MixingKnife), 80.0);
    assert_eq!(
        state.stroke_diameter_px(Tool::Brush),
        TEST_BRUSH_DIAMETER_PX
    );
    assert_eq!(state.brush_diameter_px(), TEST_BRUSH_DIAMETER_PX);
    assert_eq!(state.worn_brush_spec().size_px, TEST_BRUSH_DIAMETER_PX);
    state.release();
    assert_eq!(
        state.stroke_diameter_px(Tool::Brush),
        TEST_BRUSH_DIAMETER_PX
    );
}

#[test]
fn the_mixing_knife_size_is_kept_in_the_settings() {
    let mut state = painting_state();
    state.set_tray_tool(TrayTool::MixingKnife);
    state.set_focused_size(KNIFE_SIZE_PX);
    assert_eq!(state.settings.get().mixing_knife_diameter_px, KNIFE_SIZE_PX);
    state.set_focused_size(f32::INFINITY);
    assert_eq!(state.mixing_knife_diameter_px(), 80.0);
    state.set_focused_size(1.0e6);
    assert_eq!(state.mixing_knife_diameter_px(), 500.0);
}

const BLOB_SIZE_PX: f32 = 120.0;
const OFF_CENTRE_ROW: usize = 330;

fn present_both(state: &mut AppState) {
    present_one_frame(state);
    present_one_tray_frame(state);
}

fn tray_row_volume(state: &AppState, y: usize, xs: std::ops::Range<usize>) -> f32 {
    let width = state.session.tray.spec.width as usize;
    let wet = state
        .session
        .tray
        .read_f32(&state.gpu, StateTexture::WetVol)
        .unwrap();
    xs.map(|x| wet[y * width + x]).sum()
}

/// A brush stroke queued behind a mixing knife stroke, both still waiting for frames:
/// the knife stroke scrapes with its own long blade, and the brush comes back at its size.
#[test]
fn a_brush_stroke_queued_behind_the_mixing_knife_keeps_each_its_size() {
    let mut state = painting_state();
    let brush_reserve = state.session.engine.full_reserve_volume();
    state.set_brush_size(BLOB_SIZE_PX);
    squeeze_blob_on_tray(&mut state, 0);
    state.set_brush_size(TEST_BRUSH_DIAMETER_PX);
    finish_painting(&mut state, present_both);
    let before = tray_row_volume(&state, OFF_CENTRE_ROW, 280..330);
    assert!(before > 0.0);
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
    assert!(state.session.has_queued_dabs());
    let row = row_samples(PAINT_ROW_Y);
    state.begin_stroke(Tool::Brush, row[0]);
    state.add_samples(&row[1..]);
    state.release();
    finish_painting(&mut state, present_both);
    let after = tray_row_volume(&state, OFF_CENTRE_ROW, 280..330);
    assert!(after < before, "{after} {before}");
    assert_eq!(state.session.engine.full_reserve_volume(), brush_reserve);
    assert_eq!(
        state.session.brush.geometry.max_diameter_px,
        TEST_BRUSH_DIAMETER_PX
    );
}
