use cadmium_engine::brush::BrushShape;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::support::{frame, session};

const START_X: f64 = 150.0;
const CENTRE_Y: f64 = 128.0;
const STEP_PX: f64 = 2.0;
const SAMPLES_PER_POSE: u32 = 12;
const SAMPLE_INTERVAL_MS: u32 = 8;
const ROOT_DRIFT_TOLERANCE_PX: f32 = 1.0;

struct Lean {
    tilt_deg: f32,
    azimuth_deg: f32,
}

fn leaning_sample(index: u32, lean: &Lean) -> StrokeSample {
    let slope = lean.tilt_deg.to_radians().tan();
    let azimuth = lean.azimuth_deg.to_radians();
    StrokeSample {
        x: START_X + STEP_PX * f64::from(index),
        y: CENTRE_Y,
        pressure: 0.5,
        tilt_x_deg: (slope * azimuth.cos()).atan().to_degrees(),
        tilt_y_deg: (slope * azimuth.sin()).atan().to_degrees(),
        time_ms: index * SAMPLE_INTERVAL_MS,
    }
}

fn roots_around_centre(gpu: &GpuContext, s: &PaintSession) -> Vec<[f32; 2]> {
    let roots = s.engine.bristle_snapshot(gpu).unwrap().roots_px;
    let count = roots.len() as f32;
    let centre = roots.iter().fold([0.0, 0.0], |sum, root| {
        [sum[0] + root[0] / count, sum[1] + root[1] / count]
    });
    roots
        .iter()
        .map(|root| [root[0] - centre[0], root[1] - centre[1]])
        .collect()
}

fn roots_before_and_after_turning(
    gpu: &GpuContext,
    shape: BrushShape,
    from: Lean,
    to: Lean,
) -> (Vec<[f32; 2]>, Vec<[f32; 2]>) {
    let mut s = session(gpu);
    s.brush.shape = shape;
    s.begin_stroke(gpu, leaning_sample(0, &from));
    let first_pose = (1..SAMPLES_PER_POSE).map(|index| leaning_sample(index, &from));
    s.add_samples(&first_pose.collect::<Vec<_>>());
    frame(gpu, &mut s);
    let before = roots_around_centre(gpu, &s);
    let second_pose =
        (SAMPLES_PER_POSE..2 * SAMPLES_PER_POSE).map(|index| leaning_sample(index, &to));
    s.add_samples(&second_pose.collect::<Vec<_>>());
    frame(gpu, &mut s);
    let after = roots_around_centre(gpu, &s);
    s.end_stroke();
    (before, after)
}

fn largest_drift_px(before: &[[f32; 2]], after: &[[f32; 2]]) -> f32 {
    assert_eq!(before.len(), after.len());
    assert!(!before.is_empty());
    before
        .iter()
        .zip(after)
        .map(|(was, now)| (now[0] - was[0]).hypot(now[1] - was[1]))
        .fold(0.0, f32::max)
}

#[test]
fn a_leaning_flat_brush_turning_through_a_right_angle_keeps_its_roots_in_place() {
    let gpu = GpuContext::new().unwrap();
    let (before, after) = roots_before_and_after_turning(
        &gpu,
        BrushShape::Flat,
        Lean {
            tilt_deg: 30.0,
            azimuth_deg: 89.0,
        },
        Lean {
            tilt_deg: 30.0,
            azimuth_deg: 91.0,
        },
    );
    let drift = largest_drift_px(&before, &after);
    assert!(drift <= ROOT_DRIFT_TOLERANCE_PX, "a root swept {drift} px");
}

#[test]
fn a_round_brush_keeps_its_roots_when_the_pen_turns() {
    let gpu = GpuContext::new().unwrap();
    let (before, after) = roots_before_and_after_turning(
        &gpu,
        BrushShape::Round,
        Lean {
            tilt_deg: 10.0,
            azimuth_deg: 0.0,
        },
        Lean {
            tilt_deg: 10.0,
            azimuth_deg: 90.0,
        },
    );
    let drift = largest_drift_px(&before, &after);
    assert!(drift <= ROOT_DRIFT_TOLERANCE_PX, "a root swept {drift} px");
}
