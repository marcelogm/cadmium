use cadmium_engine::engine::{BRISTLE_SETTLE_MS, BristleSnapshot};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::support::{frame_at, session, total_volume};

const FRAME_CLOCK_START_MS: f64 = 987_654.0;
const TICK_MS: f64 = 16.0;
const START_X: f64 = 160.0;
const CENTRE_Y: f64 = 128.0;
const STEP_PX: f64 = 2.0;
const SAMPLE_INTERVAL_MS: u32 = 8;
const STROKE_SAMPLES: u32 = 30;
const HARD_PRESSURE: f32 = 1.0;
const EARLY_TICKS: u32 = 4;
const HOLD_TICKS: u32 = 60;
const SPRING_BACK_SHARE: f32 = 0.5;
const SETTLE_TICK_LIMIT: u32 = 200;
const JUST_BEFORE_MS: f64 = 1.0;
const AFTER_SETTLING_MS: f64 = 100.0;
const OFF_CANVAS_PX: f64 = -5000.0;
const OFF_CANVAS_SAMPLES: u32 = 4;

struct HeldStroke {
    session: PaintSession,
    frame_clock_ms: f64,
}

impl HeldStroke {
    fn press(gpu: &GpuContext) -> HeldStroke {
        HeldStroke::press_in(gpu, session(gpu))
    }

    fn press_in(gpu: &GpuContext, mut s: PaintSession) -> HeldStroke {
        let mut frame_clock_ms = FRAME_CLOCK_START_MS;
        s.begin_stroke(gpu, pressed_sample(0));
        frame_at(gpu, &mut s, frame_clock_ms);
        for index in 1..STROKE_SAMPLES {
            s.add_samples(&[pressed_sample(index)]);
            frame_clock_ms += f64::from(SAMPLE_INTERVAL_MS);
            frame_at(gpu, &mut s, frame_clock_ms);
        }
        HeldStroke {
            session: s,
            frame_clock_ms,
        }
    }

    fn lift(gpu: &GpuContext) -> HeldStroke {
        let mut held = HeldStroke::press(gpu);
        held.session.end_stroke();
        held.tick(gpu);
        held
    }

    fn tick(&mut self, gpu: &GpuContext) {
        self.frame_clock_ms += TICK_MS;
        frame_at(gpu, &mut self.session, self.frame_clock_ms);
    }

    fn ticks(&mut self, gpu: &GpuContext, count: u32) -> BristleSnapshot {
        for _ in 0..count {
            self.tick(gpu);
        }
        self.snapshot(gpu)
    }

    fn settle_fully(&mut self, gpu: &GpuContext) {
        let mut ticks = 0;
        while self.settling_in(0.0) {
            self.tick(gpu);
            ticks += 1;
            assert!(ticks < SETTLE_TICK_LIMIT, "the bristles never settled");
        }
    }

    fn stroke_off_the_canvas(&mut self, gpu: &GpuContext) {
        self.session.begin_stroke(gpu, off_canvas_sample(0));
        for index in 1..OFF_CANVAS_SAMPLES {
            self.session.add_samples(&[off_canvas_sample(index)]);
        }
        self.session.end_stroke();
    }

    fn snapshot(&self, gpu: &GpuContext) -> BristleSnapshot {
        self.session.engine.bristle_snapshot(gpu).unwrap()
    }

    fn settling_in(&self, later_ms: f64) -> bool {
        self.session
            .bristles_settling(self.frame_clock_ms + later_ms)
    }
}

fn unpointed(gpu: &GpuContext) -> PaintSession {
    let mut s = session(gpu);
    s.brush.hair.wet_point = 0.0;
    s
}

fn pressed_sample(index: u32) -> StrokeSample {
    StrokeSample {
        x: START_X + STEP_PX * f64::from(index),
        y: CENTRE_Y,
        pressure: HARD_PRESSURE,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms: index * SAMPLE_INTERVAL_MS,
    }
}

fn off_canvas_sample(index: u32) -> StrokeSample {
    StrokeSample {
        x: OFF_CANVAS_PX + STEP_PX * f64::from(index),
        y: OFF_CANVAS_PX,
        ..pressed_sample(index)
    }
}

fn mean_tip_offset_px(snapshot: &BristleSnapshot) -> f32 {
    let total: f32 = snapshot
        .tips_px
        .iter()
        .zip(&snapshot.roots_px)
        .map(|(tip, root)| (tip[0] - root[0]).hypot(tip[1] - root[1]))
        .sum();
    total / snapshot.tips_px.len() as f32
}

fn wet_canvas_volume(gpu: &GpuContext, s: &PaintSession) -> f64 {
    s.canvas.wet_totals(gpu).unwrap().volume
}

#[test]
fn a_lifted_brush_springs_back_toward_rest() {
    let gpu = GpuContext::new().unwrap();
    let mut held = HeldStroke::press_in(&gpu, unpointed(&gpu));
    let pressed = mean_tip_offset_px(&held.snapshot(&gpu));
    held.session.end_stroke();
    held.tick(&gpu);
    let early = mean_tip_offset_px(&held.ticks(&gpu, EARLY_TICKS));
    let late = mean_tip_offset_px(&held.ticks(&gpu, HOLD_TICKS));
    assert!(
        early < pressed,
        "{early} px after lifting, {pressed} px pressed"
    );
    assert!(
        late < SPRING_BACK_SHARE * pressed,
        "{late} px once settled, {pressed} px pressed"
    );
}

#[test]
fn holding_a_pressed_brush_still_lays_no_paint() {
    let gpu = GpuContext::new().unwrap();
    let mut held = HeldStroke::press(&gpu);
    let before = held.snapshot(&gpu);
    let canvas_before = wet_canvas_volume(&gpu, &held.session);
    let total_before = total_volume(&gpu, &held.session);
    let after = held.ticks(&gpu, HOLD_TICKS);
    assert_ne!(before, after, "the held bristles never ticked");
    assert_eq!(wet_canvas_volume(&gpu, &held.session), canvas_before);
    assert_eq!(total_volume(&gpu, &held.session), total_before);
}

#[test]
fn a_brush_held_still_stops_ticking_once_its_last_motion_has_settled() {
    let gpu = GpuContext::new().unwrap();
    let held = HeldStroke::press(&gpu);
    let settle_ms = f64::from(BRISTLE_SETTLE_MS);
    assert!(held.settling_in(settle_ms - JUST_BEFORE_MS));
    assert!(!held.settling_in(settle_ms));
}

#[test]
fn ticks_stop_once_the_bristles_have_settled() {
    let gpu = GpuContext::new().unwrap();
    let mut held = HeldStroke::lift(&gpu);
    let lifted_ms = held.frame_clock_ms;
    held.settle_fully(&gpu);
    let settled_ms = held.frame_clock_ms - lifted_ms;
    assert!(settled_ms >= f64::from(BRISTLE_SETTLE_MS));
    assert!(settled_ms < f64::from(BRISTLE_SETTLE_MS) + 2.0 * TICK_MS);
    let settled = held.snapshot(&gpu);
    held.frame_clock_ms += AFTER_SETTLING_MS;
    frame_at(&gpu, &mut held.session, held.frame_clock_ms);
    assert_eq!(held.snapshot(&gpu), settled);
}

#[test]
fn a_stroke_that_never_reaches_the_canvas_asks_for_no_settling_frames() {
    let gpu = GpuContext::new().unwrap();
    let mut held = HeldStroke::lift(&gpu);
    held.settle_fully(&gpu);
    held.stroke_off_the_canvas(&gpu);
    let stroked_ms = held.frame_clock_ms;
    for _ in 0..SETTLE_TICK_LIMIT {
        held.tick(&gpu);
        let elapsed_ms = held.frame_clock_ms - stroked_ms;
        assert!(
            elapsed_ms < f64::from(BRISTLE_SETTLE_MS) || !held.settling_in(0.0),
            "still settling {elapsed_ms} ms after a stroke that missed the canvas"
        );
    }
}

const NEXT_STROKE_Y: f64 = 200.0;

fn dry(gpu: &GpuContext) -> PaintSession {
    let mut s = session(gpu);
    s.clean_brush(gpu);
    s
}

fn first_dab_of_next_stroke(gpu: &GpuContext, s: &mut PaintSession, frame_clock_ms: f64) {
    let start = StrokeSample {
        y: NEXT_STROKE_Y,
        ..pressed_sample(0)
    };
    s.begin_stroke(gpu, start);
    frame_at(gpu, s, frame_clock_ms);
}

#[test]
fn each_stroke_starts_from_a_brush_at_rest() {
    let gpu = GpuContext::new().unwrap();
    let mut used = HeldStroke::press_in(&gpu, dry(&gpu));
    used.session.end_stroke();
    used.settle_fully(&gpu);
    assert!(
        used.snapshot(&gpu)
            .rest_bends
            .iter()
            .any(|bend| bend[0] != 0.0 || bend[1] != 0.0),
        "the hard stroke left no bend"
    );
    let later_ms = used.frame_clock_ms + TICK_MS;
    first_dab_of_next_stroke(&gpu, &mut used.session, later_ms);
    let mut fresh = dry(&gpu);
    first_dab_of_next_stroke(&gpu, &mut fresh, later_ms);
    let used = used.snapshot(&gpu);
    let fresh = fresh.engine.bristle_snapshot(&gpu).unwrap();
    assert!(
        used.chains_px == fresh.chains_px && used.rest_bends == fresh.rest_bends,
        "the next stroke started from the last stroke's bend: tips {:?} instead of {:?}",
        &used.tips_px[..2],
        &fresh.tips_px[..2]
    );
}
