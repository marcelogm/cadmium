pub mod support;

use cadmium_engine::canvas::{Canvas, TILE_SIZE};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use support::{drag, line, presented_rgba, spec, stroke, without_wgpu_errors};

fn headless(gpu: &GpuContext) -> PaintSession {
    without_wgpu_errors(gpu, || PaintSession::headless(gpu, spec(512, 256)).unwrap())
}

#[test]
fn a_headless_session_paints_without_recording_undo() {
    let gpu = GpuContext::new().unwrap();
    let mut s = headless(&gpu);
    without_wgpu_errors(&gpu, || stroke(&gpu, &mut s, 128.0, 40.0, 470.0));
    assert!(s.canvas.wet_totals(&gpu).unwrap().volume > 0.0);
    assert!(!s.history.is_enabled());
    assert_eq!(s.history.slot_capacity(), 0);
    assert!(!s.undo(&gpu));
}

#[test]
fn a_headless_session_keeps_only_a_one_tile_tray() {
    let gpu = GpuContext::new().unwrap();
    let s = headless(&gpu);
    assert_eq!(
        (s.tray.spec.width, s.tray.spec.height),
        (TILE_SIZE, TILE_SIZE)
    );
    assert_eq!(s.canvas.spec.width, 512);
}

/// Paints a light stroke at `y` with a freshly loaded brush.
fn stroke_loaded(gpu: &GpuContext, s: &mut PaintSession, load: f32, y: f64, pressure: f32) {
    s.brush.load = load;
    s.reload_brush(gpu);
    without_wgpu_errors(gpu, || drag(gpu, s, &line(y, 40.0, 300.0, pressure, 20.0)));
}

/// The first contact of a stroke measures the surface afresh: nothing of the
/// contact height the last stroke ended on carries over, not even in rounding.
#[test]
fn a_stroke_remembers_nothing_of_the_strokes_before_it() {
    let gpu = GpuContext::new().unwrap();
    let mut used = headless(&gpu);
    stroke_loaded(&gpu, &mut used, 1.0, 180.0, 0.9);
    used.canvas = Canvas::new(&gpu, &used.db, spec(512, 256)).unwrap();
    stroke_loaded(&gpu, &mut used, 0.2, 60.0, 0.3);
    let mut fresh = headless(&gpu);
    stroke_loaded(&gpu, &mut fresh, 0.2, 60.0, 0.3);
    let same = presented_rgba(&gpu, &used, 512, 256) == presented_rgba(&gpu, &fresh, 512, 256);
    assert!(same, "the earlier stroke left a trace");
}
