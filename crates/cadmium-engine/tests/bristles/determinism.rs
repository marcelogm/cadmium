use cadmium_engine::canvas::StateTexture;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::Tool;

use crate::support::{drag, line, session};

const WET_STATE: [StateTexture; 3] = [
    StateTexture::WetVol,
    StateTexture::WetPig0,
    StateTexture::WetPig1,
];

fn painted_twice(gpu: &GpuContext, s: &mut PaintSession) {
    drag(gpu, s, &line(120.0, 60.0, 450.0, 0.9, 20.0));
    s.brush.tool = Tool::Blender;
    drag(gpu, s, &line(130.0, 450.0, 60.0, 0.7, 0.0));
}

fn wet_state_and_brush(gpu: &GpuContext, s: &PaintSession) -> (Vec<Vec<u8>>, Vec<u64>) {
    let canvas = WET_STATE
        .iter()
        .map(|&which| s.canvas.read_bytes(gpu, which).unwrap())
        .collect();
    let totals = s.engine.brush_totals(gpu).unwrap();
    let brush = std::iter::once(totals.volume)
        .chain(totals.pigment)
        .map(f64::to_bits)
        .collect();
    (canvas, brush)
}

#[test]
fn a_stroke_repeated_lays_bit_identical_paint_and_leaves_a_bit_identical_brush() {
    let gpu = GpuContext::new().unwrap();
    let mut first = session(&gpu);
    let mut second = session(&gpu);
    painted_twice(&gpu, &mut first);
    painted_twice(&gpu, &mut second);
    let (first_canvas, first_brush) = wet_state_and_brush(&gpu, &first);
    let (second_canvas, second_brush) = wet_state_and_brush(&gpu, &second);
    assert!(first_canvas.iter().flatten().any(|&byte| byte != 0));
    assert!(
        first_canvas == second_canvas,
        "the same strokes laid different paint"
    );
    assert!(
        first_brush == second_brush,
        "the same strokes left the brush differently loaded"
    );
}
