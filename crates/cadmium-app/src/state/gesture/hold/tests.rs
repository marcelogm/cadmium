use super::*;
use crate::state::test_support::{
    WHITE_SLOT, brush_totals, painting_state, present_one_frame, present_one_tray_frame, sample_at,
    settle_both_surfaces, squeeze_blob_on_tray,
};
use crate::tools::{TrayTool, UiTool};
use cadmium_engine::present::ViewTransform;

/// A tray view at `zoom` device px per tray px.
fn zoomed(zoom: f32) -> ViewTransform {
    ViewTransform {
        origin_x: 0.0,
        origin_y: 0.0,
        zoom,
        rotation: 0.0,
    }
}

#[test]
fn load_is_cancelled_past_the_threshold() {
    assert!(!load_cancelled((10.0, 10.0), (14.0, 10.0), 4.0));
    assert!(load_cancelled((10.0, 10.0), (14.01, 10.0), 4.0));
    assert!(!load_cancelled((10.0, 10.0), (10.0, 10.0), 4.0));
}

#[test]
fn the_cancel_distance_is_measured_on_screen() {
    for ((zoom, scale), (drag, cancels)) in [
        ((0.25, 1.0), (10.0, false)),
        ((0.25, 1.0), (20.0, true)),
        // On a 2x screen the same zoom in widget px is twice the device zoom.
        ((0.5, 2.0), (10.0, false)),
        ((0.5, 2.0), (20.0, true)),
    ] {
        let mut state = painting_state();
        state.set_tray_view(zoomed(zoom), scale);
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        state.motion(&[sample_at(300.0 + drag, 300.0, 0.9)]);
        assert_eq!(
            !state.holds_on(Surface::Tray),
            cancels,
            "{zoom} {scale} {drag}"
        );
        state.release();
    }
}

#[test]
fn a_tray_zoom_that_is_not_a_scale_is_ignored() {
    let mut state = painting_state();
    for zoom in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        state.set_tray_view(zoomed(zoom), 1.0);
    }
    for scale in [0.0, -1.0, f64::NAN] {
        state.set_tray_view(zoomed(0.25), scale);
    }
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    state.motion(&[sample_at(305.0, 300.0, 0.9)]);
    assert!(!state.holds_on(Surface::Tray));
    state.release();
}

#[test]
fn a_hold_step_of_no_real_time_squeezes_nothing() {
    let mut state = painting_state();
    state.set_tray_tool(TrayTool::Tube);
    state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
    state.advance_hold(f32::NAN);
    state.release();
    assert_eq!(
        state.session.tray.wet_totals(&state.gpu).unwrap().volume,
        0.0
    );
    assert!(!state.has_unsaved_edits());
}

#[test]
fn squeezing_off_the_tray_is_not_an_edit() {
    let mut state = painting_state();
    state.set_tray_tool(TrayTool::Tube);
    state.press(Surface::Tray, sample_at(-1000.0, -1000.0, 1.0), false);
    state.advance_hold(MAX_HOLD_STEP_SECONDS);
    state.release();
    assert!(!state.has_unsaved_edits());
}

const FRAME_GAP: std::time::Duration = std::time::Duration::from_millis(20);

fn tray_volume(state: &AppState) -> f64 {
    state.session.tray.wet_totals(&state.gpu).unwrap().volume
}

#[test]
fn a_hold_on_the_tray_advances_only_with_tray_frames() {
    let mut state = painting_state();
    state.set_tray_tool(TrayTool::Tube);
    state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
    for _ in 0..2 {
        present_one_frame(&mut state);
        std::thread::sleep(FRAME_GAP);
    }
    assert_eq!(tray_volume(&state), 0.0);
    for _ in 0..2 {
        present_one_tray_frame(&mut state);
        std::thread::sleep(FRAME_GAP);
    }
    state.release();
    assert!(tray_volume(&state) > 0.0);
}

#[test]
fn a_still_touch_with_the_knife_names_the_pile_after_what_the_blade_took() {
    let mut state = painting_state();
    squeeze_blob_on_tray(&mut state, WHITE_SLOT);
    state.set_ui_tool(UiTool::Paint(Tool::Knife));
    state.clean_brush();
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    state.advance_hold(MAX_HOLD_STEP_SECONDS);
    state.release();
    assert!(state.knife_level() > 0.0);
    assert_eq!(state.session.brush.recipe.len(), 1);
    assert_eq!(state.session.brush.recipe[0].0, WHITE_SLOT);
}

#[test]
fn a_still_touch_on_the_tray_loads_the_brush_from_it() {
    let mut state = painting_state();
    squeeze_blob_on_tray(&mut state, 1);
    state.clean_brush();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    assert!(state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false));
    state.advance_hold(0.2);
    let ring = state.load_ring().unwrap();
    state.release();
    assert!(ring.level > 0.0);
    assert!(state.reserve_level() > 0.0);
    let slots: Vec<usize> = state
        .session
        .brush
        .recipe
        .iter()
        .map(|(slot, _)| *slot)
        .collect();
    assert_eq!(slots, vec![1]);
}

#[test]
fn a_load_that_takes_nothing_is_not_an_edit() {
    let mut state = painting_state();
    let recipe = state.session.brush.recipe.clone();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    state.advance_hold(0.2);
    state.release();
    assert!(!state.has_unsaved_edits());
    assert_eq!(state.session.brush.recipe, recipe);
}

#[test]
fn a_tray_hold_leaves_the_painting_with_no_pending_work() {
    let mut state = painting_state();
    settle_both_surfaces(&mut state);
    state.set_tray_tool(TrayTool::Tube);
    state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
    state.advance_hold(MAX_HOLD_STEP_SECONDS);
    assert!(state.session.tray_has_pending_work());
    assert!(!state.session.has_pending_work());
    state.release();
    assert!(!state.session.has_pending_work());
}

#[test]
fn a_still_touch_on_a_clean_tray_shows_no_ring() {
    let mut state = painting_state();
    state.clean_brush();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    assert_eq!(state.load_ring(), None);
    state.advance_hold(0.2);
    assert_eq!(state.load_ring(), None);
    state.release();
}

#[test]
fn a_full_brush_closes_the_ring_at_once_on_a_clean_tray() {
    let mut state = painting_state();
    state.set_load(1.0);
    state.reload_brush();
    assert!(state.reserve_level() >= FULL_LEVEL);
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    let ring = state.load_ring().expect("a full brush rings at once");
    assert!((ring.level - 1.0).abs() < 1e-3, "{}", ring.level);
    state.advance_hold(0.2);
    let ring = state.load_ring().expect("still full");
    assert!((ring.level - 1.0).abs() < 1e-3, "{}", ring.level);
    state.release();
    assert!(!state.has_unsaved_edits());
}

#[test]
fn the_ring_appears_once_paint_is_drawn_up() {
    let mut state = painting_state();
    squeeze_blob_on_tray(&mut state, 1);
    state.clean_brush();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    assert_eq!(state.load_ring(), None);
    state.advance_hold(0.2);
    let ring = state.load_ring().expect("paint was drawn up");
    assert!(ring.level > 0.0);
    assert_eq!(ring.pressure, 0.9);
    state.release();
}

#[test]
fn dragging_past_four_pixels_turns_the_load_into_a_stroke() {
    let mut state = painting_state();
    squeeze_blob_on_tray(&mut state, 1);
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    state.motion(&[sample_at(303.0, 300.0, 0.9)]);
    assert!(state.holds_on(Surface::Tray));
    state.motion(&[sample_at(310.0, 300.0, 0.9)]);
    assert!(!state.holds_on(Surface::Tray));
    assert!(state.is_stroking());
    state.release();
    assert!(!state.is_stroking());
}

#[test]
fn a_stroke_grown_from_a_load_keeps_what_was_loaded() {
    let mut state = painting_state();
    squeeze_blob_on_tray(&mut state, WHITE_SLOT);
    state.clean_brush();
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    state.advance_hold(0.1);
    let loaded = brush_totals(&state);
    state.motion(&[sample_at(310.0, 300.0, 0.9)]);
    assert!(state.reserve_level() < 1.0);
    assert_eq!(brush_totals(&state), loaded);
    assert!(loaded.pigment[WHITE_SLOT] > 0.0);
    state.release();
}

#[test]
fn the_tube_squeezes_its_own_pigment_and_leaves_the_brush_alone() {
    let mut state = painting_state();
    state.clean_brush();
    state.set_tray_tool(TrayTool::Tube);
    state.set_tube_slot(1);
    state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
    state.advance_hold(0.2);
    state.release();
    let totals = state.session.tray.wet_totals(&state.gpu).unwrap();
    assert!(totals.pigment[1] > 0.0 && totals.pigment[0] == 0.0);
    assert!(state.brush_is_clean());
    assert!(state.has_unsaved_edits());
}

fn tray_height_at(state: &AppState, x: usize, y: usize) -> f32 {
    let volume = state
        .session
        .tray
        .read_f32(&state.gpu, cadmium_engine::canvas::StateTexture::WetVol)
        .unwrap();
    volume[y * state.session.tray.spec.width as usize + x]
}

#[test]
fn a_tube_on_a_zoomed_out_tray_keeps_growing_through_a_trembling_hand() {
    let mut state = painting_state();
    state.set_brush_size(10.0);
    state.set_tray_view(zoomed(0.25), 1.0);
    state.set_tray_tool(TrayTool::Tube);
    let one_screen_px = 1.0 / f64::from(state.tray_screen_zoom());
    let shaken = |frame: usize| {
        let shake = if frame.is_multiple_of(2) {
            one_screen_px
        } else {
            -one_screen_px
        };
        sample_at(300.5 + shake, 300.5, 0.5)
    };
    let tremble = |state: &mut AppState, frames: std::ops::Range<usize>| {
        for frame in frames {
            state.motion(&[shaken(frame)]);
            state.advance_hold(0.05);
        }
    };
    state.press(Surface::Tray, sample_at(300.5, 300.5, 0.5), false);
    tremble(&mut state, 0..10);
    let early = tray_height_at(&state, 300, 300);
    tremble(&mut state, 10..30);
    let later = tray_height_at(&state, 300, 300);
    state.release();
    assert!(later > early, "{early} -> {later}");
}

#[test]
fn a_hold_step_never_lasts_longer_than_a_tenth_of_a_second() {
    let mut once = painting_state();
    let mut long = painting_state();
    for state in [&mut once, &mut long] {
        state.set_tray_tool(TrayTool::Tube);
        state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
    }
    once.advance_hold(MAX_HOLD_STEP_SECONDS);
    long.advance_hold(10.0);
    once.release();
    long.release();
    let squeezed = |state: &AppState| state.session.tray.wet_totals(&state.gpu).unwrap().volume;
    assert_eq!(squeezed(&long), squeezed(&once));
}
