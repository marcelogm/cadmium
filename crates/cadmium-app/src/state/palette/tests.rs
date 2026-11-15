use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::state::test_support::{
    ULTRAMARINE_SLOT, brush_totals, knife_totals, painting_state, present_one_frame, sample_at,
};
use crate::tools::UiTool;
use cadmium_engine::palette::paint_recipe;
use cadmium_engine::stroke::Tool;

const YELLOW_SLOT: usize = 1;
const HALF_SHARE_TOLERANCE: f64 = 0.01;
const SHARE_TOLERANCE: f32 = 1e-6;

#[test]
fn clicking_a_paint_leaves_the_brush_clean_and_loaded_with_it_alone() {
    let mut state = painting_state();
    state.click_colour(ColourRef::Paint(1), ColourClick::Load);
    state.click_colour(ColourRef::Paint(0), ColourClick::Load);
    let held = brush_totals(&state);
    assert!(held.pigment[0] > 0.0);
    assert_eq!(held.pigment[1], 0.0);
    assert_eq!(state.session.brush.recipe, [(0, 1.0)]);
}

#[test]
fn clicking_one_of_my_colours_loads_its_recipe() {
    let mut state = painting_state();
    let mut mine = [0.0; MAX_SLOTS];
    mine[0] = 0.75;
    mine[YELLOW_SLOT] = 0.25;
    let index = state.session.palette.pin(mine).unwrap();
    state.click_colour(ColourRef::Mine(index), ColourClick::Load);
    assert_eq!(state.session.brush.recipe, [(0, 0.75), (YELLOW_SLOT, 0.25)]);
    let held = brush_totals(&state);
    assert!(held.pigment[0] > held.pigment[YELLOW_SLOT]);
    assert!(held.pigment[YELLOW_SLOT] > 0.0);
    assert_eq!(held.pigment[ULTRAMARINE_SLOT], 0.0);
}

#[test]
fn clicking_the_circle_loads_the_mix_under_the_point() {
    let mut state = painting_state();
    state.session.palette.circle = vec![ColourRef::Paint(0), ColourRef::Paint(1)];
    state.click_circle(0.25, 1.0, ColourClick::Load);
    let held = brush_totals(&state);
    let first_share = held.pigment[0] / (held.pigment[0] + held.pigment[1]);
    assert!(
        (first_share - 0.5).abs() < HALF_SHARE_TOLERANCE,
        "{first_share}"
    );
    assert_eq!(held.pigment[ULTRAMARINE_SLOT], 0.0);
}

#[test]
fn clicking_the_circle_darkens_the_mix_by_the_darken_in_force() {
    let mut state = painting_state();
    state.session.palette.circle = vec![ColourRef::Paint(YELLOW_SLOT)];
    state.set_circle_darken(0.5);
    state.click_circle(0.0, 1.0, ColourClick::Load);
    assert_eq!(state.session.brush.recipe, [(YELLOW_SLOT, 0.5), (7, 0.5)]);
}

#[test]
fn ctrl_clicking_the_circle_with_a_clean_tool_loads_the_mix() {
    let mut state = painting_state();
    state.session.brush.recipe.clear();
    state.clean_brush();
    state.session.palette.circle = vec![ColourRef::Paint(YELLOW_SLOT)];
    state.click_circle(0.0, 1.0, ColourClick::MixIn);
    assert_eq!(state.session.brush.recipe, [(YELLOW_SLOT, 1.0)]);
    assert!(brush_totals(&state).pigment[YELLOW_SLOT] > 0.0);
}

#[test]
fn clicking_an_empty_circle_changes_nothing() {
    let mut state = painting_state();
    let mut spec = state.session.canvas.spec.clone();
    spec.pigment_slots = vec!["titanium_white".to_string(), "ivory_black".to_string()];
    state.start_painting(spec).unwrap();
    assert!(state.session.palette.circle.is_empty());
    state.press(sample_at(280.0, 100.0, 1.0), false);
    assert!(state.is_stroking());
    let recipe = state.session.brush.recipe.clone();
    let before = brush_totals(&state);
    state.click_circle(0.25, 1.0, ColourClick::Load);
    assert!(state.is_stroking());
    assert_eq!(state.session.brush.recipe, recipe);
    assert_eq!(brush_totals(&state), before);
}

#[test]
fn a_darken_that_is_not_a_number_leaves_a_circle_click_without_effect() {
    let mut state = painting_state();
    state.session.palette.circle = vec![ColourRef::Paint(YELLOW_SLOT)];
    state.circle_darken = f32::NAN;
    let recipe = state.session.brush.recipe.clone();
    let before = brush_totals(&state);
    state.click_circle(0.0, 1.0, ColourClick::Load);
    assert_eq!(state.session.brush.recipe, recipe);
    assert_eq!(brush_totals(&state), before);
}

#[test]
fn a_drag_on_the_circle_reads_the_brush_in_the_next_frame_as_a_click_at_its_end_would() {
    let points = [(0.1, 0.9), (0.4, 0.6), (0.7, 0.8)];
    let mut dragged = painting_state();
    let unread_tip = dragged.tip_mixture;
    let woken = Rc::new(Cell::new(false));
    let heard = woken.clone();
    dragged.add_waker(Rc::new(move || {
        heard.set(true);
        true
    }));
    for (turn, radius) in points {
        dragged.drag_on_circle(turn, radius, ColourClick::Load);
    }
    assert!(woken.get());
    assert!(dragged.brush_measure_due());
    assert_eq!(dragged.tip_mixture, unread_tip);
    present_one_frame(&mut dragged);
    assert!(dragged.take_brush_measured());
    let mut clicked = painting_state();
    let (turn, radius) = points[points.len() - 1];
    clicked.click_circle(turn, radius, ColourClick::Load);
    assert_eq!(dragged.session.brush.recipe, clicked.session.brush.recipe);
    assert_eq!(dragged.tip_mixture, clicked.tip_mixture);
    assert_eq!(dragged.reserve_level(), clicked.reserve_level());
    assert_eq!(dragged.reserve_mixture, clicked.reserve_mixture);
}

#[test]
fn a_circle_point_that_is_not_a_number_changes_nothing() {
    let mut state = painting_state();
    let recipe = state.session.brush.recipe.clone();
    state.click_circle(f32::NAN, 1.0, ColourClick::Load);
    state.click_circle(0.25, f32::INFINITY, ColourClick::Load);
    assert_eq!(state.session.brush.recipe, recipe);
}

#[test]
fn clicking_a_colour_with_the_pencil_in_hand_loads_the_brush() {
    let mut state = painting_state();
    state.set_ui_tool(UiTool::Paint(Tool::Pencil));
    state.click_colour(ColourRef::Paint(1), ColourClick::Load);
    let held = brush_totals(&state);
    assert!(held.pigment[1] > 0.0);
    assert_eq!(held.pigment[ULTRAMARINE_SLOT], 0.0);
    assert_eq!(knife_totals(&state).volume, 0.0);
}

#[test]
fn a_colour_the_palette_does_not_hold_is_ignored() {
    let mut state = painting_state();
    let before = brush_totals(&state);
    let recipe = state.session.brush.recipe.clone();
    state.click_colour(ColourRef::Mine(0), ColourClick::Load);
    assert_eq!(brush_totals(&state), before);
    assert_eq!(state.session.brush.recipe, recipe);
}

#[test]
fn a_paint_the_painting_does_not_have_is_ignored() {
    let mut state = painting_state();
    let mut spec = state.session.canvas.spec.clone();
    spec.pigment_slots.truncate(ULTRAMARINE_SLOT + 1);
    state.start_painting(spec).unwrap();
    let recipe = state.session.brush.recipe.clone();
    state.click_colour(ColourRef::Paint(state.slot_count()), ColourClick::Load);
    assert_eq!(state.session.brush.recipe, recipe);
}

#[test]
fn ctrl_clicking_the_circle_mixes_the_point_into_what_is_selected() {
    let mut state = painting_state();
    state.session.palette.circle = vec![ColourRef::Paint(YELLOW_SLOT)];
    state.click_circle(0.0, 1.0, ColourClick::MixIn);
    assert_eq!(
        state.session.brush.recipe,
        [(ULTRAMARINE_SLOT, 1.0), (YELLOW_SLOT, 0.25)]
    );
    assert!(brush_totals(&state).pigment[ULTRAMARINE_SLOT] > 0.0);
}

#[test]
fn mixing_a_colour_into_a_wiped_brush_takes_less_than_half_a_load_of_it() {
    let mut state = painting_state();
    state.click_colour(ColourRef::Paint(YELLOW_SLOT), ColourClick::Load);
    let full = brush_totals(&state).pigment[YELLOW_SLOT];
    state.click_colour(ColourRef::Paint(ULTRAMARINE_SLOT), ColourClick::Load);
    state.clean_brush();
    state.click_colour(ColourRef::Paint(YELLOW_SLOT), ColourClick::MixIn);
    let mixed_in = brush_totals(&state).pigment[YELLOW_SLOT];
    assert!(mixed_in > 0.0);
    assert!(mixed_in < full / 2.0, "{mixed_in} of {full}");
}

#[test]
fn clicking_a_colour_lifts_a_stroke_that_is_still_down() {
    let mut state = painting_state();
    state.press(sample_at(280.0, 100.0, 1.0), false);
    assert!(state.is_stroking());
    state.click_colour(ColourRef::Paint(YELLOW_SLOT), ColourClick::Load);
    assert!(!state.is_stroking());
}

#[test]
fn darkening_is_kept_within_its_range() {
    let mut state = painting_state();
    state.set_circle_darken(3.0);
    assert_eq!(state.circle_darken(), MAX_DARKEN);
    state.set_circle_darken(f32::NAN);
    assert_eq!(state.circle_darken(), MAX_DARKEN);
    state.set_circle_darken(-1.0);
    assert_eq!(state.circle_darken(), 0.0);
}

#[test]
fn the_selected_recipe_is_the_selected_colour_summing_to_one() {
    let mut state = painting_state();
    assert_eq!(
        state.selected_recipe(),
        Some(paint_recipe(ULTRAMARINE_SLOT))
    );
    state.click_colour(ColourRef::Paint(0), ColourClick::MixIn);
    let selected = state.selected_recipe().unwrap();
    assert!((selected[ULTRAMARINE_SLOT] - 0.8).abs() < SHARE_TOLERANCE);
    assert!((selected[0] - 0.2).abs() < SHARE_TOLERANCE);
    state.session.brush.recipe.clear();
    assert_eq!(state.selected_recipe(), None);
}

#[test]
fn the_app_opens_on_the_colours_view_with_nothing_darkened() {
    let state = painting_state();
    assert_eq!(state.palette_view, PaletteView::Colours);
    assert_eq!(state.circle_darken(), 0.0);
}

#[test]
fn adding_a_pigment_with_the_brush_loads_it_clean() {
    let mut state = painting_state();
    let mut spec = state.session.canvas.spec.clone();
    spec.pigment_slots.truncate(ULTRAMARINE_SLOT + 1);
    state.start_painting(spec).unwrap();
    let slot = state.add_pile("burnt_umber").unwrap();
    assert_eq!(state.session.brush.recipe, [(slot, 1.0)]);
    assert_eq!(brush_totals(&state).pigment[ULTRAMARINE_SLOT], 0.0);
}
