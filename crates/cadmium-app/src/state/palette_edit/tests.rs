use super::*;
use crate::state::test_support::{
    PAINT_ROW_Y, painting_state, row_samples, sample_at, stroke, stroke_on,
};
use crate::state::{ColourClick, EaselId};
use crate::tools::{Press, UiTool};
use cadmium_engine::palette::{ColourRef, MAX_DARKEN};
use cadmium_engine::stroke::Tool;

const SHEET_ROW_Y: f64 = 200.0;

fn mixing_state() -> AppState {
    let mut state = painting_state();
    state.begin_palette_edit();
    state.set_palette_mode(PaletteMode::Mixing);
    state
}

fn paint_on_the_sheet(state: &mut AppState, slot: usize) {
    state.load_sheet_colour(ColourRef::Paint(slot), ColourClick::Load);
    stroke_on(state, EaselId::Sheet, &row_samples(SHEET_ROW_Y));
    state.sheet.session.flush(&state.gpu);
}

#[test]
fn an_armed_pick_takes_the_colour_under_the_pen_instead_of_painting() {
    let mut state = mixing_state();
    paint_on_the_sheet(&mut state, 1);
    let painted = state.sheet.session.canvas.wet_totals(&state.gpu).unwrap();
    assert!(painted.volume > 0.0);
    let held = state.sheet.session.brush.recipe.clone();
    state.arm_sheet_pick();
    assert_eq!(state.press_for(EaselId::Sheet), Press::PickColour);
    assert!(!state.press(EaselId::Sheet, sample_at(200.0, SHEET_ROW_Y, 0.8), false));
    assert!(!state.is_stroking());
    assert!(!state.palette_edit.pick_armed);
    let chosen = state.palette_edit.chosen.expect("paint lies under the pen");
    assert!(chosen[1] > 0.99, "{chosen:?}");
    assert_eq!(state.sheet.session.brush.recipe, held);
    assert_eq!(
        state.sheet.session.canvas.wet_totals(&state.gpu).unwrap(),
        painted
    );
    assert_eq!(state.press_for(EaselId::Sheet), Press::Stroke(Tool::Brush));
}

#[test]
fn picking_where_the_sheet_is_bare_chooses_nothing_and_disarms() {
    let mut state = painting_state();
    state.begin_palette_edit();
    state.arm_sheet_pick();
    assert!(!state.pick_colour(EaselId::Sheet, 50.0, 50.0));
    assert_eq!(
        state.palette_edit,
        PaletteEdit {
            mode: PaletteMode::Circle,
            ..PaletteEdit::default()
        }
    );
}

#[test]
fn pinning_moves_the_chosen_colour_into_my_colours() {
    let mut state = painting_state();
    state.begin_palette_edit();
    assert_eq!(state.pin_chosen(), None);
    state.painting.session.palette.circle = vec![ColourRef::Paint(0), ColourRef::Paint(1)];
    state.choose_on_circle(0.25, 1.0);
    let mark = state.edit_mark();
    assert_eq!(state.pin_chosen(), Some(0));
    assert_eq!(
        state.painting.session.palette.my_colours[0][..2],
        [0.5, 0.5]
    );
    assert_eq!(state.palette_edit.chosen, None);
    assert_ne!(state.edit_mark(), mark);
}

#[test]
fn switching_mode_forgets_the_chosen_colour_and_an_armed_pick() {
    let mut state = painting_state();
    state.begin_palette_edit();
    state.choose_on_circle(0.0, 1.0);
    assert!(state.palette_edit.chosen.is_some());
    state.arm_sheet_pick();
    state.set_palette_mode(PaletteMode::Mixing);
    assert_eq!(state.palette_edit.chosen, None);
    assert!(!state.palette_edit.pick_armed);
}

#[test]
fn dropping_and_dragging_colours_arranges_the_circle() {
    let mut state = painting_state();
    state.painting.session.palette.circle = vec![ColourRef::Paint(0), ColourRef::Paint(1)];
    state.drop_on_circle(ColourRef::Paint(2), 0.5);
    assert_eq!(
        state.painting.session.palette.circle,
        [
            ColourRef::Paint(0),
            ColourRef::Paint(2),
            ColourRef::Paint(1)
        ]
    );
    assert_eq!(state.drag_circle_colour(0, 0.66), 2);
    assert_eq!(
        state.painting.session.palette.circle,
        [
            ColourRef::Paint(2),
            ColourRef::Paint(1),
            ColourRef::Paint(0)
        ]
    );
    state.remove_circle_colour(1);
    assert_eq!(
        state.painting.session.palette.circle,
        [ColourRef::Paint(2), ColourRef::Paint(0)]
    );
}

#[test]
fn the_sheet_paints_with_the_paintings_brush_and_its_own_load() {
    let mut state = painting_state();
    state.painting.session.brush.geometry.max_diameter_px = 64.0;
    state.click_colour(ColourRef::Paint(0), ColourClick::Load);
    state.begin_palette_edit();
    assert_eq!(state.sheet.session.brush.geometry.max_diameter_px, 64.0);
    assert!(state.sheet.brush_is_clean());
    assert!(!state.painting.brush_is_clean());
}

#[test]
fn the_sheet_takes_the_brush_when_the_painting_holds_the_pencil_or_the_move_tool() {
    let mut state = painting_state();
    for tool in [UiTool::Move, UiTool::Paint(Tool::Pencil)] {
        state.set_ui_tool(tool);
        state.begin_palette_edit();
        assert_eq!(state.sheet.ui_tool, UiTool::Paint(Tool::Brush), "{tool:?}");
        assert_eq!(state.painting.ui_tool, tool);
        state.end_palette_edit();
    }
    state.set_ui_tool(UiTool::Paint(Tool::Knife));
    state.begin_palette_edit();
    assert_eq!(state.sheet.ui_tool, UiTool::Paint(Tool::Knife));
}

#[test]
fn the_pencil_and_the_move_tool_cannot_be_chosen_on_the_sheet() {
    let mut state = painting_state();
    state.begin_palette_edit();
    for tool in [UiTool::Move, UiTool::Paint(Tool::Pencil)] {
        assert!(!state.set_sheet_tool(tool), "{tool:?}");
        assert_eq!(state.sheet.ui_tool, UiTool::Paint(Tool::Brush), "{tool:?}");
    }
    assert!(state.set_sheet_tool(UiTool::Paint(Tool::Rag)));
    assert_eq!(state.sheet.ui_tool, UiTool::Paint(Tool::Rag));
    assert_eq!(state.painting.ui_tool, UiTool::Paint(Tool::Brush));
}

#[test]
fn undo_goes_to_the_sheet_only_while_mixing() {
    let mut state = painting_state();
    assert_eq!(state.history_easel(), Some(EaselId::Painting));
    state.begin_palette_edit();
    assert_eq!(state.history_easel(), None);
    state.set_palette_mode(PaletteMode::Mixing);
    assert_eq!(state.history_easel(), Some(EaselId::Sheet));
    state.end_palette_edit();
    assert_eq!(state.history_easel(), Some(EaselId::Painting));
}

#[test]
fn undoing_on_the_sheet_is_an_edit_that_leaves_the_painting_alone() {
    let mut state = painting_state();
    stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    state.painting.session.flush(&state.gpu);
    let on_painting = state
        .painting
        .session
        .canvas
        .wet_totals(&state.gpu)
        .unwrap();
    assert!(on_painting.volume > 0.0);
    state.begin_palette_edit();
    state.set_palette_mode(PaletteMode::Mixing);
    paint_on_the_sheet(&mut state, 1);
    let mark = state.edit_mark();
    assert!(state.undo(EaselId::Sheet));
    assert_eq!(
        state
            .sheet
            .session
            .canvas
            .wet_totals(&state.gpu)
            .unwrap()
            .volume,
        0.0
    );
    assert_eq!(
        state
            .painting
            .session
            .canvas
            .wet_totals(&state.gpu)
            .unwrap(),
        on_painting
    );
    assert!(state.can_undo(EaselId::Painting));
    assert_ne!(state.edit_mark(), mark);
}

#[test]
fn clearing_the_sheet_is_an_edit_only_when_there_was_paint() {
    let mut state = mixing_state();
    let mark = state.edit_mark();
    state.clear_sheet();
    assert_eq!(state.edit_mark(), mark);
    paint_on_the_sheet(&mut state, 1);
    let mark = state.edit_mark();
    state.clear_sheet();
    assert_eq!(
        state
            .sheet
            .session
            .canvas
            .wet_totals(&state.gpu)
            .unwrap()
            .volume,
        0.0
    );
    assert_ne!(state.edit_mark(), mark);
}

#[test]
fn opening_and_closing_the_palette_leaves_the_painting_unedited() {
    let mut state = painting_state();
    let mark = state.edit_mark();
    state.begin_palette_edit();
    state.end_palette_edit();
    assert_eq!(state.edit_mark(), mark);
    assert!(!state.is_editing_palette());
}

#[test]
fn a_paint_added_to_the_painting_is_added_to_the_sheet_too() {
    let mut state = painting_state();
    let mut spec = state.painting.session.canvas.spec.clone();
    spec.pigment_slots.truncate(2);
    state.start_painting(spec).unwrap();
    state.begin_palette_edit();
    state.set_palette_mode(PaletteMode::Mixing);
    paint_on_the_sheet(&mut state, 1);
    let painted = state.sheet.session.canvas.wet_totals(&state.gpu).unwrap();
    let slot = state.add_pile("burnt_sienna").unwrap();
    assert_eq!(
        state.sheet.session.canvas.spec.pigment_slots,
        state.painting.session.canvas.spec.pigment_slots
    );
    assert_eq!(
        state.sheet.session.canvas.spec.pigment_slots[slot],
        "burnt_sienna"
    );
    let kept = state.sheet.session.canvas.wet_totals(&state.gpu).unwrap();
    assert!(
        (kept.volume - painted.volume).abs() < 1e-6 * painted.volume,
        "{kept:?}"
    );
}

#[test]
fn unpinning_takes_the_colour_off_my_colours_and_the_circle() {
    let mut state = painting_state();
    state.begin_palette_edit();
    state.choose_on_circle(0.0, 1.0);
    let index = state.pin_chosen().unwrap();
    state.drop_on_circle(ColourRef::Mine(index), 0.0);
    assert!(
        state
            .painting
            .session
            .palette
            .circle
            .contains(&ColourRef::Mine(index))
    );
    let mark = state.edit_mark();
    state.unpin(index);
    assert!(state.painting.session.palette.my_colours.is_empty());
    assert!(
        !state
            .painting
            .session
            .palette
            .circle
            .contains(&ColourRef::Mine(index))
    );
    assert_ne!(state.edit_mark(), mark);
    let mark = state.edit_mark();
    state.unpin(index);
    assert_eq!(state.edit_mark(), mark);
}

#[test]
fn the_dialogs_darkening_stays_within_bounds() {
    let mut state = painting_state();
    state.begin_palette_edit();
    state.set_palette_darken(2.0);
    assert_eq!(state.palette_edit.darken, MAX_DARKEN);
    state.set_palette_darken(f32::NAN);
    assert_eq!(state.palette_edit.darken, MAX_DARKEN);
    state.set_palette_darken(-1.0);
    assert_eq!(state.palette_edit.darken, 0.0);
    assert_eq!(state.circle_darken(), 0.0);
}

#[test]
fn closing_the_palette_mid_stroke_ends_the_stroke_on_the_sheet() {
    let mut state = mixing_state();
    state.load_sheet_colour(ColourRef::Paint(1), ColourClick::Load);
    let samples = row_samples(SHEET_ROW_Y);
    assert!(state.press(EaselId::Sheet, samples[0], false));
    state.motion(EaselId::Sheet, &samples[1..]);
    assert!(state.is_stroking());
    let mark = state.edit_mark();
    state.end_palette_edit();
    assert!(!state.is_stroking());
    assert!(!state.sheet.is_stroking());
    assert_ne!(state.edit_mark(), mark);
}
