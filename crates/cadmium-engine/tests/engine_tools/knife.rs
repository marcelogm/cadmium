use cadmium_engine::{
    canvas::StateTexture,
    engine::{PaintEngine, blade_gap_um},
    gpu::GpuContext,
    render::ViewMode,
    session::PaintSession,
    stroke::{StrokeSample, Tool, pen_angles},
};

use crate::support::{drag, frame, line, session, session_of_width, stroke, write_preview};
use crate::{
    CANVAS_WIDTH, CONSERVATION_TOLERANCE, SECTION_COLUMNS, column, cross_section, relative_drift,
    total,
};

const SLOT_COUNT: usize = 3;
const FULL_DIP: f32 = 1.0;
const LAID_TILT_DEG: f32 = 60.0;
const UPRIGHT_TILT_DEG: f32 = 0.0;
const PLATE_PRESSURE: f32 = 0.5;
const LIGHT_PRESSURE: f32 = 0.3;
const HEAVY_PRESSURE: f32 = 0.8;
const FEATHER_PRESSURE: f32 = 0.1;
const KNIFE_ROW: f64 = 128.0;
const PLATE_INTERIOR: std::ops::Range<usize> = 112..145;
const KNIFE_SECTION: std::ops::Range<usize> = 96..161;
const LOWER_RIDGE: std::ops::Range<usize> = 103..109;
const UPPER_RIDGE: std::ops::Range<usize> = 147..153;
const GAP_TOLERANCE: f32 = 0.1;
const EDGE_HIGH_SHARE: f32 = 0.9;
const EDGE_LOW_SHARE: f32 = 0.1;
const SHARP_EDGE_PX: usize = 2;
const FURROW_SHARE: f32 = 0.6;
const PREDICTION_TOLERANCE: f64 = 1e-4;
const WIDE_CANVAS_WIDTH: usize = 1024;
const WIDE_STROKE_START_PX: f64 = 20.0;
const WIDE_STROKE_END_PX: f64 = 1004.0;
const FULL_GAP_PRESSURE: f32 = 0.0;
const PLATE_REACH_SHARE: f32 = 0.5;
const PLATE_START_COLUMNS: std::ops::Range<usize> = 40..140;
const SHORTEST_FULL_DIP_PLATE_PX: usize = 600;
const LONGEST_FULL_DIP_PLATE_PX: usize = 850;

fn band_volume(gpu: &GpuContext, s: &PaintSession, rows: std::ops::Range<usize>) -> f32 {
    cross_section(gpu, s, rows).iter().sum()
}

#[test]
fn a_knife_tap_without_motion_leaves_the_canvas_untouched() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let painted = s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
    s.canvas.wet_tiles.take();
    s.brush.tool = Tool::Knife;
    drag(&gpu, &mut s, &line(128.0, 256.0, 256.0, 1.0, 0.0)[..1]);
    assert!(s.canvas.wet_tiles.is_empty());
    assert_eq!(
        s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap(),
        painted
    );
}

fn dip_knife(gpu: &GpuContext, s: &mut PaintSession) {
    s.brush.tool = Tool::Knife;
    let mixture = s.brush.reservoir_mixture(SLOT_COUNT);
    s.dip(gpu, &mixture, FULL_DIP).unwrap();
}

fn laid_knife(y: f64, pressure: f32) -> Vec<StrokeSample> {
    line(y, 40.0, 470.0, pressure, LAID_TILT_DEG)
}

fn mean(values: &[f32]) -> f32 {
    values.iter().sum::<f32>() / values.len() as f32
}

fn plate_height_above_weave_peaks(
    gpu: &GpuContext,
    s: &PaintSession,
    rows: std::ops::Range<usize>,
) -> f32 {
    let dry = s.canvas.read_f32(gpu, StateTexture::DryHeight).unwrap();
    let wet = s.canvas.read_f32(gpu, StateTexture::WetVol).unwrap();
    let texels: Vec<usize> = rows
        .flat_map(|y| SECTION_COLUMNS.map(move |x| y * CANVAS_WIDTH + x))
        .collect();
    let top = mean(&texels.iter().map(|&i| dry[i] + wet[i]).collect::<Vec<_>>());
    let peaks = texels.iter().map(|&i| dry[i]).fold(f32::MIN, f32::max);
    top - peaks
}

fn widest_edge_drop(profile: &[f32], interior: f32) -> usize {
    let centre = profile.len() / 2;
    let drop_from = |rows: Vec<usize>| {
        let high = rows
            .iter()
            .position(|&row| profile[row] < EDGE_HIGH_SHARE * interior)
            .unwrap();
        let low = rows
            .iter()
            .position(|&row| profile[row] <= EDGE_LOW_SHARE * interior)
            .unwrap();
        low + 1 - high
    };
    drop_from((0..centre).rev().collect()).max(drop_from((centre..profile.len()).collect()))
}

#[test]
fn a_dipped_laid_knife_builds_a_plate_as_thick_as_its_gap_over_bare_canvas() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    dip_knife(&gpu, &mut s);
    drag(&gpu, &mut s, &laid_knife(KNIFE_ROW, PLATE_PRESSURE));
    let altitude = pen_angles(LAID_TILT_DEG, 0.0).altitude_rad;
    let gap = blade_gap_um(s.engine.params(), PLATE_PRESSURE, altitude);
    let thickness = plate_height_above_weave_peaks(&gpu, &s, PLATE_INTERIOR);
    eprintln!("plate {thickness} µm above the weave peaks for a {gap} µm gap");
    assert!((thickness - gap).abs() <= GAP_TOLERANCE * gap);
}

#[test]
fn a_knife_plate_ends_at_its_sides_in_a_sharp_edge() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    dip_knife(&gpu, &mut s);
    drag(&gpu, &mut s, &laid_knife(KNIFE_ROW, PLATE_PRESSURE));
    let profile = cross_section(&gpu, &s, KNIFE_SECTION);
    let interior = mean(&cross_section(&gpu, &s, PLATE_INTERIOR));
    let drop = widest_edge_drop(&profile, interior);
    eprintln!("plate section {profile:?}, edge drop over {drop} px");
    assert!(interior > 0.0);
    assert!(drop <= SHARP_EDGE_PX);
}

#[test]
fn an_upright_knife_ploughs_a_furrow_and_leaves_ridges_taller_than_the_paint_at_both_sides() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    for y in [116.0, 128.0, 140.0] {
        s.reload_brush(&gpu);
        stroke(&gpu, &mut s, y, 20.0, 490.0);
    }
    let before = cross_section(&gpu, &s, KNIFE_SECTION);
    let furrow_before = mean(&cross_section(&gpu, &s, PLATE_INTERIOR));
    s.brush.tool = Tool::Knife;
    drag(
        &gpu,
        &mut s,
        &line(KNIFE_ROW, 40.0, 470.0, 1.0, UPRIGHT_TILT_DEG),
    );
    let paint_peak = before.iter().cloned().fold(0.0, f32::max);
    let ridge = |rows| {
        cross_section(&gpu, &s, rows)
            .into_iter()
            .fold(0.0, f32::max)
    };
    let (lower, upper) = (ridge(LOWER_RIDGE), ridge(UPPER_RIDGE));
    let furrow = mean(&cross_section(&gpu, &s, PLATE_INTERIOR));
    eprintln!(
        "paint peak {paint_peak}, ridges {lower} and {upper}, furrow {furrow_before} -> {furrow}"
    );
    assert!(lower > paint_peak && upper > paint_peak);
    assert!(furrow < FURROW_SHARE * furrow_before);
}

#[test]
fn scraping_then_laying_with_the_knife_conserves_volume() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 60.0, 40.0, 470.0);
    stroke(&gpu, &mut s, 72.0, 470.0, 40.0);
    s.brush.recipe = vec![(1, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 200.0, 40.0, 470.0);
    let before = total(&gpu, &s);
    s.brush.tool = Tool::Knife;
    drag(&gpu, &mut s, &laid_knife(66.0, 1.0));
    drag(&gpu, &mut s, &laid_knife(KNIFE_ROW, LIGHT_PRESSURE));
    let laid = band_volume(&gpu, &s, PLATE_INTERIOR);
    drag(
        &gpu,
        &mut s,
        &line(200.0, 470.0, 40.0, 1.0, UPRIGHT_TILT_DEG),
    );
    let after = total(&gpu, &s);
    let (volume, pigment) = relative_drift(&before, &after);
    eprintln!("laid {laid}, scrape and lay volume drift {volume:e}, pigment drift {pigment:?}");
    assert!(laid > 0.0);
    assert!(volume.abs() <= CONSERVATION_TOLERANCE);
    assert!(pigment.iter().all(|drift| drift.abs() < 5e-3));
}

#[test]
fn light_knife_pressure_lays_a_thicker_plate_than_heavy_pressure() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    dip_knife(&gpu, &mut s);
    drag(&gpu, &mut s, &laid_knife(70.0, LIGHT_PRESSURE));
    dip_knife(&gpu, &mut s);
    drag(&gpu, &mut s, &laid_knife(180.0, HEAVY_PRESSURE));
    let light = mean(&cross_section(&gpu, &s, 54..87));
    let heavy = mean(&cross_section(&gpu, &s, 164..197));
    eprintln!("light plate {light} µm, heavy plate {heavy} µm");
    assert!(light > heavy && heavy > 0.0);
}

#[test]
fn an_undipped_knife_lays_none_of_the_brush_load_on_bare_canvas() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let brush = s.engine.brush_totals(&gpu).unwrap();
    s.brush.tool = Tool::Knife;
    drag(&gpu, &mut s, &laid_knife(KNIFE_ROW, LIGHT_PRESSURE));
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
    assert_eq!(s.engine.brush_totals(&gpu).unwrap(), brush);
}

#[test]
fn dipping_with_the_knife_loads_the_knife_and_leaves_the_brush_as_it_was() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let brush = s.engine.brush_totals(&gpu).unwrap();
    dip_knife(&gpu, &mut s);
    assert!(s.engine.knife_totals(&gpu).unwrap().volume > 0.0);
    assert_eq!(s.engine.brush_totals(&gpu).unwrap(), brush);
    s.clean_brush(&gpu);
    assert_eq!(s.engine.knife_totals(&gpu).unwrap().volume, 0.0);
    assert_eq!(s.engine.brush_totals(&gpu).unwrap(), brush);
}

#[test]
fn a_full_dip_lays_on_a_clean_knife_the_volume_the_engine_predicts() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let mixture = s.brush.reservoir_mixture(SLOT_COUNT);
    s.brush.tool = Tool::Knife;
    s.resize_tip();
    let predicted = f64::from(s.engine.knife_full_dip_volume(mixture.volume_um));
    assert!(predicted > 0.0);
    dip_knife(&gpu, &mut s);
    let loaded = s.engine.knife_totals(&gpu).unwrap().volume;
    assert!(
        (loaded - predicted).abs() <= predicted * PREDICTION_TOLERANCE,
        "{loaded} {predicted}"
    );
    assert!(s.engine.knife_full_dip_volume(2.0 * mixture.volume_um) > predicted as f32);
}

fn row_profile(
    gpu: &GpuContext,
    s: &PaintSession,
    width: usize,
    rows: std::ops::Range<usize>,
) -> Vec<f32> {
    let wet = s.canvas.read_f32(gpu, StateTexture::WetVol).unwrap();
    let count = rows.len() as f32;
    (0..width)
        .map(|x| rows.clone().map(|y| wet[y * width + x]).sum::<f32>() / count)
        .collect()
}

#[test]
fn a_full_knife_dip_lays_a_plate_most_of_a_canvas_width_long() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session_of_width(&gpu, WIDE_CANVAS_WIDTH as u32);
    dip_knife(&gpu, &mut s);
    drag(
        &gpu,
        &mut s,
        &line(
            KNIFE_ROW,
            WIDE_STROKE_START_PX,
            WIDE_STROKE_END_PX,
            FULL_GAP_PRESSURE,
            LAID_TILT_DEG,
        ),
    );
    let profile = row_profile(&gpu, &s, WIDE_CANVAS_WIDTH, PLATE_INTERIOR);
    let plate = mean(&profile[PLATE_START_COLUMNS]);
    let end = profile
        .iter()
        .rposition(|&height| height > PLATE_REACH_SHARE * plate)
        .unwrap();
    let length = end - WIDE_STROKE_START_PX as usize;
    eprintln!("a full dip laid a {plate} µm plate {length} px long");
    assert!((SHORTEST_FULL_DIP_PLATE_PX..=LONGEST_FULL_DIP_PLATE_PX).contains(&length));
}

#[test]
fn knife_previews_show_raised_plates_and_ploughed_ridges() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 56.0, 30.0, 480.0);
    stroke(&gpu, &mut s, 68.0, 480.0, 30.0);
    stroke(&gpu, &mut s, 80.0, 30.0, 480.0);
    s.brush.tool = Tool::Knife;
    drag(
        &gpu,
        &mut s,
        &line(68.0, 40.0, 330.0, 1.0, UPRIGHT_TILT_DEG),
    );
    s.brush.recipe = vec![(1, 1.0)];
    dip_knife(&gpu, &mut s);
    drag(
        &gpu,
        &mut s,
        &line(150.0, 40.0, 360.0, FEATHER_PRESSURE, LAID_TILT_DEG),
    );
    s.brush.recipe = vec![(2, 1.0)];
    dip_knife(&gpu, &mut s);
    drag(
        &gpu,
        &mut s,
        &line(200.0, 180.0, 470.0, PLATE_PRESSURE, LAID_TILT_DEG),
    );
    dip_knife(&gpu, &mut s);
    drag(
        &gpu,
        &mut s,
        &column(400.0, 20.0, 240.0, LIGHT_PRESSURE, LAID_TILT_DEG),
    );
    frame(&gpu, &mut s);
    write_preview(&gpu, &s, "knife-depth.png");
    s.view_mode = ViewMode::Height;
    s.canvas.dirty.mark_all();
    frame(&gpu, &mut s);
    write_preview(&gpu, &s, "knife-depth-height.png");
    assert!(s.canvas.wet_totals(&gpu).unwrap().volume > 0.0);
}
