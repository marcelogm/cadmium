use cadmium_engine::engine::PaintEngine;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::support::{frame, line, session};

const FULL_EXCHANGE: f32 = 1.0;
const FLOAT_ROUNDED_FLOWS: [&str; 5] = [
    "refill_rate",
    "advection",
    "squeeze",
    "ferrule_press_share",
    "slump_rate",
];
const SAMPLES_PER_FRAME: usize = 4;
const STROKE_ROWS_PX: [f64; 3] = [120.0, 128.0, 136.0];
const STROKE_PRESSURE: f32 = 0.9;
const STROKE_TILT_DEG: f32 = 20.0;
const WIDE_DIAMETER_PX: f32 = 200.0;
const STRONGEST_WICKING_SHARE: f32 = 0.25;

fn total_volume(gpu: &GpuContext, s: &PaintSession) -> f64 {
    s.canvas.wet_totals(gpu).unwrap().volume + s.engine.brush_totals(gpu).unwrap().volume
}

fn lowest_tip_volume_um(gpu: &GpuContext, s: &PaintSession) -> f32 {
    s.engine
        .tip_volumes_um(gpu)
        .unwrap()
        .into_iter()
        .fold(f32::INFINITY, f32::min)
}

fn drag_watching_the_tip(gpu: &GpuContext, s: &mut PaintSession, samples: &[StrokeSample]) -> f32 {
    let mut lowest_um = f32::INFINITY;
    for (index, sample) in samples.iter().enumerate() {
        if index == 0 {
            s.begin_stroke(*sample);
        } else {
            s.add_samples(&[*sample]);
        }
        if index % SAMPLES_PER_FRAME == 0 {
            frame(gpu, s);
            lowest_um = lowest_um.min(lowest_tip_volume_um(gpu, s));
        }
    }
    s.end_stroke();
    frame(gpu, s);
    lowest_um.min(lowest_tip_volume_um(gpu, s))
}

#[test]
fn a_full_exchange_conserves_paint_exactly_and_never_overdraws_a_tip_texel() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.engine
        .params_mut()
        .set("transfer_fraction", FULL_EXCHANGE);
    for key in FLOAT_ROUNDED_FLOWS {
        s.engine.params_mut().set(key, 0.0);
    }
    s.reload_brush(&gpu);
    let before = total_volume(&gpu, &s);
    let mut lowest_um = f32::INFINITY;
    for (index, row) in STROKE_ROWS_PX.into_iter().enumerate() {
        let (from, to) = if index % 2 == 0 {
            (40.0, 470.0)
        } else {
            (470.0, 40.0)
        };
        let samples = line(row, from, to, STROKE_PRESSURE, STROKE_TILT_DEG);
        lowest_um = lowest_um.min(drag_watching_the_tip(&gpu, &mut s, &samples));
    }
    let after = total_volume(&gpu, &s);
    eprintln!(
        "total volume {before} -> {after} ({:e}); lowest tip texel {lowest_um:e} µm",
        after - before
    );
    assert!(
        lowest_um >= 0.0,
        "a tip texel went down to {lowest_um:e} µm"
    );
    assert_eq!(after, before, "a full exchange changed the total volume");
}

#[test]
fn strong_wicking_between_widely_spaced_dabs_never_overdraws_a_tip_texel() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.geometry.max_diameter_px = WIDE_DIAMETER_PX;
    s.brush.hair.wicking = STRONGEST_WICKING_SHARE;
    s.reload_brush(&gpu);
    let samples = line(128.0, 40.0, 470.0, STROKE_PRESSURE, STROKE_TILT_DEG);
    let lowest_um = drag_watching_the_tip(&gpu, &mut s, &samples);
    eprintln!("lowest tip texel {lowest_um:e} um");
    assert!(
        lowest_um >= 0.0,
        "a tip texel went down to {lowest_um:e} um"
    );
}
