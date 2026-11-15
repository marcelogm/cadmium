use super::*;
use crate::state::test_support::{ULTRAMARINE_SLOT, brush_totals, painting_state};

const YELLOW_SLOT: usize = 1;

#[test]
fn a_pile_click_loads_the_brush() {
    let mut state = painting_state();
    state.click_pile(YELLOW_SLOT, PigmentClick::Load);
    assert_eq!(state.session.brush.recipe, [(YELLOW_SLOT, 1.0)]);
    assert!(brush_totals(&state).pigment[YELLOW_SLOT] > 0.0);
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
