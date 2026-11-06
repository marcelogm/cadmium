use cadmium_engine::engine::PaintEngine;
use cadmium_engine::params::SimParams;
use cadmium_studio::DEMO_PIGMENT;

use crate::presets::{PRIMARIES, ZORN};
use crate::state::test_support::painting_state;

/// Where the starting palette (`PRIMARIES`) keeps ultramarine.
const ULTRAMARINE_SLOT: usize = 4;

fn slots(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| id.to_string()).collect()
}

#[test]
fn the_examples_are_painted_in_the_paintings_paint() {
    let mut state = painting_state();
    let key = SimParams::specs()[0].key;
    state.set_param(key, SimParams::specs()[0].max);
    state.session.brush.recipe = vec![(2, 0.25), (6, 0.75)];
    state.set_load(0.6);
    state.set_medium(0.3);
    let paint = state.studio_paint();
    let spec = &state.session.canvas.spec;
    assert_eq!(paint.slots, spec.pigment_slots);
    assert_eq!(paint.primer, spec.primer);
    assert_eq!(paint.weave, spec.weave);
    assert_eq!(paint.recipe, vec![(2, 0.25), (6, 0.75)]);
    assert_eq!(paint.load, 0.6);
    assert_eq!(paint.medium, 0.3);
    assert_eq!(&paint.params, state.session.engine.params());
}

#[test]
fn an_empty_brush_shows_ultramarine_fully_loaded() {
    let mut state = painting_state();
    assert_eq!(PRIMARIES[ULTRAMARINE_SLOT], DEMO_PIGMENT);
    state.session.brush.recipe.clear();
    let paint = state.studio_paint();
    assert_eq!(paint.recipe, vec![(ULTRAMARINE_SLOT, 1.0)]);
    assert_eq!(paint.load, 1.0);
    state.session.brush.recipe = vec![(2, 1.0)];
    state.set_load(0.0);
    assert_eq!(state.studio_paint().recipe, vec![(ULTRAMARINE_SLOT, 1.0)]);
}

#[test]
fn a_brush_of_whites_only_shows_ultramarine() {
    let mut state = painting_state();
    state.session.brush.recipe = vec![(0, 1.0)];
    assert_eq!(state.studio_paint().recipe, vec![(ULTRAMARINE_SLOT, 1.0)]);
}

#[test]
fn a_brush_with_white_and_a_colour_keeps_its_paint() {
    let mut state = painting_state();
    state.session.brush.recipe = vec![(0, 0.5), (2, 0.5)];
    assert_eq!(state.studio_paint().recipe, vec![(0, 0.5), (2, 0.5)]);
}

#[test]
fn a_palette_without_ultramarine_gets_it_in_an_extra_slot() {
    let mut state = painting_state();
    state.session.canvas.spec.pigment_slots = slots(ZORN);
    state.session.brush.recipe.clear();
    let paint = state.studio_paint();
    let mut want = slots(ZORN);
    want.push(DEMO_PIGMENT.to_string());
    assert_eq!(paint.slots, want);
    assert_eq!(paint.recipe, vec![(ZORN.len(), 1.0)]);
    assert_eq!(state.session.canvas.spec.pigment_slots, slots(ZORN));
}

#[test]
fn a_full_palette_without_ultramarine_falls_back_to_its_first_colour() {
    let mut state = painting_state();
    let full = slots(&[
        "titanium_white",
        "zinc_white",
        "cadmium_yellow",
        "cadmium_red",
        "quinacridone_magenta",
        "phthalo_blue",
        "burnt_umber",
        "ivory_black",
    ]);
    state.session.canvas.spec.pigment_slots = full.clone();
    state.session.brush.recipe.clear();
    let paint = state.studio_paint();
    assert_eq!(paint.slots, full);
    assert_eq!(paint.recipe, vec![(2, 1.0)]);
}
