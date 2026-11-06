pub mod support;

use cadmium_engine::canvas::{Canvas, TILE_SIZE};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::Tool;
use support::{drag, line, presented_rgba, spec, stroke, without_wgpu_errors};

const KNIFE_DIP_SLOTS: usize = 3;

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
    s.brush.tool = Tool::Brush;
    s.brush.load = load;
    s.reload_brush(gpu);
    without_wgpu_errors(gpu, || drag(gpu, s, &line(y, 40.0, 300.0, pressure, 20.0)));
}

/// Paints `first` on one canvas, swaps in a blank canvas and paints `second`:
/// the result must match a fresh session that only ever painted `second`, so
/// nothing a stroke leaves in the engine (contact height, blade rest, bristle
/// or load state) leaks into the next one, not even in rounding. The rounding
/// leak of a rest height only shows on drivers that lower `mix` as
/// `a + t * (b - a)`, such as radv; CI's lavapipe gives `mix(a, b, 1.0) == b`
/// exactly, so there these pass with or without the first-rest replace.
fn assert_second_stroke_matches_a_fresh_session(
    first: impl Fn(&GpuContext, &mut PaintSession),
    second: impl Fn(&GpuContext, &mut PaintSession),
) {
    let gpu = GpuContext::new().unwrap();
    let mut used = headless(&gpu);
    first(&gpu, &mut used);
    used.canvas = Canvas::new(&gpu, &used.db, spec(512, 256)).unwrap();
    second(&gpu, &mut used);
    let mut fresh = headless(&gpu);
    second(&gpu, &mut fresh);
    let same = presented_rgba(&gpu, &used, 512, 256) == presented_rgba(&gpu, &fresh, 512, 256);
    assert!(same, "the earlier stroke left a trace");
}

#[test]
fn a_stroke_remembers_nothing_of_the_strokes_before_it() {
    assert_second_stroke_matches_a_fresh_session(
        |gpu, s| stroke_loaded(gpu, s, 1.0, 180.0, 0.9),
        |gpu, s| stroke_loaded(gpu, s, 0.2, 60.0, 0.3),
    );
}

/// Pulls a freshly dipped, laid knife along the row at `y`.
fn knife_stroke(gpu: &GpuContext, s: &mut PaintSession, y: f64, pressure: f32) {
    s.brush.tool = Tool::Knife;
    s.engine.clean_knife(gpu);
    let mixture = s.brush.reservoir_mixture(KNIFE_DIP_SLOTS);
    s.dip(gpu, &mixture, 1.0).unwrap();
    without_wgpu_errors(gpu, || drag(gpu, s, &line(y, 40.0, 300.0, pressure, 60.0)));
}

/// The blade rest takes the same first-rest replace as the brush's contact
/// height. Without it radv does leak a rounding of the last stroke's rest into
/// the blade, but the knife's quantized transfers absorb it, so this catches
/// only a first rest that follows the last stroke's rest outright; the
/// placement unit tests pin the first-rest flag itself.
#[test]
fn a_knife_stroke_remembers_nothing_of_the_strokes_before_it() {
    assert_second_stroke_matches_a_fresh_session(
        |gpu, s| {
            stroke_loaded(gpu, s, 1.0, 180.0, 0.9);
            without_wgpu_errors(gpu, || s.dry(gpu));
            knife_stroke(gpu, s, 180.0, 0.9);
        },
        |gpu, s| knife_stroke(gpu, s, 60.0, 0.3),
    );
}
