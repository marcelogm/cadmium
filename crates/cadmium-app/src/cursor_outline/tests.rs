use cadmium_engine::stroke::Tool;
use cadmium_engine::surface::Surface;

use super::CursorOutline;
use crate::state::test_support::{TEST_BRUSH_DIAMETER_PX, painting_state};
use crate::tools::UiTool;

const KNIFE_SIZE_PX: f32 = 120.0;
const TO_WIDGET: f32 = 2.0;

#[test]
fn the_knife_outline_is_as_long_as_its_blade() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Knife));
    state.set_brush_size(KNIFE_SIZE_PX);
    assert_ne!(KNIFE_SIZE_PX, TEST_BRUSH_DIAMETER_PX);
    let outline = CursorOutline::of_brush(&state, Surface::Painting, TO_WIDGET);
    assert_eq!(outline.radius(), KNIFE_SIZE_PX / 2.0 * TO_WIDGET);
}
