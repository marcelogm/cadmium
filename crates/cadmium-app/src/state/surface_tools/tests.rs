use cadmium_engine::engine::Tip;
use cadmium_engine::stroke::Tool;
use cadmium_engine::surface::Surface;

use super::tip_for;
use crate::state::test_support::{
    painting_state, present_one_tray_frame, sample_at, squeeze_blob_on_tray,
};
use crate::state::{AppState, PigmentClick};
use crate::tools::panel::PanelTool;
use crate::tools::{Press, TrayTool, UiTool};

#[test]
fn each_surface_answers_with_its_own_tool() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.set_tray_tool(TrayTool::Blender);
    assert_eq!(
        state.press_for(Surface::Painting),
        Press::Stroke(Tool::Brush)
    );
    assert_eq!(state.press_for(Surface::Tray), Press::Stroke(Tool::Blender));
}

#[test]
fn load_on_the_tray_loads_the_painting_tool() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Knife));
    state.set_tray_tool(TrayTool::Load);
    assert_eq!(
        state.press_for(Surface::Tray),
        Press::LoadFromPile(Tool::Knife)
    );
}

#[test]
fn the_side_panel_follows_the_tool_chosen_last() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.set_tray_tool(TrayTool::Tube);
    assert_eq!(state.focused_tool(), PanelTool::Tube);
    state.set_ui_tool(UiTool::Paint(Tool::Knife));
    assert_eq!(state.focused_tool(), PanelTool::Paint(Tool::Knife));
    state.set_tray_tool(TrayTool::Load);
    assert_eq!(state.focused_tool(), PanelTool::Paint(Tool::Knife));
}

#[test]
fn a_pile_click_picks_the_tube_while_the_tray_tube_is_focused() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.set_tray_tool(TrayTool::Tube);
    let recipe_before = state.session.brush.recipe.clone();
    state.click_pile(1, PigmentClick::CleanAndLoad);
    assert_eq!(state.tube_slot, 1);
    assert_eq!(state.session.brush.recipe, recipe_before);
}

#[test]
fn choosing_a_tray_tool_keeps_the_brush_in_hand() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Knife));
    state.set_tray_tool(TrayTool::Rag);
    assert_eq!(state.session.brush.tool, Tool::Knife);
}

#[test]
fn the_tray_rag_wipes_the_tray_and_hands_back_the_painting_tool() {
    let mut state = painting_state();
    squeeze_blob_on_tray(&mut state, 0);
    let before = state.session.tray.wet_totals(&state.gpu).unwrap().volume;
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.set_tray_tool(TrayTool::Rag);
    assert!(state.press(Surface::Tray, sample_at(280.0, 300.0, 1.0), false));
    state.motion(&[sample_at(320.0, 300.0, 1.0)]);
    state.release();
    present_one_tray_frame(&mut state);
    assert!(state.session.tray.wet_totals(&state.gpu).unwrap().volume < before);
    assert_eq!(state.session.brush.tool, Tool::Brush);
}

#[test]
fn load_does_not_touch_down_with_a_tool_that_holds_no_paint() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Blender));
    state.set_tray_tool(TrayTool::Load);
    assert!(!state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false));
    assert!(!state.is_stroking());
}

#[test]
fn a_pile_click_chooses_the_pigment_while_the_tray_loads() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.set_tray_tool(TrayTool::Load);
    let tube_before = state.tube_slot;
    state.click_pile(1, PigmentClick::Choose);
    assert_eq!(state.session.brush.recipe, [(1, 1.0)]);
    assert_eq!(state.tube_slot, tube_before);
}

#[test]
fn the_eraser_wipes_the_tray_when_the_painting_tool_cannot_load() {
    for tool in [UiTool::Move, UiTool::Paint(Tool::Eyedropper)] {
        let mut state = painting_state();
        squeeze_blob_on_tray(&mut state, 0);
        let before = state.session.tray.wet_totals(&state.gpu).unwrap().volume;
        state.set_ui_tool(tool);
        state.set_tray_tool(TrayTool::Load);
        assert_eq!(state.press_for(Surface::Tray), Press::Nothing, "{tool:?}");
        assert!(state.press(Surface::Tray, sample_at(280.0, 300.0, 1.0), true));
        state.motion(&[sample_at(320.0, 300.0, 1.0)]);
        state.release();
        present_one_tray_frame(&mut state);
        let after = state.session.tray.wet_totals(&state.gpu).unwrap().volume;
        assert!(after < before, "{tool:?}");
    }
}

#[test]
fn the_tray_s_own_tools_work_the_tray_s_tip() {
    assert_eq!(tip_for(Surface::Tray, Tool::Blender), Tip::Tray);
    assert_eq!(tip_for(Surface::Tray, Tool::Rag), Tip::Tray);
    // A load cancelled into a stroke paints with the painting brush.
    assert_eq!(tip_for(Surface::Tray, Tool::Brush), Tip::Painting);
    assert_eq!(tip_for(Surface::Tray, Tool::Knife), Tip::Painting);
    assert_eq!(tip_for(Surface::Painting, Tool::Blender), Tip::Painting);
    assert_eq!(tip_for(Surface::Painting, Tool::Rag), Tip::Painting);
}

fn blend_across_the_tray(state: &mut AppState) {
    assert!(state.press(Surface::Tray, sample_at(260.0, 300.0, 1.0), false));
    state.motion(&[sample_at(300.0, 300.0, 1.0), sample_at(340.0, 300.0, 1.0)]);
    state.release();
    present_one_tray_frame(state);
}

#[test]
fn the_tray_blender_leaves_the_painting_brush_and_keeps_its_own_mix() {
    let mut state = painting_state();
    squeeze_blob_on_tray(&mut state, 1);
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    let before = state.session.read_brush(&state.gpu).unwrap();
    state.set_tray_tool(TrayTool::Blender);
    blend_across_the_tray(&mut state);
    assert_eq!(state.session.read_brush(&state.gpu).unwrap(), before);
    // The panel's read waits for the frames to paint the stroke; this one reads now.
    state.measure_brush();
    assert!(
        !state.tray_tip_mixture.is_empty(),
        "the panel shows the blender's mix"
    );
    state.clean_brush();
    assert!(state.tray_tip_mixture.is_empty());
    assert_eq!(state.session.read_brush(&state.gpu).unwrap(), before);
}

#[test]
fn the_move_tool_s_panel_shows_the_painting_tool_after_a_tray_stroke() {
    let mut state = painting_state();
    squeeze_blob_on_tray(&mut state, 1);
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.set_ui_tool(UiTool::Move);
    state.set_tray_tool(TrayTool::Blender);
    blend_across_the_tray(&mut state);
    assert_eq!(state.session.brush.tool, Tool::Brush);
    state.set_tray_tool(TrayTool::Load);
    assert_eq!(state.focused_tool(), PanelTool::Paint(Tool::Brush));
}
