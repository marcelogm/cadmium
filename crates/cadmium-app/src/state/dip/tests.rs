use super::*;
use crate::state::test_support::{
    PAINT_ROW_Y, RESERVE_TOLERANCE, ULTRAMARINE_SLOT, WHITE_SLOT, brush_totals, holding_the_knife,
    knife_totals, paint_stroke, painting_state, reserve_volume, row_samples,
};
use cadmium_engine::pigments::MAX_SLOTS;
use cadmium_engine::stroke::Tool;

const BARE_CANVAS_POINT: (f32, f32) = (20.0, 20.0);

#[test]
fn choosing_another_pigment_cleans_the_brush_before_loading_it() {
    let mut state = painting_state();
    paint_stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    state.choose_pigment(WHITE_SLOT, PigmentClick::Choose);
    let totals = brush_totals(&state);
    assert_eq!(totals.pigment[ULTRAMARINE_SLOT], 0.0);
    assert!(totals.pigment[WHITE_SLOT] > 0.0);
    assert_eq!(state.session.brush.recipe, [(WHITE_SLOT, 1.0)]);
}

#[test]
fn ctrl_click_mixes_a_little_of_the_pigment_into_what_the_brush_holds() {
    let mut state = painting_state();
    let before = brush_totals(&state);
    state.choose_pigment(WHITE_SLOT, PigmentClick::Mix);
    let after = brush_totals(&state);
    assert!(after.pigment[WHITE_SLOT] > 0.0);
    assert!(after.pigment[WHITE_SLOT] < after.pigment[ULTRAMARINE_SLOT]);
    let kept = after.pigment[ULTRAMARINE_SLOT] / before.pigment[ULTRAMARINE_SLOT];
    assert!((kept - 1.0).abs() < RESERVE_TOLERANCE, "{kept}");
    assert_eq!(
        state.session.brush.recipe,
        [(ULTRAMARINE_SLOT, 1.0), (WHITE_SLOT, PARTIAL_DIP)]
    );
}

#[test]
fn clicking_the_loaded_pigment_again_dips_more_paint_on_top() {
    let mut state = painting_state();
    paint_stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    let worn = brush_totals(&state);
    state.choose_pigment(ULTRAMARINE_SLOT, PigmentClick::Choose);
    let dipped = brush_totals(&state);
    assert!(dipped.volume > worn.volume);
    assert_eq!(dipped.pigment[WHITE_SLOT], 0.0);
}

#[test]
fn eyedropper_on_bare_canvas_changes_nothing() {
    let mut state = painting_state();
    paint_stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    let before = brush_totals(&state);
    let recipe = state.session.brush.recipe.clone();
    let (x, y) = BARE_CANVAS_POINT;
    assert!(!state.pick_colour_on(Surface::Painting, x, y));
    assert_eq!(brush_totals(&state), before);
    assert_eq!(state.session.brush.recipe, recipe);
}

#[test]
fn eyedropper_mixes_the_picked_paint_on_the_palette_and_loads_it() {
    let mut state = painting_state();
    let samples = row_samples(PAINT_ROW_Y);
    paint_stroke(&mut state, Tool::Brush, &samples);
    state.choose_pigment(WHITE_SLOT, PigmentClick::Choose);
    let middle = samples[samples.len() / 2];
    assert!(state.pick_colour_on(Surface::Painting, middle.x as f32, middle.y as f32));
    let recipe = &state.session.brush.recipe;
    assert_eq!(recipe.len(), 1, "{recipe:?}");
    assert_eq!(recipe[0].0, ULTRAMARINE_SLOT);
    let totals = brush_totals(&state);
    assert_eq!(totals.pigment[WHITE_SLOT], 0.0);
    assert!(totals.pigment[ULTRAMARINE_SLOT] > 0.0);
}

#[test]
fn picking_the_colour_already_on_the_brush_refreshes_it() {
    let mut state = painting_state();
    let full = reserve_volume(&state);
    let samples = row_samples(PAINT_ROW_Y);
    state.reload_each_stroke = false;
    paint_stroke(&mut state, Tool::Brush, &samples);
    assert!(reserve_volume(&state) < full);
    let middle = samples[samples.len() / 2];
    assert!(state.pick_colour_on(Surface::Painting, middle.x as f32, middle.y as f32));
    assert!((reserve_volume(&state) - full).abs() <= full * RESERVE_TOLERANCE);
    assert_eq!(state.session.brush.recipe.len(), 1);
    assert_eq!(state.session.brush.recipe[0].0, ULTRAMARINE_SLOT);
}

#[test]
fn brush_readout_follows_loading_painting_and_cleaning() {
    let mut state = painting_state();
    assert!(!state.tip_mixture.is_empty());
    let loaded = state.reserve_level();
    assert!(loaded > 0.0);
    paint_stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    assert!(state.reserve_level() < loaded);
    state.clean_brush();
    assert!(state.brush_is_clean());
    assert_eq!(state.reserve_level(), 0.0);
}

#[test]
fn knife_colour_is_the_mean_of_what_it_carries() {
    let mut pigment = [0.0; MAX_SLOTS];
    pigment[1] = 30.0;
    pigment[3] = 60.0;
    let mixture = mean_mixture(&PaintTotals {
        volume: 100.0,
        pigment,
    });
    assert_eq!(mixture.volume_um, 100.0);
    assert!((mixture.fractions[1] - 0.3).abs() < 1e-6);
    assert!((mixture.fractions[3] - 0.6).abs() < 1e-6);
    assert!(mean_mixture(&PaintTotals::default()).is_empty());
}

const KNIFE_LEVEL_TOLERANCE: f32 = 0.01;

#[test]
fn a_swatch_click_with_the_knife_loads_the_knife_and_leaves_the_brush_alone() {
    let mut state = holding_the_knife();
    let brush = brush_totals(&state);
    state.choose_pigment(WHITE_SLOT, PigmentClick::Choose);
    let knife = knife_totals(&state);
    assert!(knife.pigment[WHITE_SLOT] > 0.0);
    assert_eq!(knife.pigment[ULTRAMARINE_SLOT], 0.0);
    assert_eq!(brush_totals(&state), brush);
    let level = state.knife_level();
    assert!(
        (level - state.session.brush.load).abs() < KNIFE_LEVEL_TOLERANCE,
        "{level}"
    );
    assert!(!state.knife_is_clean());
    assert!(state.knife_mixture.fractions[WHITE_SLOT] > 0.0);
}

#[test]
fn another_pigment_on_the_knife_cleans_it_first_and_ctrl_click_adds_a_little() {
    let mut state = holding_the_knife();
    state.choose_pigment(WHITE_SLOT, PigmentClick::Choose);
    state.choose_pigment(ULTRAMARINE_SLOT, PigmentClick::Choose);
    let full = knife_totals(&state);
    assert_eq!(full.pigment[WHITE_SLOT], 0.0);
    state.choose_pigment(WHITE_SLOT, PigmentClick::Mix);
    let mixed = knife_totals(&state);
    assert!(mixed.pigment[WHITE_SLOT] > 0.0);
    assert!(mixed.pigment[WHITE_SLOT] < mixed.pigment[ULTRAMARINE_SLOT]);
    assert_eq!(
        state.session.brush.recipe,
        [(ULTRAMARINE_SLOT, 1.0), (WHITE_SLOT, PARTIAL_DIP)]
    );
}

#[test]
fn cleaning_the_knife_keeps_the_palette_pile_and_the_brush() {
    let mut state = holding_the_knife();
    state.choose_pigment(ULTRAMARINE_SLOT, PigmentClick::Choose);
    let brush = brush_totals(&state);
    let recipe = state.session.brush.recipe.clone();
    state.clean_brush();
    assert_eq!(knife_totals(&state).volume, 0.0);
    assert!(state.knife_is_clean());
    assert_eq!(state.knife_level(), 0.0);
    assert_eq!(state.session.brush.recipe, recipe);
    assert_eq!(brush_totals(&state), brush);
}

#[test]
fn reserve_level_is_a_share_of_the_full_reserve() {
    let small = 100.0;
    assert_eq!(fill_level(small / 2.0, small), 0.5);
    assert_eq!(fill_level(small * 2.0, small), 1.0);
    assert_eq!(fill_level(1.0, 0.0), 0.0);
}
