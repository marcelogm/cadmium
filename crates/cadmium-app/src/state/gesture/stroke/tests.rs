use cadmium_engine::pigments::Mixture;

use super::*;
use crate::state::recipe::PARTIAL_DIP;
use crate::state::test_support::{
    PAINT_ROW_Y, RAG_ROW_Y, RESERVE_TOLERANCE, ULTRAMARINE_SLOT, WHITE_SLOT, brush_totals,
    finish_painting, holding_the_knife, knife_totals, paint_stroke, painting_state,
    present_one_tray_frame, reserve_volume, row_samples, sample_at, settle_both_surfaces,
};
use crate::tools::{TrayTool, UiTool};

#[test]
fn a_tray_stroke_leaves_the_painting_with_no_pending_work() {
    let mut state = painting_state();
    settle_both_surfaces(&mut state);
    state.set_tray_tool(TrayTool::Blender);
    state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
    state.motion(&[sample_at(340.0, 300.0, 0.9)]);
    assert!(state.session.tray_has_pending_work());
    assert!(!state.session.has_pending_work());
    state.release();
    assert!(state.session.tray_has_pending_work());
    assert!(!state.session.has_pending_work());
    finish_painting(&mut state, present_one_tray_frame);
    present_one_tray_frame(&mut state);
    assert!(!state.session.tray_has_pending_work());
}

#[test]
fn a_painting_stroke_leaves_the_tray_with_no_pending_work() {
    let mut state = painting_state();
    settle_both_surfaces(&mut state);
    state.set_ui_tool(UiTool::Paint(Tool::Brush));
    state.press(Surface::Painting, sample_at(100.0, 100.0, 0.9), false);
    state.motion(&[sample_at(140.0, 100.0, 0.9)]);
    state.release();
    assert!(state.session.has_pending_work());
    assert!(!state.session.tray_has_pending_work());
}

const DIRT_DIP: f32 = 0.1;

fn single_pigment(state: &AppState, slot: usize) -> Mixture {
    let mut brush = state.session.brush.clone();
    brush.recipe = vec![(slot, 1.0)];
    brush.reservoir_mixture(state.slot_count())
}

#[test]
fn each_stroke_tops_the_reserve_back_up_while_the_tip_keeps_its_dirt() {
    let mut state = painting_state();
    let full = reserve_volume(&state);
    paint_stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    assert!(reserve_volume(&state) < full);
    let white = single_pigment(&state, WHITE_SLOT);
    state
        .session
        .dip(&state.gpu.clone(), &white, DIRT_DIP)
        .unwrap();
    let samples = row_samples(RAG_ROW_Y);
    state.begin_stroke(Tool::Brush, samples[0]);
    assert!((reserve_volume(&state) - full).abs() <= full * RESERVE_TOLERANCE);
    assert!(brush_totals(&state).pigment[WHITE_SLOT] > 0.0);
}

#[test]
fn without_reloading_the_reserve_keeps_running_down() {
    let mut state = painting_state();
    state.reload_each_stroke = false;
    paint_stroke(&mut state, Tool::Brush, &row_samples(PAINT_ROW_Y));
    let worn = reserve_volume(&state);
    let samples = row_samples(RAG_ROW_Y);
    state.begin_stroke(Tool::Brush, samples[0]);
    assert_eq!(reserve_volume(&state), worn);
}

#[test]
fn cleaning_each_stroke_starts_every_stroke_from_the_palette_mix() {
    let mut state = painting_state();
    state.clean_each_stroke = true;
    let white = single_pigment(&state, WHITE_SLOT);
    state
        .session
        .dip(&state.gpu.clone(), &white, PARTIAL_DIP)
        .unwrap();
    assert!(brush_totals(&state).pigment[WHITE_SLOT] > 0.0);
    let samples = row_samples(PAINT_ROW_Y);
    state.begin_stroke(Tool::Brush, samples[0]);
    let totals = brush_totals(&state);
    assert_eq!(totals.pigment[WHITE_SLOT], 0.0);
    assert!(totals.pigment[ULTRAMARINE_SLOT] > 0.0);
}

#[test]
fn brush_and_knife_reload_or_clean_at_pen_down_and_other_tools_do_not() {
    for tool in [Tool::Brush, Tool::Knife] {
        assert_eq!(
            stroke_preparation(tool, true, false),
            StrokePreparation::TopUp
        );
        assert_eq!(
            stroke_preparation(tool, true, true),
            StrokePreparation::CleanAndLoad
        );
        assert_eq!(
            stroke_preparation(tool, false, false),
            StrokePreparation::None
        );
    }
    for tool in [
        Tool::Blender,
        Tool::Rag,
        Tool::Eyedropper,
        Tool::MixingKnife,
    ] {
        assert_eq!(
            stroke_preparation(tool, true, true),
            StrokePreparation::None
        );
    }
}

#[test]
fn knife_comes_down_loaded_to_the_palette_load_and_no_further() {
    let mut state = holding_the_knife();
    assert_eq!(knife_totals(&state).volume, 0.0);
    let samples = row_samples(PAINT_ROW_Y);
    state.begin_stroke(Tool::Knife, samples[0]);
    let mixture = state.palette_mixture();
    let target = f64::from(
        state
            .session
            .engine
            .knife_full_dip_volume(mixture.volume_um),
    );
    let loaded = knife_totals(&state).volume;
    assert!(
        (loaded - target).abs() <= target * RESERVE_TOLERANCE,
        "{loaded} {target}"
    );
    state.release();
    state.begin_stroke(Tool::Knife, samples[0]);
    let again = knife_totals(&state).volume;
    assert!(
        again <= target * (1.0 + RESERVE_TOLERANCE),
        "{again} {target}"
    );
    state.release();
}

#[test]
fn without_reloading_the_knife_stays_as_empty_as_it_is() {
    let mut state = holding_the_knife();
    state.reload_each_stroke = false;
    let samples = row_samples(PAINT_ROW_Y);
    state.begin_stroke(Tool::Knife, samples[0]);
    assert_eq!(knife_totals(&state).volume, 0.0);
    state.release();
}
