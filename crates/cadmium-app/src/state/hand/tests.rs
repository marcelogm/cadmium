use crate::settings::SettingsStore;
use crate::state::test_support::{
    PAINT_ROW_Y, TEST_BRUSH_DIAMETER_PX, brush_totals, painting_state, present_one_frame,
    row_samples, stroke, wet_volume,
};
use crate::tools::UiTool;
use cadmium_engine::brush::factory_brushes;
use cadmium_engine::stroke::Tool;

#[test]
fn tool_chosen_mid_stroke_waits_for_the_stroke_to_end() {
    let mut state = painting_state();
    let samples = row_samples(PAINT_ROW_Y);
    state.begin_stroke(Tool::Brush, samples[0]);
    state.set_ui_tool(UiTool::Paint(Tool::Rag));
    state.add_samples(&samples[1..]);
    assert_eq!(state.session.brush.tool, Tool::Brush);
    state.release();
    present_one_frame(&mut state);
    assert!(wet_volume(&state) > 0.0);
    assert_eq!(state.session.brush.tool, Tool::Rag);
}

#[test]
fn resizing_the_brush_keeps_the_paint_on_its_tip() {
    let mut state = painting_state();
    stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    let before = brush_totals(&state);
    state.set_brush_size(TEST_BRUSH_DIAMETER_PX * 2.0);
    state.wear_brush(&factory_brushes()[3]);
    assert_eq!(brush_totals(&state), before);
}

#[test]
fn wearing_a_brush_takes_its_size_head_and_hair() {
    let mut state = painting_state();
    let mop = factory_brushes()
        .into_iter()
        .find(|brush| brush.name.starts_with("Mop"))
        .unwrap();
    state.wear_brush(&mop);
    assert_eq!(state.session.brush.head, mop.head);
    assert_eq!(state.session.brush.hair, mop.hair);
    assert_eq!(state.session.brush.geometry.max_diameter_px, mop.size_px);
}

#[test]
fn choosing_the_hardness_sharpens_the_pencil_and_remembers_it() {
    let mut state = painting_state();
    state.set_pencil_hardness(0.8);
    assert_eq!(state.session.brush.pencil_hardness, 0.8);
    assert_eq!(state.settings.get().pencil_hardness, 0.8);
    state.set_pencil_hardness(3.0);
    assert_eq!(state.session.brush.pencil_hardness, 1.0);
}

#[test]
fn the_remembered_hardness_is_taken_up_with_the_settings() {
    let mut state = painting_state();
    let mut store = SettingsStore::default();
    store
        .update(|settings| settings.pencil_hardness = 0.2)
        .unwrap();
    state.use_settings(store);
    assert_eq!(state.session.brush.pencil_hardness, 0.2);
    assert_eq!(state.settings.get().pencil_hardness, 0.2);
}

#[test]
fn load_and_medium_wait_for_the_next_dip() {
    let mut state = painting_state();
    let before = brush_totals(&state);
    state.set_load(0.2);
    state.set_medium(0.5);
    assert_eq!(brush_totals(&state), before);
}
