#[path = "../support/mod.rs"]
pub mod support;

mod knife;
mod spreading;

use cadmium_engine::{
    canvas::{PaintTotals, StateTexture},
    gpu::GpuContext,
    session::PaintSession,
    stroke::StrokeSample,
};

use crate::support::line;

const CANVAS_WIDTH: usize = 512;
const SECTION_COLUMNS: std::ops::Range<usize> = 150..350;
const CONSERVATION_TOLERANCE: f64 = 1e-6;

fn column(x: f64, from: f64, to: f64, pressure: f32, tilt_x_deg: f32) -> Vec<StrokeSample> {
    line(0.0, from, to, pressure, tilt_x_deg)
        .into_iter()
        .map(|sample| StrokeSample {
            x,
            y: sample.x,
            ..sample
        })
        .collect()
}

fn total(gpu: &GpuContext, s: &PaintSession) -> PaintTotals {
    let c = s.canvas.wet_totals(gpu).unwrap();
    let mut t = c;
    for tool in [
        s.engine.brush_totals(gpu).unwrap(),
        s.engine.knife_totals(gpu).unwrap(),
    ] {
        t.volume += tool.volume;
        for i in 0..8 {
            t.pigment[i] += tool.pigment[i];
        }
    }
    t
}

fn relative_drift(before: &PaintTotals, after: &PaintTotals) -> (f64, Vec<f64>) {
    (
        (after.volume - before.volume) / before.volume,
        (0..3)
            .map(|i| (after.pigment[i] - before.pigment[i]) / before.volume)
            .collect(),
    )
}

fn cross_section(gpu: &GpuContext, s: &PaintSession, rows: std::ops::Range<usize>) -> Vec<f32> {
    let vol = s.canvas.read_f32(gpu, StateTexture::WetVol).unwrap();
    rows.map(|y| {
        let sum: f32 = SECTION_COLUMNS.map(|x| vol[y * CANVAS_WIDTH + x]).sum();
        sum / SECTION_COLUMNS.len() as f32
    })
    .collect()
}
