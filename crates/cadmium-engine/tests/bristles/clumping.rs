use cadmium_engine::brush::FULL_RESERVOIR_VOLUME_UM;
use cadmium_engine::engine::{BristleSnapshot, FULL_DIP};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::pigments::{MAX_SLOTS, Mixture};
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::support::{drag, session, without_wgpu_errors};

const START_X: f64 = 200.0;
const CENTRE_Y: f64 = 128.0;
const PRESSURE: f32 = 0.8;
const PRESS_SAMPLES: u32 = 4;
const SAMPLE_INTERVAL_MS: u32 = 8;
const STEP_PX: f64 = 0.5;
const WIDER_SHARE: f32 = 1.02;
const FULL_CLUMPING: f32 = 1.0;
const NARROW_GAP_RANK_DIVISOR: usize = 10;
const MIN_NARROW_GAP_PX: f32 = 1.0;

fn full_load() -> Mixture {
    let mut fractions = [0.0; MAX_SLOTS];
    fractions[0] = 1.0;
    Mixture {
        volume_um: FULL_RESERVOIR_VOLUME_UM,
        fractions,
    }
}

fn press(gpu: &GpuContext, s: &mut PaintSession) -> BristleSnapshot {
    let samples: Vec<StrokeSample> = (0..PRESS_SAMPLES)
        .map(|index| StrokeSample {
            x: START_X + STEP_PX * f64::from(index),
            y: CENTRE_Y,
            pressure: PRESSURE,
            tilt_x_deg: 0.0,
            tilt_y_deg: 0.0,
            time_ms: index * SAMPLE_INTERVAL_MS,
        })
        .collect();
    drag(gpu, s, &samples);
    s.engine.bristle_snapshot(gpu).unwrap()
}

fn mean_spread_px(snapshot: &BristleSnapshot) -> f32 {
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

#[test]
fn a_dry_brush_fans_wider_than_a_wet_one() {
    let gpu = GpuContext::new().unwrap();
    let mut wet = session(&gpu);
    without_wgpu_errors(&gpu, || wet.dip(&gpu, &full_load(), FULL_DIP).unwrap());
    let mut dry = session(&gpu);
    without_wgpu_errors(&gpu, || dry.clean_brush(&gpu));
    let wet_spread = mean_spread_px(&press(&gpu, &mut wet));
    let dry_spread = mean_spread_px(&press(&gpu, &mut dry));
    assert!(
        dry_spread > WIDER_SHARE * wet_spread,
        "the dry brush fans to {dry_spread} px, the wet one to {wet_spread} px"
    );
}

fn nearest_tip_gaps_px(snapshot: &BristleSnapshot) -> Vec<f32> {
    let tips = &snapshot.tips_px;
    let gap = |a: &[f32; 3], b: &[f32; 3]| {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    };
    let mut gaps: Vec<f32> = tips
        .iter()
        .enumerate()
        .map(|(index, tip)| {
            tips.iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .map(|(_, other)| gap(tip, other))
                .fold(f32::INFINITY, f32::min)
        })
        .collect();
    gaps.sort_by(f32::total_cmp);
    gaps
}

#[test]
fn fully_clumped_wet_bristles_still_keep_apart_from_one_another() {
    let gpu = GpuContext::new().unwrap();
    let mut wet = session(&gpu);
    wet.brush.hair.clumping = FULL_CLUMPING;
    without_wgpu_errors(&gpu, || wet.dip(&gpu, &full_load(), FULL_DIP).unwrap());
    let gaps = nearest_tip_gaps_px(&press(&gpu, &mut wet));
    let narrow_gap_px = gaps[gaps.len() / NARROW_GAP_RANK_DIVISOR];
    assert!(
        narrow_gap_px > MIN_NARROW_GAP_PX,
        "a tenth of the tips sit within {narrow_gap_px} px of another tip"
    );
}
