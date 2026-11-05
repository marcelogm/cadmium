use cadmium_engine::canvas::StateTexture;
use cadmium_engine::engine::{BristleSnapshot, PaintEngine, RELIEF_UM_PER_PX};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::support::{frame, session};

const WIDTH: usize = 512;
const HEIGHT: usize = 256;
const CENTRE_Y: f64 = 128.0;
const SAMPLE_INTERVAL_MS: u32 = 8;
const PRESS_SAMPLES: u32 = 24;
const PRESS_STEP_PX: f64 = 1.0;
const CONTACT_TOLERANCE_PX: f32 = 0.05;

const STEP_EDGE_X: usize = 220;
const STEP_HEIGHT_UM: f32 = 400.0;
const EDGE_MARGIN_PX: f32 = 2.0;
const STEP_PRESSURE: f32 = 0.8;
const FULL_CLUMPING: f32 = 1.0;

const LAYER_HEIGHT_UM: f32 = 1200.0;
const LAYER_PRESS_X: f64 = 240.0;
const WET_SUPPORT: f32 = 0.5;
const MIN_WET_LIFT_SHARE: f32 = 0.7;

const RIDGE_PERIOD_PX: usize = 24;
const RIDGE_WIDTH_PX: usize = 12;
const RIDGE_HEIGHT_UM: f32 = 1200.0;
const LIGHT_PRESSURE: f32 = 0.15;
const PRESS_DEPTH_SHARE: f32 = 0.35;
const SHARED_PRESS_MARGIN: f32 = 2.0;

fn upright(x: f64, pressure: f32, index: u32) -> StrokeSample {
    StrokeSample {
        x,
        y: CENTRE_Y,
        pressure,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms: index * SAMPLE_INTERVAL_MS,
    }
}

fn layered(
    gpu: &GpuContext,
    painting: &PaintSession,
    which: StateTexture,
    height_um: impl Fn(usize) -> f32,
) {
    let heights: Vec<f32> = (0..WIDTH * HEIGHT)
        .map(|pixel| height_um(pixel % WIDTH))
        .collect();
    painting.canvas.write_f32(gpu, which, &heights);
}

fn relief_px(painting: &PaintSession, height_um: f32) -> f32 {
    (height_um - painting.canvas.tooth_height_um()).max(0.0) / RELIEF_UM_PER_PX
}

fn dried(gpu: &GpuContext, painting: &PaintSession, height_um: impl Fn(usize) -> f32) {
    layered(gpu, painting, StateTexture::DryHeight, height_um);
}

fn pressed_snapshot(
    gpu: &GpuContext,
    painting: &mut PaintSession,
    x: f64,
    pressure: f32,
) -> BristleSnapshot {
    painting.begin_stroke(gpu, upright(x, pressure, 0));
    for index in 1..PRESS_SAMPLES {
        let moved = x + PRESS_STEP_PX * f64::from(index);
        painting.add_samples(&[upright(moved, pressure, index)]);
        frame(gpu, painting);
    }
    let snapshot = painting.engine.bristle_snapshot(gpu).unwrap();
    painting.end_stroke();
    frame(gpu, painting);
    snapshot
}

#[test]
fn bristles_pressed_onto_a_dried_ridge_rest_on_its_top() {
    let gpu = GpuContext::new().unwrap();
    let mut painting = session(&gpu);
    dried(&gpu, &painting, |x| {
        if x >= STEP_EDGE_X {
            STEP_HEIGHT_UM
        } else {
            0.0
        }
    });
    let start_x = STEP_EDGE_X as f64 - PRESS_STEP_PX * f64::from(PRESS_SAMPLES) / 2.0;
    let snapshot = pressed_snapshot(&gpu, &mut painting, start_x, STEP_PRESSURE);
    let step_px = relief_px(&painting, STEP_HEIGHT_UM);
    let on_step: Vec<f32> = snapshot
        .tips_px
        .iter()
        .filter(|tip| tip[0] >= STEP_EDGE_X as f32 + EDGE_MARGIN_PX)
        .map(|tip| tip[2])
        .collect();
    let lowest_on_step = on_step.iter().copied().fold(f32::INFINITY, f32::min);
    eprintln!(
        "{} tips over the {step_px} px step, the lowest at z = {lowest_on_step}; lowest clearance {}",
        on_step.len(),
        snapshot.lowest_clearance_px
    );
    assert!(!on_step.is_empty(), "no tip reached over the step");
    assert!(
        lowest_on_step >= step_px - CONTACT_TOLERANCE_PX,
        "a tip sank to z = {lowest_on_step} into the {step_px} px step"
    );
    assert!(snapshot.lowest_clearance_px >= -CONTACT_TOLERANCE_PX);
    assert!(snapshot.touching.iter().any(|&touching| touching));
}

fn ferrule_height_px(gpu: &GpuContext, layer_um: f32) -> f32 {
    let mut painting = session(gpu);
    dried(gpu, &painting, |_| layer_um);
    ferrule_height_on(gpu, &mut painting)
}

fn ferrule_height_on(gpu: &GpuContext, painting: &mut PaintSession) -> f32 {
    ferrule_z_px(&pressed_snapshot(
        gpu,
        painting,
        LAYER_PRESS_X,
        STEP_PRESSURE,
    ))
}

fn ferrule_z_px(snapshot: &BristleSnapshot) -> f32 {
    let roots = &snapshot.chains_px;
    roots.iter().map(|chain| chain[0][2]).sum::<f32>() / roots.len() as f32
}

#[test]
fn a_brush_pressed_onto_a_thick_dried_layer_holds_its_ferrule_as_high_above_it() {
    let gpu = GpuContext::new().unwrap();
    let above_bare_canvas_px = ferrule_height_px(&gpu, 0.0);
    let layer_px = relief_px(&session(&gpu), LAYER_HEIGHT_UM);
    let above_layer_px = ferrule_height_px(&gpu, LAYER_HEIGHT_UM) - layer_px;
    eprintln!(
        "ferrule {above_bare_canvas_px:.3} px above bare canvas, \
         {above_layer_px:.3} px above a {layer_px} px dried layer"
    );
    assert!(
        (above_layer_px - above_bare_canvas_px).abs() <= CONTACT_TOLERANCE_PX,
        "the ferrule rides {above_layer_px:.3} px above the layer, {above_bare_canvas_px:.3} px above bare canvas"
    );
}

#[test]
fn wet_paint_holds_the_hairs_up_only_by_its_support_share() {
    let gpu = GpuContext::new().unwrap();
    let above_bare_canvas_px = ferrule_height_px(&gpu, 0.0);
    let mut painting = session(&gpu);
    painting
        .engine
        .params_mut()
        .set("bristle_wet_support", WET_SUPPORT);
    layered(&gpu, &painting, StateTexture::WetVol, |_| LAYER_HEIGHT_UM);
    let supported_px = relief_px(&painting, WET_SUPPORT * LAYER_HEIGHT_UM);
    let lifted_px = ferrule_height_on(&gpu, &mut painting) - above_bare_canvas_px;
    eprintln!("a {supported_px} px wet support lifts the ferrule {lifted_px:.3} px");
    assert!(
        lifted_px >= MIN_WET_LIFT_SHARE * supported_px
            && lifted_px <= supported_px + CONTACT_TOLERANCE_PX,
        "a {supported_px} px wet support lifted the ferrule {lifted_px:.3} px"
    );
}

fn on_ridge(x: usize) -> bool {
    x % RIDGE_PERIOD_PX < RIDGE_WIDTH_PX
}

fn hair_length_px(chain: &[[f32; 3]]) -> f32 {
    chain
        .windows(2)
        .map(|pair| {
            let [a, b] = [pair[0], pair[1]];
            ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt()
        })
        .sum()
}

fn reach_px(snapshot: &BristleSnapshot) -> f32 {
    snapshot
        .chains_px
        .iter()
        .map(|chain| hair_length_px(chain))
        .fold(0.0, f32::max)
}

fn share_of_tips_over_ridges(snapshot: &BristleSnapshot) -> f32 {
    let over_ridges = snapshot
        .tips_px
        .iter()
        .filter(|tip| on_ridge(tip[0].max(0.0) as usize))
        .count();
    over_ridges as f32 / snapshot.tips_px.len() as f32
}

#[test]
fn a_light_touch_on_ridges_taller_than_its_press_is_carried_by_their_tops() {
    let gpu = GpuContext::new().unwrap();
    let mut painting = session(&gpu);
    dried(&gpu, &painting, |x| {
        if on_ridge(x) { RIDGE_HEIGHT_UM } else { 0.0 }
    });
    let snapshot = pressed_snapshot(&gpu, &mut painting, 240.0, LIGHT_PRESSURE);
    let ridge_px = relief_px(&painting, RIDGE_HEIGHT_UM);
    let reach_px = reach_px(&snapshot);
    let press_px = LIGHT_PRESSURE * PRESS_DEPTH_SHARE * reach_px;
    let sink_px = ridge_px + reach_px - ferrule_z_px(&snapshot);
    let carried_share = share_of_tips_over_ridges(&snapshot);
    let shared_press_px = press_px / carried_share.max(f32::MIN_POSITIVE);
    eprintln!(
        "ferrule sinks {sink_px:.2} px below first touch on the {ridge_px} px ridges; \
         a {carried_share:.2} share of the tips over them share a {press_px:.2} px press \
         as {shared_press_px:.2} px"
    );
    assert!(snapshot.touching.iter().any(|&touching| touching));
    assert!(
        sink_px <= SHARED_PRESS_MARGIN * shared_press_px,
        "the ferrule sank {sink_px:.2} px into ridges that carry {shared_press_px:.2} px each"
    );
}

#[test]
fn hairs_pulled_into_clumps_across_a_dried_edge_stay_on_top_of_it() {
    let gpu = GpuContext::new().unwrap();
    let mut painting = session(&gpu);
    painting
        .engine
        .params_mut()
        .set("bristle_clumping", FULL_CLUMPING);
    dried(&gpu, &painting, |x| {
        if x >= STEP_EDGE_X {
            STEP_HEIGHT_UM
        } else {
            0.0
        }
    });
    let start_x = STEP_EDGE_X as f64 - PRESS_STEP_PX * f64::from(PRESS_SAMPLES) / 2.0;
    let snapshot = pressed_snapshot(&gpu, &mut painting, start_x, STEP_PRESSURE);
    let step_px = relief_px(&painting, STEP_HEIGHT_UM);
    let lowest_on_step = snapshot
        .chains_px
        .iter()
        .flat_map(|chain| chain[1..].iter())
        .filter(|vertex| vertex[0] >= STEP_EDGE_X as f32 + EDGE_MARGIN_PX)
        .map(|vertex| vertex[2])
        .fold(f32::INFINITY, f32::min);
    eprintln!("the lowest clumped vertex over the {step_px} px step sits at z = {lowest_on_step}");
    assert!(
        lowest_on_step >= step_px - CONTACT_TOLERANCE_PX,
        "a clumped vertex sank to z = {lowest_on_step} into the {step_px} px step"
    );
}

#[test]
fn hairs_bridge_the_canvas_weave_instead_of_following_it() {
    let gpu = GpuContext::new().unwrap();
    let mut painting = session(&gpu);
    let snapshot = pressed_snapshot(&gpu, &mut painting, LAYER_PRESS_X, STEP_PRESSURE);
    let resting: Vec<f32> = snapshot
        .chains_px
        .iter()
        .zip(&snapshot.touching)
        .filter(|(_, touching)| **touching)
        .map(|(chain, _)| {
            chain[1..]
                .iter()
                .map(|vertex| vertex[2])
                .fold(f32::INFINITY, f32::min)
        })
        .collect();
    let highest_px = resting.iter().copied().fold(0.0, f32::max);
    eprintln!(
        "{} hairs rest on the canvas, the highest at z = {highest_px}",
        resting.len()
    );
    assert!(!resting.is_empty());
    assert!(
        highest_px <= CONTACT_TOLERANCE_PX,
        "a resting hair rose to z = {highest_px} over the canvas weave"
    );
}
