use std::path::PathBuf;

use cadmium_engine::{
    canvas::*,
    engine::{PaintEngine, blade_gap_um},
    gpu::GpuContext,
    present::{ViewTransform, readback_rgba8},
    render::ViewMode,
    session::PaintSession,
    stroke::{StrokeSample, Tool, pen_angles},
};

const PREVIEW_WIDTH: u32 = 512;
const PREVIEW_HEIGHT: u32 = 256;
const CANVAS_WIDTH: usize = 512;
const SECTION_COLUMNS: std::ops::Range<usize> = 150..350;
const STROKE_EXTENT_SHARE: f32 = 0.05;
const CONSERVATION_TOLERANCE: f64 = 1e-6;
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

fn session(gpu: &GpuContext) -> PaintSession {
    session_of_width(gpu, CANVAS_WIDTH as u32)
}

fn session_of_width(gpu: &GpuContext, width: u32) -> PaintSession {
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let session = PaintSession::new(
        gpu,
        CanvasSpec {
            width,
            height: 256,
            primer: Primer::WhiteGesso,
            weave: Weave::Linen,
            pigment_slots: vec![
                "ultramarine_blue".into(),
                "cadmium_yellow".into(),
                "titanium_white".into(),
            ],
        },
    );
    let error = pollster::block_on(scope.pop());
    assert!(error.is_none(), "wgpu error: {error:?}");
    session
}

fn frame(gpu: &GpuContext, s: &mut PaintSession) {
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(gpu, &mut e);
    gpu.queue.submit([e.finish()]);
    let error = pollster::block_on(scope.pop());
    assert!(error.is_none(), "wgpu error: {error:?}");
}

fn drag(gpu: &GpuContext, s: &mut PaintSession, samples: &[StrokeSample]) {
    for (i, sample) in samples.iter().enumerate() {
        if i == 0 {
            s.begin_stroke(gpu, *sample)
        } else {
            s.add_samples(&[*sample])
        }
        if i % 4 == 0 {
            frame(gpu, s);
        }
    }
    s.end_stroke(gpu);
    frame(gpu, s);
}

fn line(y: f64, from: f64, to: f64, pressure: f32, tilt_x_deg: f32) -> Vec<StrokeSample> {
    let steps = 60;
    (0..=steps)
        .map(|i| StrokeSample {
            x: from + (to - from) * i as f64 / steps as f64,
            y,
            pressure,
            tilt_x_deg,
            tilt_y_deg: 0.0,
            time_ms: i * 4,
        })
        .collect()
}

fn column(x: f64, from: f64, to: f64, pressure: f32, tilt_x_deg: f32) -> Vec<StrokeSample> {
    line(0.0, from, to, pressure, tilt_x_deg)
        .into_iter()
        .map(|sample| StrokeSample {
            x,
            y: sample.x,
            ..sample
        })
        .collect()
}

fn stroke(gpu: &GpuContext, s: &mut PaintSession, y: f64, from: f64, to: f64) {
    drag(gpu, s, &line(y, from, to, 0.8, 20.0));
}

fn total(gpu: &GpuContext, s: &PaintSession) -> PaintTotals {
    let c = s.canvas.wet_totals(gpu);
    let mut t = c;
    for tool in [s.engine.brush_totals(gpu), s.engine.knife_totals(gpu)] {
        t.volume += tool.volume;
        for i in 0..8 {
            t.pigment[i] += tool.pigment[i];
        }
    }
    t
}

fn write_preview(gpu: &GpuContext, s: &PaintSession, name: &str) {
    let target = s
        .presenter
        .create_target(gpu, PREVIEW_WIDTH, PREVIEW_HEIGHT);
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.presenter.encode_present(
        gpu,
        &s.canvas,
        ViewTransform {
            origin_x: 0.0,
            origin_y: 0.0,
            zoom: 1.0,
        },
        &target,
        &mut e,
    );
    gpu.queue.submit([e.finish()]);
    let rgba = readback_rgba8(gpu, &target);
    let target_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .expect("cargo target tmpdir lives inside the target directory")
        .to_path_buf();
    let file = std::fs::File::create(target_dir.join(name)).unwrap();
    let mut encoder =
        png::Encoder::new(std::io::BufWriter::new(file), PREVIEW_WIDTH, PREVIEW_HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&rgba)
        .unwrap();
}

fn relative_drift(before: &PaintTotals, after: &PaintTotals) -> (f64, Vec<f64>) {
    (
        (after.volume - before.volume) / before.volume,
        (0..3)
            .map(|i| (after.pigment[i] - before.pigment[i]) / before.volume)
            .collect(),
    )
}

#[test]
fn spreading_conserves_volume() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let before = total(&gpu, &s);
    s.brush.tool = Tool::Blender;
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    let after = total(&gpu, &s);
    let (volume, pigment) = relative_drift(&before, &after);
    eprintln!("blender volume drift {volume:e}, pigment drift {pigment:?}");
    assert!(volume.abs() < CONSERVATION_TOLERANCE);
}

#[test]
fn brush_and_knife_spreading_conserve_volume_and_each_pigment() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let before = total(&gpu, &s);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    stroke(&gpu, &mut s, 140.0, 470.0, 40.0);
    s.brush.tool = Tool::Knife;
    drag(&gpu, &mut s, &line(134.0, 40.0, 470.0, 1.0, 60.0));
    drag(&gpu, &mut s, &line(120.0, 470.0, 40.0, 1.0, 0.0));
    let after = total(&gpu, &s);
    let (volume, pigment) = relative_drift(&before, &after);
    eprintln!("brush+knife volume drift {volume:e}, pigment drift {pigment:?}");
    assert!(volume.abs() < CONSERVATION_TOLERANCE);
    assert!(pigment.iter().all(|drift| drift.abs() < 5e-3));
}

fn cross_section(gpu: &GpuContext, s: &PaintSession, rows: std::ops::Range<usize>) -> Vec<f32> {
    let vol = s.canvas.read_f32(gpu, StateTexture::WetVol);
    rows.map(|y| {
        let sum: f32 = SECTION_COLUMNS.map(|x| vol[y * CANVAS_WIDTH + x]).sum();
        sum / SECTION_COLUMNS.len() as f32
    })
    .collect()
}

fn band_means(profile: &[f32]) -> (f32, f32) {
    let peak = profile.iter().cloned().fold(0.0, f32::max);
    let painted: Vec<usize> = (0..profile.len())
        .filter(|&row| profile[row] > STROKE_EXTENT_SHARE * peak)
        .collect();
    let (first, last) = (painted[0], painted[painted.len() - 1]);
    let band = (last - first + 1) / 4;
    let mean =
        |rows: Vec<usize>| rows.iter().map(|&row| profile[row]).sum::<f32>() / rows.len() as f32;
    let outer = mean(
        (first..first + band)
            .chain(last + 1 - band..=last)
            .collect(),
    );
    let central = mean((first + band..=last - band).collect());
    (outer, central)
}

fn diagonal(from: (f64, f64), to: (f64, f64), pressure: f32, tilt_x_deg: f32) -> Vec<StrokeSample> {
    line(0.0, 0.0, 1.0, pressure, tilt_x_deg)
        .into_iter()
        .map(|sample| StrokeSample {
            x: from.0 + (to.0 - from.0) * sample.x,
            y: from.1 + (to.1 - from.1) * sample.x,
            ..sample
        })
        .collect()
}

#[test]
fn strokes_running_off_the_canvas_edges_conserve_volume() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let before = total(&gpu, &s);
    drag(&gpu, &mut s, &line(3.0, -30.0, 540.0, 1.0, 20.0));
    drag(&gpu, &mut s, &column(508.0, 280.0, -30.0, 1.0, 50.0));
    let edged = total(&gpu, &s);
    s.brush.geometry.max_diameter_px = 500.0;
    s.reload_brush(&gpu);
    let reloaded = total(&gpu, &s);
    drag(
        &gpu,
        &mut s,
        &diagonal((-40.0, 300.0), (560.0, -40.0), 1.0, 60.0),
    );
    s.brush.tool = Tool::Knife;
    drag(
        &gpu,
        &mut s,
        &diagonal((560.0, 290.0), (-40.0, -30.0), 1.0, 0.0),
    );
    let after = total(&gpu, &s);
    let (first, _) = relative_drift(&before, &edged);
    let (second, pigment) = relative_drift(&reloaded, &after);
    eprintln!("edge volume drift {first:e} then {second:e}, pigment drift {pigment:?}");
    assert!(first.abs() < CONSERVATION_TOLERANCE && second.abs() < CONSERVATION_TOLERANCE);
}

#[test]
fn brush_stroke_piles_paint_at_its_edges() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.engine.params_mut().set("bristle_contrast", 0.0);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let profile = cross_section(&gpu, &s, 96..161);
    eprintln!("cross-section {profile:?}");
    let (outer, central) = band_means(&profile);
    assert!(outer > central, "outer band {outer} central band {central}");
}

fn band_volume(gpu: &GpuContext, s: &PaintSession, rows: std::ops::Range<usize>) -> f32 {
    cross_section(gpu, s, rows).iter().sum()
}

#[test]
fn a_knife_tap_without_motion_leaves_the_canvas_untouched() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let painted = s.canvas.read_f32(&gpu, StateTexture::WetVol);
    s.canvas.wet_tiles.take();
    s.brush.tool = Tool::Knife;
    drag(&gpu, &mut s, &line(128.0, 256.0, 256.0, 1.0, 0.0)[..1]);
    assert!(s.canvas.wet_tiles.is_empty());
    assert_eq!(s.canvas.read_f32(&gpu, StateTexture::WetVol), painted);
}

#[test]
fn spread_mixtures_keep_their_fractions_within_the_volume() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.medium = 0.0;
    s.brush.recipe = vec![(0, 0.31), (1, 0.27), (2, 0.42)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 120.0, 40.0, 470.0);
    s.brush.recipe = vec![(1, 0.6), (2, 0.4)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 132.0, 470.0, 40.0);
    s.brush.tool = Tool::Blender;
    s.clean_brush(&gpu);
    drag(&gpu, &mut s, &column(256.0, 60.0, 200.0, 1.0, 20.0));
    s.brush.tool = Tool::Knife;
    drag(&gpu, &mut s, &line(126.0, 40.0, 470.0, 1.0, 60.0));
    let low = s.canvas.read_f32(&gpu, StateTexture::WetPig0);
    let high = s.canvas.read_f32(&gpu, StateTexture::WetPig1);
    let worst = low
        .chunks(4)
        .zip(high.chunks(4))
        .map(|(low, high)| low.iter().chain(high).sum::<f32>())
        .fold(0.0, f32::max);
    assert!(worst <= 1.0, "fractions sum to {worst}");
}

#[test]
fn blender_previews_show_smeared_colours() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 100.0, 40.0, 470.0);
    s.brush.recipe = vec![(1, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 150.0, 40.0, 470.0);
    s.brush.tool = Tool::Blender;
    s.clean_brush(&gpu);
    for (x, from, to) in [
        (120.0, 60.0, 200.0),
        (200.0, 200.0, 60.0),
        (280.0, 60.0, 200.0),
        (360.0, 200.0, 60.0),
    ] {
        drag(&gpu, &mut s, &column(x, from, to, 1.0, 20.0));
    }
    write_preview(&gpu, &s, "engine-blend.png");
    assert!(s.canvas.wet_totals(&gpu).volume > 0.0);
}

fn dip_knife(gpu: &GpuContext, s: &mut PaintSession) {
    s.brush.tool = Tool::Knife;
    let mixture = s.brush.reservoir_mixture(SLOT_COUNT);
    s.dip(gpu, &mixture, FULL_DIP);
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
    let dry = s.canvas.read_f32(gpu, StateTexture::DryHeight);
    let wet = s.canvas.read_f32(gpu, StateTexture::WetVol);
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
    let brush = s.engine.brush_totals(&gpu);
    s.brush.tool = Tool::Knife;
    drag(&gpu, &mut s, &laid_knife(KNIFE_ROW, LIGHT_PRESSURE));
    assert_eq!(s.canvas.wet_totals(&gpu).volume, 0.0);
    assert_eq!(s.engine.brush_totals(&gpu), brush);
}

#[test]
fn dipping_with_the_knife_loads_the_knife_and_leaves_the_brush_as_it_was() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let brush = s.engine.brush_totals(&gpu);
    dip_knife(&gpu, &mut s);
    assert!(s.engine.knife_totals(&gpu).volume > 0.0);
    assert_eq!(s.engine.brush_totals(&gpu), brush);
    s.clean_brush(&gpu);
    assert_eq!(s.engine.knife_totals(&gpu).volume, 0.0);
    assert_eq!(s.engine.brush_totals(&gpu), brush);
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
    let loaded = s.engine.knife_totals(&gpu).volume;
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
    let wet = s.canvas.read_f32(gpu, StateTexture::WetVol);
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
    assert!(s.canvas.wet_totals(&gpu).volume > 0.0);
}
