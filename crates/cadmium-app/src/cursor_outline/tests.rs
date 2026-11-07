use cadmium_engine::surface::Surface;

use super::CursorOutline;
use crate::state::test_support::{TEST_BRUSH_DIAMETER_PX, painting_state};
use crate::tools::TrayTool;

const KNIFE_SIZE_PX: f32 = 120.0;
const TO_WIDGET: f32 = 2.0;

#[test]
fn the_tray_outline_is_as_long_as_the_mixing_knife_not_the_brush() {
    let mut state = painting_state();
    state.set_tray_tool(TrayTool::MixingKnife);
    state.set_focused_size(KNIFE_SIZE_PX);
    assert_ne!(KNIFE_SIZE_PX, TEST_BRUSH_DIAMETER_PX);
    let outline = CursorOutline::of_brush(&state, Surface::Tray, TO_WIDGET);
    assert_eq!(outline.radius(), KNIFE_SIZE_PX / 2.0 * TO_WIDGET);
}
