use cadmium_engine::brush::HairSpec;
use cadmium_engine::engine::{BristleSnapshot, PaintEngine};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::support::{frame, session};

const FERRULE_RADIUS_PX: f32 = 20.0;
const START_X: f64 = 120.0;
const CENTRE_Y: f64 = 128.0;
const STEP_PX: f64 = 2.0;
const SAMPLE_INTERVAL_MS: u32 = 8;
const STROKE_SAMPLES: u32 = 40;
const TURN_SAMPLES: u32 = 3;
const PRESSURE: f32 = 0.8;
const HARD_PRESSURE: f32 = 1.0;
const TRAIL_SHARE: f32 = 0.2;
const MEANINGFUL_LAG_SHARE: f32 = 0.1;
const TICK_MS: u32 = 16;
const SETTLE_MS: u32 = 1500;
const SPRUNG_BACK_PX: f32 = 0.5;
const TIMELESS_PRESSES: u32 = 12;
const LIGHT_PRESSURE: f32 = 0.1;
const MAX_RADIAL_SET_LEAN: f32 = 5e-4;

#[derive(Clone, Copy)]
struct Heading {
    x: f64,
    y: f64,
}

const RIGHTWARD: Heading = Heading { x: 1.0, y: 0.0 };
const DOWNWARD: Heading = Heading { x: 0.0, y: 1.0 };

fn pressed(x: f64, y: f64, pressure: f32, time_ms: u32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms,
    }
}

fn nominal_length_px(s: &PaintSession) -> f32 {
    s.brush.head.length * FERRULE_RADIUS_PX
}

fn handle(snapshot: &BristleSnapshot) -> [f32; 2] {
    centroid(snapshot.roots_px.iter().map(|root| [root[0], root[1]]))
}

fn tip_centroid(snapshot: &BristleSnapshot) -> [f32; 2] {
    centroid(snapshot.tips_px.iter().map(|tip| [tip[0], tip[1]]))
}

fn centroid(points: impl ExactSizeIterator<Item = [f32; 2]>) -> [f32; 2] {
    let count = points.len() as f32;
    points.fold([0.0, 0.0], |sum, point| {
        [sum[0] + point[0] / count, sum[1] + point[1] / count]
    })
}

struct Stroke {
    at: [f64; 2],
    time_ms: u32,
    pressure: f32,
}

impl Stroke {
    fn begin(gpu: &GpuContext, s: &mut PaintSession, pressure: f32) -> Stroke {
        let stroke = Stroke {
            at: [START_X, CENTRE_Y],
            time_ms: 0,
            pressure,
        };
        s.begin_stroke(gpu, stroke.sample());
        stroke
    }

    fn sample(&self) -> StrokeSample {
        pressed(self.at[0], self.at[1], self.pressure, self.time_ms)
    }

    fn step(
        &mut self,
        gpu: &GpuContext,
        s: &mut PaintSession,
        heading: Heading,
    ) -> BristleSnapshot {
        self.at = [
            self.at[0] + STEP_PX * heading.x,
            self.at[1] + STEP_PX * heading.y,
        ];
        self.time_ms += SAMPLE_INTERVAL_MS;
        s.add_samples(&[self.sample()]);
        frame(gpu, s);
        s.engine.bristle_snapshot(gpu).unwrap()
    }

    fn run(&mut self, gpu: &GpuContext, s: &mut PaintSession, steps: u32) -> BristleSnapshot {
        let mut last = None;
        for _ in 0..steps {
            last = Some(self.step(gpu, s, RIGHTWARD));
        }
        last.expect("a stroke takes at least one step")
    }
}

fn tip_offsets(snapshot: &BristleSnapshot) -> Vec<[f32; 2]> {
    snapshot
        .tips_px
        .iter()
        .zip(&snapshot.roots_px)
        .map(|(tip, root)| [tip[0] - root[0], tip[1] - root[1]])
        .collect()
}

fn loose_session(gpu: &GpuContext, plasticity: f32) -> PaintSession {
    let mut s = session(gpu);
    s.brush.hair.clumping = 0.0;
    s.brush.hair.wet_point = 0.0;
    s.brush.hair.bend_memory = plasticity;
    s
}

fn settled_rest_offsets(gpu: &GpuContext, plasticity: f32) -> Vec<[f32; 2]> {
    let mut s = loose_session(gpu, plasticity);
    s.begin_stroke(gpu, pressed(START_X, CENTRE_Y, 0.0, 0));
    s.end_stroke();
    frame(gpu, &mut s);
    tip_offsets(&settle(gpu, &mut s, 0))
}

fn mean_bend_px(snapshot: &BristleSnapshot, resting: &[[f32; 2]]) -> f32 {
    let offsets = tip_offsets(snapshot);
    assert_eq!(offsets.len(), resting.len());
    let total: f32 = offsets
        .iter()
        .zip(resting)
        .map(|(now, rest)| (now[0] - rest[0]).hypot(now[1] - rest[1]))
        .sum();
    total / offsets.len() as f32
}

fn settle(gpu: &GpuContext, s: &mut PaintSession, from_ms: u32) -> BristleSnapshot {
    for tick in 1..=SETTLE_MS / TICK_MS {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let now_ms = (from_ms + tick * TICK_MS) as f32;
        s.engine
            .settle_bristles(gpu, &s.canvas, now_ms, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
    }
    s.engine.bristle_snapshot(gpu).unwrap()
}

struct HardStroke {
    during: BristleSnapshot,
    settled: BristleSnapshot,
    resting: Vec<[f32; 2]>,
}

fn hard_stroke_then_lift(gpu: &GpuContext, plasticity: f32) -> HardStroke {
    let mut s = loose_session(gpu, plasticity);
    let mut stroke = Stroke::begin(gpu, &mut s, HARD_PRESSURE);
    let during = stroke.run(gpu, &mut s, STROKE_SAMPLES);
    s.end_stroke();
    frame(gpu, &mut s);
    HardStroke {
        during,
        settled: settle(gpu, &mut s, stroke.time_ms),
        resting: settled_rest_offsets(gpu, plasticity),
    }
}

fn along(offset: [f32; 2], heading: Heading) -> f32 {
    offset[0] * heading.x as f32 + offset[1] * heading.y as f32
}

#[test]
fn bristle_tips_trail_behind_the_handle_while_it_moves() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let mut stroke = Stroke::begin(&gpu, &mut s, PRESSURE);
    let snapshot = stroke.run(&gpu, &mut s, STROKE_SAMPLES);
    let handle_x = handle(&snapshot)[0];
    let tips_x = tip_centroid(&snapshot)[0];
    let trail_px = TRAIL_SHARE * nominal_length_px(&s);
    assert!(
        tips_x < handle_x - trail_px,
        "the tips sit at x = {tips_x} with the handle at x = {handle_x}"
    );
}

#[test]
fn on_a_turn_the_tips_lag_the_new_direction() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let mut stroke = Stroke::begin(&gpu, &mut s, PRESSURE);
    stroke.run(&gpu, &mut s, STROKE_SAMPLES);
    let meaningful_px = MEANINGFUL_LAG_SHARE * nominal_length_px(&s);
    for dab in 0..TURN_SAMPLES {
        let snapshot = stroke.step(&gpu, &mut s, DOWNWARD);
        let tips = tip_centroid(&snapshot);
        let centre = handle(&snapshot);
        let lag = [tips[0] - centre[0], tips[1] - centre[1]];
        let behind_old = -along(lag, RIGHTWARD);
        let behind_new = -along(lag, DOWNWARD);
        assert!(
            behind_old > meaningful_px && behind_old > behind_new,
            "dab {dab} after the turn: the tips lag by {lag:?}"
        );
    }
}

#[test]
fn a_brush_stays_bent_after_a_hard_stroke() {
    let gpu = GpuContext::new().unwrap();
    let plasticity = HairSpec::default().bend_memory;
    let stroke = hard_stroke_then_lift(&gpu, plasticity);
    let during_px = mean_bend_px(&stroke.during, &stroke.resting);
    let settled_px = mean_bend_px(&stroke.settled, &stroke.resting);
    assert!(
        settled_px > SPRUNG_BACK_PX && settled_px < during_px,
        "the bend went from {during_px} px during the stroke to {settled_px} px at rest"
    );
}

#[test]
fn no_plasticity_lets_the_brush_spring_back_straight() {
    let gpu = GpuContext::new().unwrap();
    let stroke = hard_stroke_then_lift(&gpu, 0.0);
    let settled_px = mean_bend_px(&stroke.settled, &stroke.resting);
    assert!(
        settled_px < SPRUNG_BACK_PX,
        "the brush kept {settled_px} px of bend without plasticity"
    );
}

fn trail_px(gpu: &GpuContext, drag_anisotropy: f32) -> f32 {
    let mut s = session(gpu);
    s.brush.hair.drag_anisotropy = drag_anisotropy;
    let mut stroke = Stroke::begin(gpu, &mut s, PRESSURE);
    let snapshot = stroke.run(gpu, &mut s, STROKE_SAMPLES);
    handle(&snapshot)[0] - tip_centroid(&snapshot)[0]
}

#[test]
fn pulled_tips_slide_more_easily_the_more_anisotropic_the_drag() {
    let gpu = GpuContext::new().unwrap();
    let isotropic_px = trail_px(&gpu, 0.0);
    let anisotropic_px = trail_px(&gpu, 1.0);
    assert!(
        anisotropic_px < isotropic_px,
        "pulled tips trail {anisotropic_px} px with anisotropic drag, {isotropic_px} px without"
    );
}

fn rest_bend_bits(snapshot: &BristleSnapshot) -> Vec<[u32; 2]> {
    snapshot
        .rest_bends
        .iter()
        .map(|bend| bend.map(f32::to_bits))
        .collect()
}

fn bends_after_timeless_presses(gpu: &GpuContext, presses: u32) -> Vec<[u32; 2]> {
    let plasticity = HairSpec::default().bend_memory;
    let mut s = loose_session(gpu, plasticity);
    let mut stroke = Stroke::begin(gpu, &mut s, HARD_PRESSURE);
    stroke.run(gpu, &mut s, STROKE_SAMPLES);
    let mut last = None;
    for _ in 0..presses {
        stroke.time_ms -= SAMPLE_INTERVAL_MS;
        last = Some(stroke.step(gpu, &mut s, RIGHTWARD));
    }
    let last = last.expect("at least one timeless press");
    assert!(
        last.rest_bends
            .iter()
            .any(|bend| bend[0] != 0.0 || bend[1] != 0.0),
        "the hard stroke left no bend to keep"
    );
    rest_bend_bits(&last)
}

#[test]
fn holding_a_pressed_brush_still_does_not_bend_it_further() {
    let gpu = GpuContext::new().unwrap();
    let once = bends_after_timeless_presses(&gpu, 1);
    let many = bends_after_timeless_presses(&gpu, TIMELESS_PRESSES);
    assert!(
        once == many,
        "presses that took no time kept bending the brush"
    );
}

fn radial_set_lean(gpu: &GpuContext, pressure: f32) -> f32 {
    let mut s = session(gpu);
    s.brush.hair.clumping = 0.0;
    let mut stroke = Stroke::begin(gpu, &mut s, pressure);
    stroke.run(gpu, &mut s, STROKE_SAMPLES);
    s.end_stroke();
    frame(gpu, &mut s);
    let settled = settle(gpu, &mut s, stroke.time_ms);
    let centre = handle(&settled);
    let radial_bends: Vec<f32> = settled
        .rest_bends
        .iter()
        .zip(&settled.roots_px)
        .map(|(bend, root)| {
            let offset = [root[0] - centre[0], root[1] - centre[1]];
            let distance = offset[0].hypot(offset[1]).max(f32::MIN_POSITIVE);
            (bend[0] * offset[0] + bend[1] * offset[1]) / distance
        })
        .collect();
    radial_bends.iter().sum::<f32>() / radial_bends.len() as f32
}

#[test]
fn a_wet_brush_never_sets_its_point_into_its_bend_memory() {
    let gpu = GpuContext::new().unwrap();
    for pressure in [LIGHT_PRESSURE, HARD_PRESSURE] {
        let set_lean = radial_set_lean(&gpu, pressure);
        eprintln!(
            "at pressure {pressure} the settled bends lean {set_lean:.5} along the roots' radius"
        );
        assert!(
            set_lean.abs() <= MAX_RADIAL_SET_LEAN,
            "at pressure {pressure} the point set into the bends: {set_lean:.5}"
        );
    }
}
