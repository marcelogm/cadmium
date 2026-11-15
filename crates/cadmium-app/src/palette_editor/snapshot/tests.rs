use super::*;
use crate::state::ColourClick;
use crate::state::test_support::painting_state;

#[test]
fn only_the_sheet_highlights_the_colour_its_tool_holds() {
    let mut state = painting_state();
    state.begin_palette_edit();
    state.set_palette_mode(PaletteMode::Mixing);
    state.load_sheet_colour(ColourRef::Paint(1), ColourClick::Load);
    assert_eq!(Snapshot::of(&state).highlighted, Some(ColourRef::Paint(1)));
    state.set_palette_mode(PaletteMode::Circle);
    assert_eq!(Snapshot::of(&state).highlighted, None);
}
