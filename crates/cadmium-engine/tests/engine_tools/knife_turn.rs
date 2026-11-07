//! The knife's blade turns only as far as the hand travels.

use cadmium_engine::{canvas::StateTexture, gpu::GpuContext, stroke::StrokeSample};

use crate::CANVAS_WIDTH;
use crate::knife::{LAID_TILT_DEG, PLATE_PRESSURE, dip_knife};
use crate::support::{drag, line, session};

const JIGGLE_AT: [f64; 2] = [256.0, 128.0];
/// Tiny moves every which way around one point, as a hand holding the knife still makes.
const JIGGLE_STEPS: [[f64; 2]; 8] = [
    [0.0, 1.0],
    [0.7, 0.7],
    [0.0, -1.0],
    [-0.7, 0.7],
    [0.0, 1.0],
    [0.7, -0.7],
    [0.0, -1.0],
    [-0.7, -0.7],
];
const JIGGLE_MARGIN_PX: usize = 6;

/// A laid knife drawn rightwards to `JIGGLE_AT`, then jiggled there.
fn jiggled_knife() -> Vec<StrokeSample> {
    let mut samples = line(
        JIGGLE_AT[1],
        200.0,
        JIGGLE_AT[0],
        PLATE_PRESSURE,
        LAID_TILT_DEG,
    );
    let last = *samples.last().unwrap();
    let mut at = JIGGLE_AT;
    for (i, [dx, dy]) in JIGGLE_STEPS.into_iter().enumerate() {
        at = [at[0] + dx, at[1] + dy];
        samples.push(StrokeSample {
            x: at[0],
            y: at[1],
            time_ms: last.time_ms + 4 * (i as u32 + 1),
            ..last
        });
    }
    samples
}

#[test]
fn a_knife_jiggled_in_place_keeps_its_blade_across_the_stroke() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    dip_knife(&gpu, &mut s);
    drag(&gpu, &mut s, &jiggled_knife());
    let wet = s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
    let beyond = JIGGLE_AT[0] as usize + JIGGLE_MARGIN_PX;
    let ahead: f32 = (0..wet.len() / CANVAS_WIDTH)
        .flat_map(|y| (beyond..CANVAS_WIDTH).map(move |x| y * CANVAS_WIDTH + x))
        .map(|i| wet[i])
        .sum();
    assert_eq!(ahead, 0.0, "paint swung out ahead of the blade");
}

const ARC_CENTRE: [f64; 2] = [256.0, 180.0];
const ARC_RADIUS_PX: f64 = 60.0;
/// Few samples round the arc, as a quick hand gives: each corner turns the heading at once.
const ARC_SAMPLES: u32 = 6;
/// Where the outer half of the blade passes, as a share of its half length past the arc.
const OUTER_REACH_SHARE: f64 = 0.8;
const ARC_CHECKS: u32 = 90;

fn arc_point(angle: f64, radius: f64) -> [f64; 2] {
    [
        ARC_CENTRE[0] + radius * angle.cos(),
        ARC_CENTRE[1] - radius * angle.sin(),
    ]
}

/// A laid knife rounding the top of a circle, right to left.
fn arc_knife() -> Vec<StrokeSample> {
    (0..=ARC_SAMPLES)
        .map(|i| {
            let angle = std::f64::consts::PI * f64::from(i) / f64::from(ARC_SAMPLES);
            let [x, y] = arc_point(angle, ARC_RADIUS_PX);
            StrokeSample {
                x,
                y,
                pressure: PLATE_PRESSURE,
                tilt_x_deg: LAID_TILT_DEG,
                tilt_y_deg: 0.0,
                time_ms: i * 16,
            }
        })
        .collect()
}

#[test]
fn a_knife_rounding_a_curve_leaves_its_outer_edge_unbroken() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    dip_knife(&gpu, &mut s);
    drag(&gpu, &mut s, &arc_knife());
    let wet = s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
    let half_length = f64::from(s.brush.geometry.max_diameter_px) / 2.0;
    let outer = ARC_RADIUS_PX + OUTER_REACH_SHARE * half_length;
    let gaps: Vec<u32> = (0..=ARC_CHECKS)
        .filter(|&i| {
            let angle = std::f64::consts::PI * (0.2 + 0.6 * f64::from(i) / f64::from(ARC_CHECKS));
            let [x, y] = arc_point(angle, outer);
            wet[y.round() as usize * CANVAS_WIDTH + x.round() as usize] <= 0.0
        })
        .collect();
    assert!(
        gaps.is_empty(),
        "gaps along the outer edge at checks {gaps:?}"
    );
}
