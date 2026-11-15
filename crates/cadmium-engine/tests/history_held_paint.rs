pub mod support;

use cadmium_engine::{
    canvas::PaintTotals,
    engine::FULL_DIP,
    gpu::GpuContext,
    pigments::{MAX_SLOTS, Mixture},
    session::PaintSession,
    stroke::Tool,
};
use support::{drag, frame, line, session, stroke, without_wgpu_errors};

const BLUE: usize = 0;
const YELLOW: usize = 1;
const SLOT_COUNT: usize = 3;
const DIP_THICKNESS_UM: f32 = 40.0;
const HALF_DIP: f32 = 0.5;
const LAID_KNIFE_TILT_DEG: f32 = 60.0;
const DABS_PER_FRAME: usize = 3;
const FRAMES_TO_DRAIN: usize = 10_000;
const UPPER_ROW: f64 = 64.0;
const LOWER_ROW: f64 = 192.0;

fn blue_of(volume_um: f32) -> Mixture {
    let mut fractions = [0.0; MAX_SLOTS];
    fractions[BLUE] = 1.0;
    Mixture {
        volume_um,
        fractions,
    }
}

fn load(gpu: &GpuContext, s: &mut PaintSession, slot: usize) {
    s.brush.recipe = vec![(slot, 1.0)];
    without_wgpu_errors(gpu, || s.reload_brush(gpu));
}

fn held(gpu: &GpuContext, s: &PaintSession) -> (Vec<f32>, PaintTotals, PaintTotals) {
    (
        s.engine.tip_volumes_um(gpu).unwrap(),
        s.engine.brush_totals(gpu).unwrap(),
        s.engine.knife_totals(gpu).unwrap(),
    )
}

fn queue_stroke(s: &mut PaintSession, row: f64) {
    let samples = line(row, 40.0, 470.0, 0.8, 20.0);
    s.begin_stroke(samples[0]);
    s.add_samples(&samples[1..]);
    s.end_stroke();
}

fn drain(gpu: &GpuContext, s: &mut PaintSession) {
    for _ in 0..FRAMES_TO_DRAIN {
        if !s.has_queued_dabs() {
            return;
        }
        frame(gpu, s);
    }
    panic!("the queued dabs never got painted");
}

#[test]
fn held_paint_restored_after_a_stroke_is_the_paint_held_before_it() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    let before = held(&gpu, &s);
    let captured = s.engine.capture_held_paint(&gpu).unwrap();
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    assert!(held(&gpu, &s) != before);
    without_wgpu_errors(&gpu, || s.engine.restore_held_paint(&gpu, &captured));
    assert!(held(&gpu, &s) == before);
}

#[test]
fn a_capture_holds_only_the_part_of_the_tip_the_brush_uses() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.geometry.max_diameter_px = 40.0;
    s.resize_tip();
    let small = s.engine.capture_held_paint(&gpu).unwrap().bytes();
    s.brush.geometry.max_diameter_px = 400.0;
    s.resize_tip();
    let large = s.engine.capture_held_paint(&gpu).unwrap().bytes();
    assert!(small * 4 < large, "{small} vs {large}");
}

#[test]
fn loading_cleaning_and_resizing_each_start_a_new_load_epoch_and_a_stroke_does_not() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let fresh = s.engine.load_epoch();
    load(&gpu, &mut s, BLUE);
    let loaded = s.engine.load_epoch();
    s.clean_brush(&gpu);
    let cleaned = s.engine.load_epoch();
    s.brush.geometry.max_diameter_px = 90.0;
    s.resize_tip();
    let resized = s.engine.load_epoch();
    assert!(fresh < loaded && loaded < cleaned && cleaned < resized);
    s.resize_tip();
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    assert_eq!(s.engine.load_epoch(), resized);
}

#[test]
fn every_dip_and_knife_cleaning_starts_a_new_load_epoch_and_an_empty_dip_or_a_restore_does_not() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let paint = blue_of(DIP_THICKNESS_UM);
    let mut epochs = vec![s.engine.load_epoch()];
    without_wgpu_errors(&gpu, || {
        s.engine.dip(&gpu, &paint, HALF_DIP);
        epochs.push(s.engine.load_epoch());
        s.engine.restore_reserve(&gpu, &paint);
        epochs.push(s.engine.load_epoch());
        s.engine.dip_knife(&gpu, &paint, HALF_DIP).unwrap();
        epochs.push(s.engine.load_epoch());
        s.engine.clean_knife(&gpu);
        epochs.push(s.engine.load_epoch());
    });
    assert!(epochs.is_sorted_by(|a, b| a < b), "{epochs:?}");
    let settled = s.engine.load_epoch();
    let captured = s.engine.capture_held_paint(&gpu).unwrap();
    without_wgpu_errors(&gpu, || {
        s.engine.dip(&gpu, &blue_of(0.0), HALF_DIP);
        s.engine.restore_reserve(&gpu, &blue_of(0.0));
        s.engine.dip_knife(&gpu, &paint, 0.0).unwrap();
        s.engine.restore_held_paint(&gpu, &captured);
    });
    assert_eq!(s.engine.load_epoch(), settled);
}

#[test]
fn undoing_a_stroke_gives_the_brush_back_the_paint_it_laid() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    let before = held(&gpu, &s);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    assert!(held(&gpu, &s) == before);
}

#[test]
fn undoing_a_stroke_through_wet_paint_takes_its_dirt_off_the_brush() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, YELLOW);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    load(&gpu, &mut s, BLUE);
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    assert!(s.engine.brush_totals(&gpu).unwrap().pigment[YELLOW] > 0.0);
    s.undo(&gpu);
    assert_eq!(s.engine.brush_totals(&gpu).unwrap().pigment[YELLOW], 0.0);
}

#[test]
fn redoing_a_stroke_leaves_the_brush_as_the_stroke_left_it() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let after = held(&gpu, &s);
    s.undo(&gpu);
    assert!(without_wgpu_errors(&gpu, || s.redo(&gpu)));
    assert!(held(&gpu, &s) == after);
}

#[test]
fn a_redone_stroke_undone_and_redone_again_moves_the_brush_both_ways() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    let before = held(&gpu, &s);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let after = held(&gpu, &s);
    for _ in 0..2 {
        assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
        assert!(held(&gpu, &s) == before);
        assert!(without_wgpu_errors(&gpu, || s.redo(&gpu)));
        assert!(held(&gpu, &s) == after);
    }
}

#[test]
fn a_brush_loaded_after_a_stroke_keeps_that_load_when_the_stroke_is_undone() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    load(&gpu, &mut s, YELLOW);
    let loaded = held(&gpu, &s);
    assert!(s.undo(&gpu));
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
    assert!(held(&gpu, &s) == loaded);
    assert!(s.redo(&gpu));
    assert!(held(&gpu, &s) == loaded);
}

#[test]
fn a_load_queued_behind_a_stroke_survives_undoing_that_stroke() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    s.limit_dabs_per_frame(Some(DABS_PER_FRAME));
    queue_stroke(&mut s, UPPER_ROW);
    load(&gpu, &mut s, YELLOW);
    assert!(s.has_queued_dabs());
    drain(&gpu, &mut s);
    let loaded = held(&gpu, &s);
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
    assert!(held(&gpu, &s) == loaded);
}

#[test]
fn a_stroke_queued_behind_a_load_gives_that_load_back_when_undone() {
    let gpu = GpuContext::new().unwrap();
    let mut fresh = session(&gpu);
    load(&gpu, &mut fresh, YELLOW);
    let full_yellow = held(&gpu, &fresh);
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    s.limit_dabs_per_frame(Some(DABS_PER_FRAME));
    queue_stroke(&mut s, UPPER_ROW);
    load(&gpu, &mut s, YELLOW);
    queue_stroke(&mut s, LOWER_ROW);
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    assert!(held(&gpu, &s) == full_yellow);
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
    assert!(held(&gpu, &s) == full_yellow);
}

#[test]
fn undoing_a_knife_stroke_gives_the_knife_back_its_load() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.tool = Tool::Knife;
    s.brush.recipe = vec![(BLUE, 1.0)];
    let mixture = s.brush.reservoir_mixture(SLOT_COUNT);
    s.dip(&gpu, &mixture, FULL_DIP).unwrap();
    let before = held(&gpu, &s);
    drag(
        &gpu,
        &mut s,
        &line(128.0, 40.0, 470.0, 0.8, LAID_KNIFE_TILT_DEG),
    );
    assert!(held(&gpu, &s) != before);
    s.undo(&gpu);
    assert!(held(&gpu, &s) == before);
}

#[test]
fn a_pencil_or_rag_stroke_holds_no_tool_paint_and_leaves_the_brush_alone() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load(&gpu, &mut s, BLUE);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    s.history.clear();
    let before = held(&gpu, &s);
    for tool in [Tool::Pencil, Tool::Rag] {
        s.brush.tool = tool;
        stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
        assert_eq!(s.history.held_paint_bytes(), 0, "{tool:?}");
        assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)), "{tool:?}");
        assert!(held(&gpu, &s) == before, "{tool:?}");
    }
}
