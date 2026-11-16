//! Behaviours the bristle physics must show whatever the model, from the literature the
//! XPBD rewrite is grounded in (docs/superpowers/specs/2026-11-16-xpbd-bristles-design.md,
//! section 5). A test today's model fails is ignored and names the phase of #62 that must
//! make it pass.

use cadmium_engine::brush::factory_brushes;
use cadmium_engine::canvas::StateTexture;
use cadmium_engine::engine::BristleSnapshot;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::support::{frame, session};

const ROW_Y: f64 = 128.0;
const FROM_X: f64 = 100.0;
const LENGTH_PX: f64 = 240.0;
const PEN_SPEED_PX_PER_S: f64 = 400.0;
const MS_PER_S: f64 = 1000.0;
const TILT_DEG: f32 = 20.0;
const STEADY_PRESSURE: f32 = 0.6;
const COARSE_RATE_HZ: f64 = 120.0;
const FINE_RATE_HZ: f64 = 480.0;
const MAX_RELATIVE_GAP: f64 = 0.1;
const VISIBLE_UM: f32 = 0.5;
/// Columns this far from either end of the line are left out of the mark's width.
const END_MARGIN_PX: usize = 40;
const SPLAY_PRESSURES: [f32; 4] = [0.15, 0.35, 0.6, 0.9];
const DENSITY_SCALE: f32 = 2.0;

const LIGHT_TOUCH_PRESSURE: f32 = 0.04;
const LIGHT_TOUCH_TILT_DEG: f32 = 25.0;
const LIGHT_TOUCH_DIAMETER_PX: f32 = 24.0;
const LIGHT_TOUCH_SAMPLES: u32 = 12;
const LIGHT_TOUCH_STEP_PX: f64 = 2.0;
const LIGHT_TOUCH_INTERVAL_MS: u32 = 6;
const POINTED_BRUSHES: [&str; 2] = ["Round · Synthetic", "Round · Kolinsky Sable"];

/// A straight line at `pressure`, sampled at `rate_hz` while the pen moves at a steady speed.
fn line_at_rate(pressure: f32, rate_hz: f64) -> Vec<StrokeSample> {
    let seconds = LENGTH_PX / PEN_SPEED_PX_PER_S;
    let samples = (seconds * rate_hz).round() as u32;
    (0..=samples)
        .map(|index| {
            let t = f64::from(index) / f64::from(samples);
            StrokeSample {
                x: FROM_X + LENGTH_PX * t,
                y: ROW_Y,
                pressure,
                tilt_x_deg: TILT_DEG,
                tilt_y_deg: 0.0,
                time_ms: (t * seconds * MS_PER_S).round() as u32,
            }
        })
        .collect()
}

/// Feeds `samples` as one stroke, one frame per sample, the way the app feeds the pen,
/// and leaves the pen down.
fn press(gpu: &GpuContext, s: &mut PaintSession, samples: &[StrokeSample]) {
    for (index, sample) in samples.iter().enumerate() {
        if index == 0 {
            s.begin_stroke(*sample);
        } else {
            s.add_samples(&[*sample]);
        }
        frame(gpu, s);
    }
}

/// Paints `samples` as one stroke and lifts the pen.
fn paint(gpu: &GpuContext, s: &mut PaintSession, samples: &[StrokeSample]) {
    press(gpu, s, samples);
    s.end_stroke();
    frame(gpu, s);
}

/// Mean count of visibly painted pixels per column, away from the line's ends.
fn mean_mark_width_px(gpu: &GpuContext, s: &PaintSession) -> f64 {
    let volume = s.canvas.read_f32(gpu, StateTexture::WetVol).unwrap();
    let width = s.canvas.spec.width as usize;
    let first = FROM_X as usize + END_MARGIN_PX;
    let last = (FROM_X + LENGTH_PX) as usize - END_MARGIN_PX;
    let total: usize = (first..=last)
        .map(|x| {
            let column = volume.iter().skip(x).step_by(width);
            column.filter(|&&v| v > VISIBLE_UM).count()
        })
        .sum();
    total as f64 / (last - first + 1) as f64
}

/// Mean distance of the tips from the roots' centre, in the canvas plane.
fn mean_splay_px(snapshot: &BristleSnapshot) -> f32 {
    let count = snapshot.roots_px.len() as f32;
    let centre = snapshot.roots_px.iter().fold([0.0, 0.0], |sum, root| {
        [sum[0] + root[0] / count, sum[1] + root[1] / count]
    });
    let total: f32 = snapshot
        .tips_px
        .iter()
        .map(|tip| (tip[0] - centre[0]).hypot(tip[1] - centre[1]))
        .sum();
    total / count
}

/// Spread of the tips across the stroke (along y), from the outermost tip on each side.
fn tip_spread_across_px(snapshot: &BristleSnapshot) -> f32 {
    let (low, high) = snapshot
        .tips_px
        .iter()
        .fold((f32::MAX, f32::MIN), |(low, high), tip| {
            (low.min(tip[1]), high.max(tip[1]))
        });
    high - low
}

fn relative_gap(a: f64, b: f64) -> f64 {
    (a - b).abs() / a.abs().max(b.abs()).max(f64::MIN_POSITIVE)
}

fn splay_while_pressed(gpu: &GpuContext, pressure: f32) -> f32 {
    let mut s = session(gpu);
    press(gpu, &mut s, &line_at_rate(pressure, COARSE_RATE_HZ));
    mean_splay_px(&s.engine.bristle_snapshot(gpu).unwrap())
}

fn mark_width_px(gpu: &GpuContext, rate_hz: f64, density_scale: f32) -> f64 {
    let mut s = session(gpu);
    s.brush.head.density_per_px *= density_scale;
    s.reload_brush(gpu);
    paint(gpu, &mut s, &line_at_rate(STEADY_PRESSURE, rate_hz));
    mean_mark_width_px(gpu, &s)
}

fn light_touch_sample(index: u32) -> StrokeSample {
    StrokeSample {
        x: FROM_X + LIGHT_TOUCH_STEP_PX * f64::from(index),
        y: ROW_Y,
        pressure: LIGHT_TOUCH_PRESSURE,
        tilt_x_deg: LIGHT_TOUCH_TILT_DEG,
        tilt_y_deg: 0.0,
        time_ms: index * LIGHT_TOUCH_INTERVAL_MS,
    }
}

/// The tips' spread across the stroke after its first dab and after its last.
fn light_touch_spreads(gpu: &GpuContext, s: &mut PaintSession) -> (f32, f32) {
    s.limit_dabs_per_frame(Some(1));
    s.begin_stroke(light_touch_sample(0));
    frame(gpu, s);
    let first = tip_spread_across_px(&s.engine.bristle_snapshot(gpu).unwrap());
    for index in 1..LIGHT_TOUCH_SAMPLES {
        s.add_samples(&[light_touch_sample(index)]);
        while s.has_queued_dabs() {
            frame(gpu, s);
        }
    }
    let last = tip_spread_across_px(&s.engine.bristle_snapshot(gpu).unwrap());
    (first, last)
}

#[test]
fn harder_pressure_splays_the_tips_wider() {
    let gpu = GpuContext::new().unwrap();
    let splays: Vec<f32> = SPLAY_PRESSURES
        .iter()
        .map(|&pressure| splay_while_pressed(&gpu, pressure))
        .collect();
    assert!(
        splays.windows(2).all(|pair| pair[1] > pair[0]),
        "splay at pressures {SPLAY_PRESSURES:?}: {splays:?}"
    );
}

#[test]
fn the_mark_does_not_depend_on_how_often_the_pen_is_sampled() {
    let gpu = GpuContext::new().unwrap();
    let coarse = mark_width_px(&gpu, COARSE_RATE_HZ, 1.0);
    let fine = mark_width_px(&gpu, FINE_RATE_HZ, 1.0);
    assert!(
        relative_gap(coarse, fine) <= MAX_RELATIVE_GAP,
        "mark width {coarse} px at {COARSE_RATE_HZ} Hz, {fine} px at {FINE_RATE_HZ} Hz"
    );
}

#[test]
fn the_mark_does_not_depend_on_how_many_hairs_are_simulated() {
    let gpu = GpuContext::new().unwrap();
    let base = mark_width_px(&gpu, COARSE_RATE_HZ, 1.0);
    let denser = mark_width_px(&gpu, COARSE_RATE_HZ, DENSITY_SCALE);
    assert!(
        relative_gap(base, denser) <= MAX_RELATIVE_GAP,
        "mark width {base} px, {denser} px with {DENSITY_SCALE}x hair density"
    );
}

#[test]
#[ignore = "phase 1 of #62: each stroke hangs the hairs into one point and the first clump step flings them apart"]
fn a_light_first_touch_lands_the_wet_point_instead_of_splaying_the_hairs() {
    let gpu = GpuContext::new().unwrap();
    for brush in factory_brushes()
        .into_iter()
        .filter(|brush| POINTED_BRUSHES.contains(&brush.name.as_str()))
    {
        let mut s = session(&gpu);
        s.brush.wear(&brush);
        s.brush.geometry.max_diameter_px = LIGHT_TOUCH_DIAMETER_PX;
        s.reload_brush(&gpu);
        s.flush(&gpu);
        let (first_px, later_px) = light_touch_spreads(&gpu, &mut s);
        assert!(
            first_px <= later_px,
            "{}: the first dab splays the tips {first_px} px across, the stroke then holds them to {later_px} px",
            brush.name
        );
    }
}
