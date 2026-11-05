pub mod support;

use cadmium_engine::{
    brush::BrushShape,
    canvas::*,
    gpu::GpuContext,
    session::PaintSession,
    stroke::{StrokeSample, Tool},
};
use support::{encode_frame, median};

const SIDE: u32 = 512;
const CENTRE: f64 = 256.0;
const DIAMETER_PX: f32 = 120.0;
const STROKE_HALF_LENGTH_PX: f64 = 120.0;
const SAMPLE_SPACING_PX: f64 = 3.0;
const BODY_HALF_LENGTH_PX: f64 = STROKE_HALF_LENGTH_PX / 2.0;
const MIN_FAN_TUFTS: usize = 4;
const PRESSURE: f32 = 0.8;
const LEANING_TILT_DEG: f32 = 40.0;
const PAINTED_UM: f32 = 1.0;
const ANGLE_TOLERANCE_RAD: f32 = 1e-3;
const MAX_FLAT_THICKNESS_SHARE: f32 = 0.3;
const MIN_BROADSIDE_TO_EDGE_RATIO: f64 = 2.5;
const LIGHT_TOUCH_PRESSURE: f32 = 0.1;
const FULL_PRESSURE: f32 = 1.0;
const MAX_LIGHT_TOUCH_WIDTH_SHARE: f64 = 0.4;
const MIN_FULL_PRESS_WIDTH_SHARE: f64 = 0.9;

fn session(gpu: &GpuContext, shape: BrushShape) -> PaintSession {
    let mut s = PaintSession::new(
        gpu,
        CanvasSpec {
            width: SIDE,
            height: SIDE,
            primer: Primer::WhiteGesso,
            weave: Weave::Smooth,
            pigment_slots: vec!["ultramarine_blue".into()],
        },
    )
    .unwrap();
    s.brush.shape = shape;
    s.brush.geometry.max_diameter_px = DIAMETER_PX;
    s.brush.recipe = vec![(0, 1.0)];
    s.reload_brush(gpu);
    s
}

fn pen(tilt_x_deg: f32, tilt_y_deg: f32) -> StrokeSample {
    StrokeSample {
        x: CENTRE,
        y: CENTRE,
        pressure: PRESSURE,
        tilt_x_deg,
        tilt_y_deg,
        time_ms: 0,
    }
}

fn axial_difference(a: f32, b: f32) -> f32 {
    let difference = (a - b).rem_euclid(std::f32::consts::PI);
    difference.min(std::f32::consts::PI - difference)
}

fn cross_section(volume: &[f32], horizontal_stroke: bool, along: usize) -> Vec<bool> {
    let side = SIDE as usize;
    (0..side)
        .map(|across| {
            let index = if horizontal_stroke {
                across * side + along
            } else {
                along * side + across
            };
            volume[index] > PAINTED_UM
        })
        .collect()
}

fn body_cross_sections(
    gpu: &GpuContext,
    s: &PaintSession,
    horizontal_stroke: bool,
) -> Vec<Vec<bool>> {
    let volume = s.canvas.read_f32(gpu, StateTexture::WetVol).unwrap();
    let half_body = BODY_HALF_LENGTH_PX as usize;
    let centre = CENTRE as usize;
    (centre - half_body..=centre + half_body)
        .map(|along| cross_section(&volume, horizontal_stroke, along))
        .collect()
}

fn painted_width(gpu: &GpuContext, s: &PaintSession, horizontal_stroke: bool) -> f64 {
    let widths = body_cross_sections(gpu, s, horizontal_stroke)
        .iter()
        .map(|section| section.iter().filter(|&&painted| painted).count())
        .collect();
    median(widths) as f64
}

fn painted_extent(gpu: &GpuContext, s: &PaintSession, horizontal_stroke: bool) -> f64 {
    let extents = body_cross_sections(gpu, s, horizontal_stroke)
        .iter()
        .map(|section| {
            let first = section.iter().position(|&painted| painted);
            let last = section.iter().rposition(|&painted| painted);
            first.zip(last).map_or(0, |(first, last)| last + 1 - first)
        })
        .collect();
    median(extents) as f64
}

fn painted_runs(section: &[bool]) -> usize {
    section
        .windows(2)
        .filter(|pair| pair[1] && !pair[0])
        .count()
        + usize::from(section[0])
}

fn paint_straight(gpu: &GpuContext, s: &mut PaintSession, horizontal: bool) {
    paint_straight_pressed(gpu, s, horizontal, PRESSURE);
}

fn paint_straight_pressed(gpu: &GpuContext, s: &mut PaintSession, horizontal: bool, pressure: f32) {
    paint_straight_with(
        gpu,
        s,
        horizontal,
        StrokeSample {
            pressure,
            ..pen(0.0, 0.0)
        },
    );
}

fn paint_leaning(gpu: &GpuContext, s: &mut PaintSession, tilt_x_deg: f32) {
    paint_straight_with(gpu, s, false, pen(tilt_x_deg, 0.0));
}

fn paint_straight_with(
    gpu: &GpuContext,
    s: &mut PaintSession,
    horizontal: bool,
    nib: StrokeSample,
) {
    let count = (2.0 * STROKE_HALF_LENGTH_PX / SAMPLE_SPACING_PX) as u32;
    for i in 0..=count {
        let along = -STROKE_HALF_LENGTH_PX + f64::from(i) * SAMPLE_SPACING_PX;
        let (x, y) = if horizontal {
            (CENTRE + along, CENTRE)
        } else {
            (CENTRE, CENTRE + along)
        };
        let sample = StrokeSample {
            x,
            y,
            time_ms: i * 4,
            ..nib
        };
        if i == 0 {
            s.begin_stroke(gpu, sample)
        } else {
            s.add_samples(&[sample])
        }
    }
    s.end_stroke();
    encode_frame(gpu, s);
}

#[test]
fn a_round_brush_outline_is_a_circle_when_the_pen_is_upright() {
    let gpu = GpuContext::new().unwrap();
    let s = session(&gpu, BrushShape::Round);
    let outline = s.brush_outline(&pen(0.0, 0.0)).unwrap();
    assert_eq!(outline.semi_axes[0], outline.semi_axes[1]);
}

#[test]
fn a_leaning_round_brush_outline_stays_a_circle_under_the_nib() {
    let gpu = GpuContext::new().unwrap();
    let s = session(&gpu, BrushShape::Round);
    let upright = s.brush_outline(&pen(0.0, 0.0)).unwrap();
    let leaning = s.brush_outline(&pen(LEANING_TILT_DEG, 0.0)).unwrap();
    assert_eq!(leaning.semi_axes, upright.semi_axes);
}

#[test]
fn the_blender_shows_the_outline_of_its_bristles() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu, BrushShape::Round);
    s.brush.tool = Tool::Blender;
    let outline = s.brush_outline(&pen(0.0, 0.0)).unwrap();
    assert_eq!(outline.semi_axes, [DIAMETER_PX / 2.0; 2]);
}

#[test]
fn a_flat_brush_outline_is_a_horizontal_bar_when_the_pen_is_upright() {
    let gpu = GpuContext::new().unwrap();
    let s = session(&gpu, BrushShape::Flat);
    let outline = s.brush_outline(&pen(0.0, 0.0)).unwrap();
    let [thickness, width] = outline.semi_axes;
    assert!(
        thickness < MAX_FLAT_THICKNESS_SHARE * width,
        "{thickness} vs {width}"
    );
    assert!(
        axial_difference(outline.orientation, std::f32::consts::FRAC_PI_2) < ANGLE_TOLERANCE_RAD
    );
}

#[test]
fn a_fan_brush_outline_is_thinner_than_a_flat_one() {
    let gpu = GpuContext::new().unwrap();
    let flat = session(&gpu, BrushShape::Flat);
    let fan = session(&gpu, BrushShape::Fan);
    let flat_outline = flat.brush_outline(&pen(0.0, 0.0)).unwrap();
    let fan_outline = fan.brush_outline(&pen(0.0, 0.0)).unwrap();
    assert!(fan_outline.semi_axes[0] < flat_outline.semi_axes[0]);
    assert_eq!(fan_outline.semi_axes[1], flat_outline.semi_axes[1]);
}

#[test]
fn a_flat_brush_turns_its_thin_side_towards_the_lean_of_the_pen() {
    let gpu = GpuContext::new().unwrap();
    let s = session(&gpu, BrushShape::Flat);
    let outline = s.brush_outline(&pen(LEANING_TILT_DEG, 0.0)).unwrap();
    assert!(axial_difference(outline.orientation, 0.0) < ANGLE_TOLERANCE_RAD);
}

#[test]
fn a_flat_brush_paints_broadside_wide_and_edgewise_thin() {
    let gpu = GpuContext::new().unwrap();
    let mut edgewise = session(&gpu, BrushShape::Flat);
    paint_straight(&gpu, &mut edgewise, true);
    let edge = painted_width(&gpu, &edgewise, true);
    let mut broadside = session(&gpu, BrushShape::Flat);
    paint_straight(&gpu, &mut broadside, false);
    let broad = painted_width(&gpu, &broadside, false);
    eprintln!("edgewise {edge} px, broadside {broad} px");
    assert!(edge > 0.0);
    assert!(
        broad / edge > MIN_BROADSIDE_TO_EDGE_RATIO,
        "{broad} / {edge}"
    );
}

#[test]
fn a_fan_brush_paints_separate_tufts() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu, BrushShape::Fan);
    paint_straight(&gpu, &mut s, false);
    let runs: Vec<usize> = body_cross_sections(&gpu, &s, false)
        .iter()
        .map(|section| painted_runs(section))
        .collect();
    eprintln!("painted runs across the fan stroke {runs:?}");
    let tufts = median(runs);
    assert!(
        tufts >= MIN_FAN_TUFTS,
        "a cross-section of the fan stroke shows {tufts} tufts"
    );
}

fn round_width_at(gpu: &GpuContext, pressure: f32) -> f64 {
    let mut s = session(gpu, BrushShape::Round);
    paint_straight_pressed(gpu, &mut s, true, pressure);
    painted_extent(gpu, &s, true)
}

#[test]
fn a_light_touch_with_a_wet_round_brush_paints_a_thin_line() {
    let gpu = GpuContext::new().unwrap();
    let light = round_width_at(&gpu, LIGHT_TOUCH_PRESSURE);
    let full = round_width_at(&gpu, FULL_PRESSURE);
    let ferrule = f64::from(DIAMETER_PX);
    eprintln!("a {ferrule} px round paints {light} px at a light touch, {full} px pressed");
    assert!(
        light <= MAX_LIGHT_TOUCH_WIDTH_SHARE * ferrule,
        "a light touch paints {light} px"
    );
    assert!(
        full >= MIN_FULL_PRESS_WIDTH_SHARE * ferrule,
        "a full press paints only {full} px"
    );
}

#[test]
fn a_fan_brush_paints_whichever_way_the_pen_leans() {
    let gpu = GpuContext::new().unwrap();
    for tilt_x_deg in [LEANING_TILT_DEG, -LEANING_TILT_DEG] {
        let mut s = session(&gpu, BrushShape::Fan);
        paint_leaning(&gpu, &mut s, tilt_x_deg);
        let volume = s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
        assert!(
            volume.iter().any(|&um| um > PAINTED_UM),
            "a fan leaning {tilt_x_deg}° laid no paint"
        );
    }
}
