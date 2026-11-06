pub mod support;

use cadmium_engine::canvas::TILE_SIZE;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use support::{spec, stroke, without_wgpu_errors};

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
