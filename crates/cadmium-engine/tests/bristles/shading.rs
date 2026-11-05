use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::Tool;

use crate::support::{drag, frame, line, presented_rgba, session};

const CANVAS_WIDTH: u32 = 512;
const CANVAS_HEIGHT: u32 = 256;
const STROKE_ROWS: [f64; 3] = [70.0, 128.0, 190.0];
const STROKE_FROM_X: f64 = 30.0;
const STROKE_TO_X: f64 = 480.0;
const BLENDER_ROW: f64 = 120.0;
const STROKE_PRESSURE: f32 = 0.9;
const STROKE_TILT_DEG: f32 = 20.0;

fn picture(gpu: &GpuContext, s: &PaintSession) -> Vec<u8> {
    presented_rgba(gpu, s, CANVAS_WIDTH, CANVAS_HEIGHT)
}

fn fully_reshaded(gpu: &GpuContext, s: &mut PaintSession) -> Vec<u8> {
    s.canvas.dirty.mark_all();
    frame(gpu, s);
    picture(gpu, s)
}

fn stroke_across(gpu: &GpuContext, s: &mut PaintSession, y: f64) {
    let samples = line(
        y,
        STROKE_FROM_X,
        STROKE_TO_X,
        STROKE_PRESSURE,
        STROKE_TILT_DEG,
    );
    drag(gpu, s, &samples);
}

#[test]
fn bristle_strokes_show_the_picture_that_shading_every_tile_would() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    for y in STROKE_ROWS {
        stroke_across(&gpu, &mut s, y);
        assert_eq!(
            picture(&gpu, &s),
            fully_reshaded(&gpu, &mut s),
            "brush row {y}"
        );
    }
    s.brush.tool = Tool::Blender;
    s.clean_brush(&gpu);
    stroke_across(&gpu, &mut s, BLENDER_ROW);
    assert_eq!(picture(&gpu, &s), fully_reshaded(&gpu, &mut s), "blender");
}
