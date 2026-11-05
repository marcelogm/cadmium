use cadmium_engine::brush::BrushShape;
use cadmium_engine::engine::{BristleSnapshot, PaintEngine};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::{StrokeSample, Tool};

use crate::support::{drag, frame, session};

const FERRULE_RADIUS_PX: f32 = 20.0;
const START_X: f64 = 200.0;
const CENTRE_Y: f64 = 128.0;
const HOLD_SAMPLES: u32 = 6;
const MOVE_SAMPLES: u32 = 30;
const MOVE_STEP_PX: f64 = 2.0;
const SAMPLE_INTERVAL_MS: u32 = 8;
const RESTING_TIP_REACH_PX: f32 = 1.0;
const PRESSED_SPLAY_SHARE: f32 = 1.3;
const MAX_WET_POINT_SHARE: f32 = 0.5;
const MIN_DRY_SPREAD_SHARE: f32 = 0.9;
const BELOW_CANVAS_TOLERANCE_PX: f32 = 1e-3;
const ZIG_ZAG_SAMPLES: u32 = 48;
const ZIG_ZAG_AMPLITUDE_PX: f64 = 30.0;
const ZIG_ZAG_STEP_PX: f64 = 6.0;
const ZIG_ZAG_PERIOD: u32 = 6;

type SnapshotBits = (Vec<[u32; 3]>, Vec<[u32; 2]>);

fn sample(x: f64, y: f64, pressure: f32, time_ms: u32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms,
    }
}

fn press_samples(pressure: f32, time_ms: u32) -> Vec<StrokeSample> {
    (0..HOLD_SAMPLES + MOVE_SAMPLES)
        .map(|index| {
            let moved = index.saturating_sub(HOLD_SAMPLES - 1);
            let x = START_X + MOVE_STEP_PX * f64::from(moved);
            sample(x, CENTRE_Y, pressure, time_ms + index * SAMPLE_INTERVAL_MS)
        })
        .collect()
}

fn unclumped(gpu: &GpuContext) -> PaintSession {
    let mut s = session(gpu);
    s.engine.params_mut().set("bristle_clumping", 0.0);
    s
}

fn loose(gpu: &GpuContext) -> PaintSession {
    let mut s = unclumped(gpu);
    s.engine.params_mut().set("bristle_pointing", 0.0);
    s
}

fn press(gpu: &GpuContext, s: &mut PaintSession, pressure: f32) -> BristleSnapshot {
    drag(gpu, s, &press_samples(pressure, 0));
    s.engine.bristle_snapshot(gpu).unwrap()
}

fn as_flat_with_round_length(s: &mut PaintSession) {
    let round_length = s.engine.params().get("bristle_length_round");
    s.engine
        .params_mut()
        .set("bristle_length_flat", round_length);
    s.brush.shape = BrushShape::Flat;
}

fn touch_once(gpu: &GpuContext, s: &mut PaintSession, pressure: f32) -> BristleSnapshot {
    drag(gpu, s, &[sample(300.0, 100.0, pressure, 0)]);
    s.engine.bristle_snapshot(gpu).unwrap()
}

fn ferrule_centre(snapshot: &BristleSnapshot) -> [f32; 2] {
    let count = snapshot.roots_px.len() as f32;
    let sum = snapshot
        .roots_px
        .iter()
        .fold([0.0, 0.0], |sum, root| [sum[0] + root[0], sum[1] + root[1]]);
    [sum[0] / count, sum[1] / count]
}

fn tip_distances_from_centre(snapshot: &BristleSnapshot) -> Vec<f32> {
    let centre = ferrule_centre(snapshot);
    snapshot
        .tips_px
        .iter()
        .map(|tip| (tip[0] - centre[0]).hypot(tip[1] - centre[1]))
        .collect()
}

fn mean(values: &[f32]) -> f32 {
    values.iter().sum::<f32>() / values.len() as f32
}

fn tip_spread_share(snapshot: &BristleSnapshot) -> f32 {
    let centre = ferrule_centre(snapshot);
    let roots: Vec<f32> = snapshot
        .roots_px
        .iter()
        .map(|root| (root[0] - centre[0]).hypot(root[1] - centre[1]))
        .collect();
    mean(&tip_distances_from_centre(snapshot)) / mean(&roots)
}

fn bits(snapshot: &BristleSnapshot) -> SnapshotBits {
    let tips = snapshot.tips_px.iter().map(|tip| tip.map(f32::to_bits));
    let roots = snapshot.roots_px.iter().map(|root| root.map(f32::to_bits));
    (tips.collect(), roots.collect())
}

fn zig_zag(index: u32) -> StrokeSample {
    let side = if (index / ZIG_ZAG_PERIOD).is_multiple_of(2) {
        1.0
    } else {
        -1.0
    };
    let phase = f64::from(index % ZIG_ZAG_PERIOD) / f64::from(ZIG_ZAG_PERIOD);
    let x = 100.0 + ZIG_ZAG_STEP_PX * f64::from(index);
    let y = CENTRE_Y + side * ZIG_ZAG_AMPLITUDE_PX * (2.0 * phase - 1.0);
    sample(x, y, 0.9, index * SAMPLE_INTERVAL_MS)
}

fn zig_zag_snapshots(gpu: &GpuContext, s: &mut PaintSession) -> Vec<BristleSnapshot> {
    s.begin_stroke(gpu, zig_zag(0));
    let mut snapshots = Vec::new();
    for index in 1..ZIG_ZAG_SAMPLES {
        s.add_samples(&[zig_zag(index)]);
        frame(gpu, s);
        snapshots.push(s.engine.bristle_snapshot(gpu).unwrap());
    }
    s.end_stroke();
    frame(gpu, s);
    snapshots
}

fn assert_above_canvas(snapshot: &BristleSnapshot) {
    assert!(
        snapshot.lowest_clearance_px >= -BELOW_CANVAS_TOLERANCE_PX,
        "a vertex sank {} px into the canvas",
        snapshot.lowest_clearance_px
    );
}

#[test]
fn a_resting_brush_hangs_its_bristles_straight_down_from_the_ferrule() {
    let gpu = GpuContext::new().unwrap();
    let mut s = loose(&gpu);
    let snapshot = press(&gpu, &mut s, 0.0);
    assert!(!snapshot.tips_px.is_empty());
    assert_eq!(snapshot.tips_px.len(), snapshot.roots_px.len());
    for (tip, root) in snapshot.tips_px.iter().zip(&snapshot.roots_px) {
        let reach = (tip[0] - root[0]).hypot(tip[1] - root[1]);
        assert!(
            reach <= RESTING_TIP_REACH_PX,
            "tip {tip:?} strays {reach} px from its root {root:?}"
        );
    }
    assert_above_canvas(&snapshot);
}

#[test]
fn a_wet_round_brush_hangs_to_a_point_and_a_dry_one_hangs_straight() {
    let gpu = GpuContext::new().unwrap();
    let mut wet = unclumped(&gpu);
    let wet_share = tip_spread_share(&press(&gpu, &mut wet, 0.0));
    let mut dry = unclumped(&gpu);
    dry.clean_brush(&gpu);
    let dry_share = tip_spread_share(&press(&gpu, &mut dry, 0.0));
    eprintln!("tips spread {wet_share:.3} of the roots wet, {dry_share:.3} dry");
    assert!(
        wet_share <= MAX_WET_POINT_SHARE,
        "a wet brush keeps its tips at {wet_share:.3}"
    );
    assert!(
        dry_share >= MIN_DRY_SPREAD_SHARE,
        "a dry brush gathers its tips to {dry_share:.3}"
    );
}

#[test]
fn pressing_an_upright_round_brush_splays_its_tips_beyond_the_ferrule() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let snapshot = press(&gpu, &mut s, 1.0);
    let widest = tip_distances_from_centre(&snapshot)
        .into_iter()
        .fold(0.0f32, f32::max);
    assert!(
        widest > PRESSED_SPLAY_SHARE * FERRULE_RADIUS_PX,
        "the widest tip reaches only {widest} px"
    );
    assert!(snapshot.touching.iter().any(|&touching| touching));
    assert_above_canvas(&snapshot);
}

#[test]
fn pressing_harder_splays_wider() {
    let gpu = GpuContext::new().unwrap();
    let mut light = session(&gpu);
    let mut firm = session(&gpu);
    let light_spread = mean(&tip_distances_from_centre(&press(&gpu, &mut light, 0.5)));
    let firm_spread = mean(&tip_distances_from_centre(&press(&gpu, &mut firm, 1.0)));
    assert!(
        firm_spread > light_spread,
        "firm {firm_spread} px against light {light_spread} px"
    );
}

#[test]
fn no_vertex_ever_goes_below_the_canvas() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    for snapshot in zig_zag_snapshots(&gpu, &mut s) {
        assert_above_canvas(&snapshot);
    }
}

#[test]
fn a_stroke_repeated_gives_bit_identical_bristles() {
    let gpu = GpuContext::new().unwrap();
    let mut first = session(&gpu);
    let mut second = session(&gpu);
    let first_run: Vec<SnapshotBits> = zig_zag_snapshots(&gpu, &mut first)
        .iter()
        .map(bits)
        .collect();
    let second_run: Vec<SnapshotBits> = zig_zag_snapshots(&gpu, &mut second)
        .iter()
        .map(bits)
        .collect();
    assert!(!first_run.is_empty());
    assert!(
        first_run == second_run,
        "the same stroke moved the bristles differently"
    );
}

#[test]
fn timestamps_that_repeat_or_run_backwards_keep_bristles_finite() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let samples: Vec<StrokeSample> = [0, 0, 30, 10, 5000]
        .into_iter()
        .enumerate()
        .map(|(i, time_ms)| sample(200.0 + 10.0 * i as f64, CENTRE_Y, 1.0, time_ms))
        .collect();
    drag(&gpu, &mut s, &samples);
    let snapshot = s.engine.bristle_snapshot(&gpu).unwrap();
    assert!(!snapshot.tips_px.is_empty());
    assert!(
        snapshot
            .tips_px
            .iter()
            .flatten()
            .all(|value| value.is_finite())
    );
    assert!(
        snapshot
            .roots_px
            .iter()
            .flatten()
            .all(|value| value.is_finite())
    );
    assert_above_canvas(&snapshot);
}

#[test]
fn changing_the_shape_rebuilds_the_bristles_at_rest() {
    let gpu = GpuContext::new().unwrap();
    let mut used = unclumped(&gpu);
    press(&gpu, &mut used, 1.0);
    as_flat_with_round_length(&mut used);
    let rebuilt = touch_once(&gpu, &mut used, 0.6);
    let mut fresh = unclumped(&gpu);
    as_flat_with_round_length(&mut fresh);
    let expected = touch_once(&gpu, &mut fresh, 0.6);
    assert!(
        bits(&rebuilt) == bits(&expected),
        "the flat brush kept the round brush's bristles"
    );
}

#[test]
fn cleaning_the_brush_rebuilds_the_bristles_at_rest() {
    let gpu = GpuContext::new().unwrap();
    let mut used = unclumped(&gpu);
    press(&gpu, &mut used, 1.0);
    used.clean_brush(&gpu);
    let cleaned = touch_once(&gpu, &mut used, 0.6);
    let mut fresh = unclumped(&gpu);
    let expected = touch_once(&gpu, &mut fresh, 0.6);
    assert!(
        bits(&cleaned) == bits(&expected),
        "cleaning kept the bent bristles"
    );
}

#[test]
fn taking_the_brush_up_again_after_another_tool_rebuilds_the_bristles_at_rest() {
    let gpu = GpuContext::new().unwrap();
    let mut used = unclumped(&gpu);
    press(&gpu, &mut used, 1.0);
    used.brush.tool = Tool::Knife;
    drag(&gpu, &mut used, &press_samples(1.0, 0));
    used.brush.tool = Tool::Brush;
    let retaken = touch_once(&gpu, &mut used, 0.6);
    let mut fresh = unclumped(&gpu);
    let expected = touch_once(&gpu, &mut fresh, 0.6);
    assert!(
        bits(&retaken) == bits(&expected),
        "the brush came back from the knife still bent"
    );
}
