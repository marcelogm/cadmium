use cadmium_engine::brush::BrushShape;

use super::*;
use crate::state::test_support::{TEST_BRUSH_DIAMETER_PX, painting_state};

#[test]
fn the_tube_shows_its_nozzle_whatever_brush_was_last_held() {
    let mut state = painting_state();
    state.session.brush.shape = BrushShape::Fan;
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.set_ui_tool(UiTool::Tube);
    let outline = CursorOutline::of_tool(&state, 1.0).unwrap();
    assert!(matches!(
        outline.shape,
        OutlineShape::Ellipse { semi_axes, .. } if semi_axes == [TEST_BRUSH_DIAMETER_PX / 2.0; 2]
    ));
}

#[test]
fn moving_the_view_shows_no_outline() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Move);
    assert!(CursorOutline::of_tool(&state, 1.0).is_none());
}

#[test]
fn a_brush_shows_the_outline_of_its_bristles() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    let outline = CursorOutline::of_tool(&state, 1.0).unwrap();
    assert!(matches!(outline.shape, OutlineShape::Polygon(_)));
}
