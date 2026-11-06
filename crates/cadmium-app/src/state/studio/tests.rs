use std::cell::Cell;
use std::rc::Rc;

use cadmium_engine::brush::BrushSpec;
use cadmium_engine::brush::factory_brushes;
use cadmium_engine::canvas::Primer;
use cadmium_engine::engine::BristleSnapshot;
use cadmium_engine::engine::PaintEngine;
use cadmium_engine::params::SimParams;
use cadmium_engine::present::readback_rgba8;
use cadmium_studio::{DemoStroke, OrbitCamera, PlayerRequest, SLOW_MOTION, StudioPlayer};

use crate::state::AppState;
use crate::state::test_support::{ScriptedClock, painting_state, scripted_painting_state};

const WORN_INDEX: usize = 2;
const STALL_MS: f64 = 5000.0;
const FRAME_MS: f64 = 50.0;
const TARGET_SIZE: u32 = 64;

fn snapshot(state: &AppState) -> BristleSnapshot {
    let studio = state.studio.as_ref().expect("the studio is open");
    studio
        .player
        .session()
        .engine
        .bristle_snapshot(&state.gpu)
        .unwrap()
}

fn open_at_rest(clock: &ScriptedClock) -> AppState {
    let mut state = scripted_painting_state(clock);
    state.open_studio();
    state.encode_studio_frame().unwrap();
    state
}

/// A player shown what the state's studio shows, advanced by `elapsed_ms`.
fn reference_player(state: &AppState, speed: f64, elapsed_ms: f64) -> StudioPlayer {
    let shown = &state.studio.as_ref().unwrap().player;
    let paint = state.studio_paint();
    let request = PlayerRequest {
        brush: shown.brush(),
        paint: &paint,
        stroke: shown.stroke(),
    };
    let mut player = StudioPlayer::new(&state.gpu, request).unwrap();
    player.set_speed(speed);
    player.advance(&state.gpu, elapsed_ms).unwrap();
    player
}

fn reference(state: &AppState, speed: f64, elapsed_ms: f64) -> BristleSnapshot {
    let player = reference_player(state, speed, elapsed_ms);
    player
        .session()
        .engine
        .bristle_snapshot(&state.gpu)
        .unwrap()
}

/// The studio is at the start of a loop: no brush on show and a fresh canvas.
/// (A restart in the same session keeps the lifted hairs where they were, so
/// the bristles are not compared.)
fn assert_at_loop_start(state: &AppState) {
    let player = &state.studio.as_ref().unwrap().player;
    assert!(!player.brush_visible());
    let fresh = reference_player(state, 1.0, 0.0);
    assert_eq!(
        player.rgba(&state.gpu).unwrap(),
        fresh.rgba(&state.gpu).unwrap()
    );
}

#[test]
fn the_worn_library_brush_is_shown_at_the_current_size() {
    let mut state = painting_state();
    assert!(state.wear_library_brush(WORN_INDEX));
    state.session.brush.geometry.max_diameter_px = 33.0;
    let expected = BrushSpec {
        size_px: 33.0,
        ..state.brushes.get(WORN_INDEX).unwrap().clone()
    };
    assert_eq!(state.worn_brush_spec(), expected);
}

#[test]
fn a_brush_outside_the_library_is_the_current_brush() {
    let mut state = painting_state();
    state.session.brush.head.thickness += 0.123;
    assert_eq!(state.worn_library_brush(), None);
    let settings = &state.session.brush;
    let expected = BrushSpec {
        name: "Current Brush".into(),
        size_px: settings.geometry.max_diameter_px,
        head: settings.head,
        hair: settings.hair,
    };
    assert_eq!(state.worn_brush_spec(), expected);
}

#[test]
fn the_studio_paint_is_the_paintings() {
    let mut state = painting_state();
    let key = SimParams::specs()[0].key;
    state.set_param(key, SimParams::specs()[0].max);
    state.session.brush.recipe = vec![(2, 0.25), (4, 0.75)];
    state.set_load(0.6);
    state.set_medium(0.3);
    let paint = state.studio_paint();
    let spec = &state.session.canvas.spec;
    assert_eq!(paint.slots, spec.pigment_slots);
    assert_eq!(paint.primer, spec.primer);
    assert_eq!(paint.weave, spec.weave);
    assert_eq!(paint.recipe, vec![(2, 0.25), (4, 0.75)]);
    assert_eq!(paint.load, 0.6);
    assert_eq!(paint.medium, 0.3);
    assert_eq!(&paint.params, state.session.engine.params());
}

#[test]
fn an_empty_brush_shows_the_first_colour_fully_loaded() {
    let mut state = painting_state();
    state.session.brush.recipe.clear();
    let paint = state.studio_paint();
    assert_eq!(paint.recipe, vec![(1, 1.0)]);
    assert_eq!(paint.load, 1.0);
    state.session.brush.recipe = vec![(4, 1.0)];
    state.set_load(0.0);
    assert_eq!(state.studio_paint().recipe, vec![(1, 1.0)]);
}

#[test]
fn an_all_white_palette_falls_back_to_slot_zero() {
    let mut state = painting_state();
    state.session.canvas.spec.pigment_slots = vec!["titanium_white".into(), "zinc_white".into()];
    state.session.brush.recipe.clear();
    assert_eq!(state.studio_paint().recipe, vec![(0, 1.0)]);
}

#[test]
fn the_studio_opens_and_closes() {
    let mut state = painting_state();
    assert!(!state.studio_open());
    state.open_studio();
    assert!(state.studio_open());
    assert!(state.studio_mut().is_some());
    assert_eq!(state.take_studio_error(), None);
    state.close_studio();
    assert!(!state.studio_open());
    assert!(state.encode_studio_frame().is_ok());
}

#[test]
fn opening_and_closing_the_studio_wake_its_views() {
    let mut state = painting_state();
    let wakes = Rc::new(Cell::new(0));
    let counted = wakes.clone();
    state.add_waker(Rc::new(move || counted.set(counted.get() + 1)));
    state.open_studio();
    assert_eq!(wakes.get(), 1);
    state.close_studio();
    assert_eq!(wakes.get(), 2);
}

#[test]
fn a_stalled_frame_advances_at_most_the_frame_limit() {
    let clock = ScriptedClock::starting_at(1000.0);
    let mut state = open_at_rest(&clock);
    state.studio_mut().unwrap().player.set_speed(SLOW_MOTION);
    clock.advance(STALL_MS);
    state.encode_studio_frame().unwrap();
    assert_eq!(snapshot(&state), reference(&state, SLOW_MOTION, 100.0));
    assert_ne!(snapshot(&state), reference(&state, SLOW_MOTION, 0.0));
}

#[test]
fn a_frame_advances_by_the_time_since_the_last() {
    let clock = ScriptedClock::starting_at(1000.0);
    let mut state = open_at_rest(&clock);
    clock.advance(FRAME_MS);
    state.encode_studio_frame().unwrap();
    assert_eq!(snapshot(&state), reference(&state, 1.0, FRAME_MS));
}

/// Behaviour only: the player itself treats a negative time as none.
#[test]
fn a_clock_going_back_does_not_advance() {
    let clock = ScriptedClock::starting_at(1000.0);
    let mut state = open_at_rest(&clock);
    let before = snapshot(&state);
    clock.advance(-FRAME_MS);
    state.encode_studio_frame().unwrap();
    assert_eq!(snapshot(&state), before);
}

#[test]
fn a_paused_studio_stands_still() {
    let clock = ScriptedClock::starting_at(1000.0);
    let mut state = open_at_rest(&clock);
    let before = snapshot(&state);
    state.studio_mut().unwrap().player.pause();
    clock.advance(FRAME_MS);
    state.encode_studio_frame().unwrap();
    assert_eq!(snapshot(&state), before);
}

#[test]
fn new_paint_restarts_the_studio() {
    let clock = ScriptedClock::starting_at(1000.0);
    let mut state = open_at_rest(&clock);
    for _ in 0..3 {
        clock.advance(FRAME_MS);
        state.encode_studio_frame().unwrap();
    }
    assert_ne!(snapshot(&state), reference(&state, 1.0, 0.0));
    state.set_load(0.4);
    state.session.canvas.spec.primer = Primer::SiennaImprimatura;
    assert_ne!(
        state.studio.as_ref().unwrap().player.paint(),
        &state.studio_paint()
    );
    state.encode_studio_frame().unwrap();
    assert_eq!(
        state.studio.as_ref().unwrap().player.paint(),
        &state.studio_paint()
    );
    assert_eq!(snapshot(&state), reference(&state, 1.0, 0.0));
}

fn present(state: &mut AppState, target: &wgpu::Texture) -> Vec<u8> {
    let gpu = state.gpu.clone();
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    state.encode_studio_present(&gpu, target, &mut encoder);
    gpu.queue.submit([encoder.finish()]);
    readback_rgba8(&gpu, target).unwrap()
}

#[test]
fn a_closed_studio_leaves_the_target_untouched() {
    let clock = ScriptedClock::starting_at(1000.0);
    let mut state = open_at_rest(&clock);
    let target = state
        .session
        .presenter
        .create_target(&state.gpu, TARGET_SIZE, TARGET_SIZE);
    let shown = present(&mut state, &target);
    assert!(shown.iter().any(|&byte| byte != 0));
    state.close_studio();
    clock.advance(FRAME_MS);
    assert_eq!(present(&mut state, &target), shown);
}

fn advanced_and_paused(clock: &ScriptedClock) -> AppState {
    let mut state = open_at_rest(clock);
    clock.advance(FRAME_MS);
    state.encode_studio_frame().unwrap();
    let studio = state.studio_mut().unwrap();
    assert!(studio.player.brush_visible());
    studio.player.pause();
    studio.camera.orbit(40.0, 10.0);
    state
}

#[test]
fn a_new_stroke_restarts_the_loop_and_keeps_the_view() {
    let clock = ScriptedClock::starting_at(1000.0);
    let mut state = advanced_and_paused(&clock);
    let camera = state.studio.as_ref().unwrap().camera;
    state.set_studio_stroke(DemoStroke::Zigzag);
    let studio = state.studio.as_ref().unwrap();
    assert_eq!(studio.player.stroke(), DemoStroke::Zigzag);
    assert!(!studio.player.is_playing());
    assert_eq!(studio.camera, camera);
    assert_at_loop_start(&state);
}

#[test]
fn a_new_brush_restarts_the_loop_and_reframes_the_view() {
    let clock = ScriptedClock::starting_at(1000.0);
    let mut state = advanced_and_paused(&clock);
    let target = state
        .session
        .presenter
        .create_target(&state.gpu, TARGET_SIZE * 2, TARGET_SIZE);
    present(&mut state, &target);
    let brush = BrushSpec {
        size_px: 48.0,
        ..factory_brushes()[WORN_INDEX].clone()
    };
    state.set_studio_brush(&brush);
    let studio = state.studio.as_ref().unwrap();
    assert_eq!(studio.player.brush(), &brush);
    assert_eq!(studio.player.stroke(), DemoStroke::SCurve);
    assert!(!studio.player.is_playing());
    let framed = OrbitCamera::framing(studio.player.stroke_box(), studio.player.reach_px(), 2.0);
    assert_eq!(studio.camera, framed);
    assert_at_loop_start(&state);
}

#[test]
fn a_closed_studio_ignores_new_strokes_and_brushes() {
    let mut state = painting_state();
    state.set_studio_stroke(DemoStroke::Zigzag);
    state.set_studio_brush(&factory_brushes()[0]);
    assert!(!state.studio_open());
    assert_eq!(state.take_studio_error(), None);
}
