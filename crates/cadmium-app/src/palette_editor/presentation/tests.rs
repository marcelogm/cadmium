use super::*;
use cadmium_engine::palette::paint_recipe;
use cadmium_engine::stroke::Tool;

#[test]
fn the_hint_says_what_ends_the_flow_in_each_state() {
    let circle = PaletteEdit::default();
    let mixing = PaletteEdit {
        mode: PaletteMode::Mixing,
        ..PaletteEdit::default()
    };
    let armed = PaletteEdit {
        pick_armed: true,
        ..mixing.clone()
    };
    let chosen = PaletteEdit {
        chosen: Some(paint_recipe(0)),
        ..mixing.clone()
    };
    assert_eq!(chosen_hint(&circle), "Click a point on the circle.");
    assert_eq!(
        chosen_hint(&mixing),
        "Mix on the sheet, then pick a colour from it."
    );
    assert_eq!(chosen_hint(&armed), "Click the paint on the sheet.");
    assert_eq!(chosen_hint(&chosen), "Ready to pin.");
}

#[test]
fn pinning_waits_for_a_chosen_colour_and_markers_show_only_with_the_circle() {
    assert!(!pin_button_is_enabled(&PaletteEdit::default()));
    assert!(pin_button_is_enabled(&PaletteEdit {
        chosen: Some(paint_recipe(0)),
        ..PaletteEdit::default()
    }));
    assert!(shows_circle_markers(PaletteMode::Circle));
    assert!(!shows_circle_markers(PaletteMode::Mixing));
}

#[test]
fn tool_keys_choose_the_sheets_tools_and_swallow_the_pencil_and_move() {
    let knife = UiTool::Paint(Tool::Knife);
    assert_eq!(tool_key(Some(knife)), ToolKey::Choose(knife));
    assert_eq!(
        tool_key(Some(UiTool::Paint(Tool::Pencil))),
        ToolKey::Swallow
    );
    assert_eq!(tool_key(Some(UiTool::Move)), ToolKey::Swallow);
    assert_eq!(tool_key(None), ToolKey::NotATool);
}

#[test]
fn each_mode_is_found_again_by_its_name() {
    for (mode, _) in MODES {
        assert_eq!(mode_named(mode_name(mode)), Some(mode));
    }
    assert_eq!(mode_named("wheel"), None);
}
