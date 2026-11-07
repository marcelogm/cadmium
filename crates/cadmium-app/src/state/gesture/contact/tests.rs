use cadmium_engine::stroke::Tool;

use super::*;
use crate::state::test_support::{painting_state, sample_at};
use crate::tools::{TrayTool, UiTool};

#[test]
fn a_hold_is_on_the_surface_it_began_on() {
    let mut state = painting_state();
    state.set_tray_tool(TrayTool::Tube);
    state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
    assert!(state.holds_on(Surface::Tray));
    assert!(!state.holds_on(Surface::Painting));
    state.release();
    assert!(!state.holds_on(Surface::Tray));
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Painting, sample_at(100.0, 100.0, 0.9), false);
    assert!(!state.holds_on(Surface::Painting));
    assert!(!state.holds_on(Surface::Tray));
    state.release();
}

#[test]
fn a_contact_is_on_the_surface_it_began_on() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    assert!(state.contact_on(Surface::Tray));
    assert!(!state.contact_on(Surface::Painting));
    state.release();
    state.press(Surface::Painting, sample_at(100.0, 100.0, 0.9), false);
    assert!(state.contact_on(Surface::Painting));
    assert!(!state.contact_on(Surface::Tray));
    state.release();
    assert!(!state.contact_on(Surface::Painting));
}

#[test]
fn touching_the_tray_during_a_painting_stroke_leaves_only_the_tray_contact() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Painting, sample_at(100.0, 100.0, 0.9), false);
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    assert!(state.contact_on(Surface::Tray));
    assert!(!state.contact_on(Surface::Painting));
    state.release();
}

#[test]
fn a_stroke_is_not_a_hold_on_any_surface() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Painting, sample_at(100.0, 100.0, 0.9), false);
    assert!(!state.holds_on(Surface::Painting));
    assert!(!state.holds_on(Surface::Tray));
    state.release();
}

#[test]
fn a_hold_counts_as_a_contact_until_the_pen_lifts() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    assert!(state.is_stroking());
    assert!(state.holds_on(Surface::Tray));
    state.release();
    assert!(!state.is_stroking());
    assert!(!state.holds_on(Surface::Tray));
}
