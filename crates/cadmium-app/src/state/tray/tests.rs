use super::*;
use crate::state::test_support::{
    ULTRAMARINE_SLOT, brush_totals, paint_blob_on_tray, painting_state, settle_both_surfaces,
};

const YELLOW_SLOT: usize = 1;

#[test]
fn a_pile_click_loads_the_brush() {
    let mut state = painting_state();
    state.click_pile(YELLOW_SLOT, PigmentClick::Choose);
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

#[test]
fn clearing_the_tray_gives_only_the_tray_pending_work() {
    let mut state = painting_state();
    paint_blob_on_tray(&mut state, 1);
    settle_both_surfaces(&mut state);
    state.clear_tray();
    assert!(state.session.tray_has_pending_work());
    assert!(!state.session.has_pending_work());
}

#[test]
fn clearing_the_tray_is_an_edit() {
    let mut state = painting_state();
    paint_blob_on_tray(&mut state, 0);
    state.clear_tray();
    assert_eq!(
        state.session.tray.wet_totals(&state.gpu).unwrap().volume,
        0.0
    );
    assert!(state.has_unsaved_edits());
}

#[test]
fn clearing_a_clean_tray_is_not_an_edit() {
    let mut state = painting_state();
    state.clear_tray();
    assert!(!state.has_unsaved_edits());
}

fn zoomed(zoom: f32) -> ViewTransform {
    ViewTransform {
        origin_x: 0.0,
        origin_y: 0.0,
        zoom,
        rotation: 0.0,
    }
}

#[test]
fn the_tray_s_screen_zoom_is_in_widget_px() {
    let mut state = painting_state();
    state.set_tray_view(zoomed(0.5), 2.0);
    assert_eq!(state.tray_screen_zoom(), 0.25);
}

#[test]
fn a_tray_view_that_is_not_a_scale_is_ignored() {
    let mut state = painting_state();
    state.set_tray_view(zoomed(0.5), 2.0);
    for zoom in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        state.set_tray_view(zoomed(zoom), 1.0);
    }
    for scale in [0.0, -1.0, f64::NAN] {
        state.set_tray_view(zoomed(0.75), scale);
    }
    assert_eq!(state.tray_screen_zoom(), 0.25);
}
