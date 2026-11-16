pub mod support;

use cadmium_engine::{
    canvas::StateTexture,
    gpu::GpuContext,
    history::{History, HistoryLimits},
    session::PaintSession,
};
use support::{load, session, stroke, without_wgpu_errors};

const BLUE: usize = 0;
const YELLOW: usize = 1;
const UPPER_ROW: f64 = 64.0;
const LOWER_ROW: f64 = 192.0;
/// Far enough above the canvas that no dab of the stroke reaches a tile.
const ROW_OFF_THE_CANVAS: f64 = -600.0;
const NARROW_DIAMETER_PX: f32 = 24.0;
const WIDE_DIAMETER_PX: f32 = 160.0;

fn one_capture_bytes(gpu: &GpuContext, s: &PaintSession) -> u64 {
    s.engine.capture_held_paint(gpu).unwrap().bytes()
}

fn budget_held_paint(gpu: &GpuContext, s: &mut PaintSession, held_paint_budget_bytes: u64) {
    s.history = History::new(
        gpu,
        HistoryLimits {
            tile_budget_bytes: 64 * 1024 * 1024,
            max_steps: 100,
            held_paint_budget_bytes,
        },
    );
}

fn resize_tip_to(s: &mut PaintSession, diameter_px: f32) {
    s.brush.geometry.max_diameter_px = diameter_px;
    s.resize_tip();
}

fn snapshot(gpu: &GpuContext, s: &PaintSession) -> Vec<Vec<u8>> {
    StateTexture::PAINT_STATE
        .iter()
        .map(|texture| s.canvas.read_bytes(gpu, *texture).unwrap())
        .collect()
}

fn undo_every_step(gpu: &GpuContext, s: &mut PaintSession) -> usize {
    let mut undone = 0;
    while without_wgpu_errors(gpu, || s.undo(gpu)) {
        undone += 1;
    }
    undone
}

#[test]
fn held_paint_past_its_budget_pushes_the_oldest_steps_out() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    let one_capture = one_capture_bytes(&gpu, &s);
    budget_held_paint(&gpu, &mut s, one_capture * 2);
    for row in 0..4 {
        stroke(&gpu, &mut s, 60.0 + 40.0 * f64::from(row), 40.0, 470.0);
    }
    assert!(s.history.held_paint_bytes() <= one_capture * 2);
    assert_eq!(undo_every_step(&gpu, &mut s), 2);
}

#[test]
fn a_load_makes_the_captures_of_earlier_strokes_stop_counting() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    let one_capture = one_capture_bytes(&gpu, &s);
    budget_held_paint(&gpu, &mut s, one_capture);
    stroke(&gpu, &mut s, UPPER_ROW, 40.0, 470.0);
    assert_eq!(s.history.held_paint_bytes(), one_capture);
    load(&gpu, &mut s, YELLOW);
    stroke(&gpu, &mut s, LOWER_ROW, 40.0, 470.0);
    assert_eq!(s.history.held_paint_bytes(), one_capture);
    assert_eq!(undo_every_step(&gpu, &mut s), 2);
}

#[test]
fn redoing_a_step_leaves_only_its_before_capture_counted() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    let one_capture = one_capture_bytes(&gpu, &s);
    stroke(&gpu, &mut s, UPPER_ROW, 40.0, 470.0);
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    assert_eq!(s.history.held_paint_bytes(), one_capture * 2);
    assert!(without_wgpu_errors(&gpu, || s.redo(&gpu)));
    assert_eq!(s.history.held_paint_bytes(), one_capture);
}

#[test]
fn a_stroke_that_touches_nothing_costs_no_undo_step_while_steps_to_redo_hold_captures() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    let one_capture = one_capture_bytes(&gpu, &s);
    budget_held_paint(&gpu, &mut s, one_capture * 3);
    stroke(&gpu, &mut s, UPPER_ROW, 40.0, 470.0);
    stroke(&gpu, &mut s, LOWER_ROW, 40.0, 470.0);
    let painted = snapshot(&gpu, &s);
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    assert_eq!(s.history.held_paint_bytes(), one_capture * 3);
    stroke(&gpu, &mut s, ROW_OFF_THE_CANVAS, 40.0, 470.0);
    assert!(s.history.held_paint_bytes() <= one_capture * 3);
    assert!(without_wgpu_errors(&gpu, || s.redo(&gpu)));
    assert!(snapshot(&gpu, &s) == painted);
    assert_eq!(undo_every_step(&gpu, &mut s), 2);
}

#[test]
fn a_capture_larger_than_the_whole_budget_leaves_the_step_to_undo_the_canvas_alone() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    let one_capture = one_capture_bytes(&gpu, &s);
    budget_held_paint(&gpu, &mut s, one_capture - 1);
    let blank = snapshot(&gpu, &s);
    stroke(&gpu, &mut s, UPPER_ROW, 40.0, 470.0);
    assert_eq!(s.history.held_paint_bytes(), 0);
    assert_eq!(undo_every_step(&gpu, &mut s), 1);
    assert!(snapshot(&gpu, &s) == blank);
}

#[test]
fn a_capture_larger_than_the_whole_budget_costs_no_earlier_step() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    resize_tip_to(&mut s, NARROW_DIAMETER_PX);
    let narrow_capture = one_capture_bytes(&gpu, &s);
    budget_held_paint(&gpu, &mut s, narrow_capture);
    let blank = snapshot(&gpu, &s);
    stroke(&gpu, &mut s, UPPER_ROW, 40.0, 470.0);
    assert_eq!(s.history.held_paint_bytes(), narrow_capture);
    resize_tip_to(&mut s, WIDE_DIAMETER_PX);
    assert!(one_capture_bytes(&gpu, &s) > narrow_capture);
    stroke(&gpu, &mut s, LOWER_ROW, 40.0, 470.0);
    assert_eq!(s.history.held_paint_bytes(), 0);
    assert_eq!(undo_every_step(&gpu, &mut s), 2);
    assert!(snapshot(&gpu, &s) == blank);
}
