pub mod support;

use cadmium_engine::{
    canvas::{StateTexture, Weave, weave_heights},
    gpu::GpuContext,
    pigments::{MAX_SLOTS, Mixture},
    session::PaintSession,
    stroke::Tool,
};
use support::{drag, frame, line, presented_rgba, session, stroke, without_wgpu_errors};

const WIDTH: u32 = 512;
const HEIGHT: u32 = 256;
const SKETCH_ROW: usize = 128;
const SKETCH_HALF_WIDTH: usize = 8;
const CLEAN_ROW: usize = 32;
const CENTRE_X: usize = 256;
const STROKE_FROM: f64 = 40.0;
const STROKE_TO: f64 = 470.0;
const GLAZE_PRESSURE: f32 = 0.15;
const GLAZE_TILT_DEG: f32 = 20.0;
const FULL_SKETCH: f32 = 1.0;
const HALF_SKETCH: f32 = 0.5;
const RGBA_CHANNELS: usize = 4;
const BANDS_PER_TEXTURE: usize = 4;
const DARKENED_RATIO_DIVISOR: u32 = 2;
const TITANIUM_WHITE: usize = 2;
const WHITE_LOAD_UM: f32 = 100.0;
const FULL_DIP: f32 = 1.0;
const OPAQUE_PASSES: usize = 6;
const HIDDEN_SHARE_OF_BARE_CONTRAST: f32 = 0.05;
const SHOWS_THROUGH_RATIO: f32 = 0.9;
const PENCIL_TILT_DEG: f32 = 0.0;
const FIRM_PRESSURE: f32 = 0.9;
const LIGHT_PRESSURE: f32 = 0.2;
const DARKER_RATIO: f32 = 1.5;
const TOOTH_RATIO: f32 = 1.5;
const TOOTH_BAND_HALF_HEIGHT: usize = 1;
const TOOTH_BAND_FROM_X: usize = 100;
const TOOTH_BAND_TO_X: usize = 400;

fn sketch_row(gpu: &GpuContext, s: &mut PaintSession, density: f32) {
    let mut sketch = vec![0.0; (WIDTH * HEIGHT) as usize];
    for y in SKETCH_ROW - SKETCH_HALF_WIDTH..SKETCH_ROW + SKETCH_HALF_WIDTH {
        for x in 0..WIDTH as usize {
            sketch[y * WIDTH as usize + x] = density;
        }
    }
    s.canvas.write_f32(gpu, StateTexture::Graphite, &sketch);
    s.canvas.dirty.mark_all();
}

fn rgb_sum_at(rgba: &[u8], x: usize, y: usize) -> u32 {
    let i = (y * WIDTH as usize + x) * RGBA_CHANNELS;
    u32::from(rgba[i]) + u32::from(rgba[i + 1]) + u32::from(rgba[i + 2])
}

fn centre() -> usize {
    SKETCH_ROW * WIDTH as usize + CENTRE_X
}

#[test]
fn graphite_darkens_the_bare_canvas() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    sketch_row(&gpu, &mut s, FULL_SKETCH);
    frame(&gpu, &mut s);
    let rgba = presented_rgba(&gpu, &s, WIDTH, HEIGHT);
    assert!(
        rgb_sum_at(&rgba, CENTRE_X, SKETCH_ROW)
            < rgb_sum_at(&rgba, CENTRE_X, CLEAN_ROW) / DARKENED_RATIO_DIVISOR
    );
}

#[test]
fn drying_paint_over_graphite_bakes_it_in() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    sketch_row(&gpu, &mut s, FULL_SKETCH);
    stroke(&gpu, &mut s, SKETCH_ROW as f64, STROKE_FROM, STROKE_TO);
    assert!(s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap()[centre()] > 0.0);
    without_wgpu_errors(&gpu, || s.dry(&gpu));
    let graphite = s.canvas.read_f32(&gpu, StateTexture::Graphite).unwrap();
    assert_eq!(graphite[centre()], 0.0);
}

#[test]
fn graphite_outside_the_paint_survives_drying() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    sketch_row(&gpu, &mut s, HALF_SKETCH);
    stroke(&gpu, &mut s, CLEAN_ROW as f64, STROKE_FROM, STROKE_TO);
    without_wgpu_errors(&gpu, || s.dry(&gpu));
    let graphite = s.canvas.read_f32(&gpu, StateTexture::Graphite).unwrap();
    assert_eq!(graphite[centre()], HALF_SKETCH);
}

fn dried_glaze_reflectance(gpu: &GpuContext, sketched: bool) -> Vec<f32> {
    let mut s = session(gpu);
    if sketched {
        sketch_row(gpu, &mut s, FULL_SKETCH);
    }
    thin_glaze(gpu, &mut s);
    assert!(s.canvas.read_f32(gpu, StateTexture::WetVol).unwrap()[centre()] > 0.0);
    without_wgpu_errors(gpu, || s.dry(gpu));
    let first = centre() * BANDS_PER_TEXTURE;
    [StateTexture::DryRefl0, StateTexture::DryRefl1]
        .into_iter()
        .flat_map(|which| {
            let refl = s.canvas.read_f32(gpu, which).unwrap();
            refl[first..first + BANDS_PER_TEXTURE].to_vec()
        })
        .collect()
}

#[test]
fn graphite_shows_through_a_dried_glaze() {
    let gpu = GpuContext::new().unwrap();
    let sketched = dried_glaze_reflectance(&gpu, true);
    let clean = dried_glaze_reflectance(&gpu, false);
    for (band, (over_graphite, over_canvas)) in sketched.iter().zip(&clean).enumerate() {
        assert!(over_graphite <= over_canvas, "band {band}: {over_graphite} vs {over_canvas}");
    }
    let sketched_total: f32 = sketched.iter().sum();
    let clean_total: f32 = clean.iter().sum();
    assert!(sketched_total < clean_total * SHOWS_THROUGH_RATIO);
}

fn white() -> Mixture {
    let mut fractions = [0.0; MAX_SLOTS];
    fractions[TITANIUM_WHITE] = 1.0;
    Mixture {
        volume_um: WHITE_LOAD_UM,
        fractions,
    }
}

fn shown_under_wet_paint(
    gpu: &GpuContext,
    sketched: bool,
    paint: impl Fn(&GpuContext, &mut PaintSession),
) -> u32 {
    let mut s = session(gpu);
    if sketched {
        sketch_row(gpu, &mut s, FULL_SKETCH);
    }
    paint(gpu, &mut s);
    assert!(s.canvas.read_f32(gpu, StateTexture::WetVol).unwrap()[centre()] > 0.0);
    frame(gpu, &mut s);
    rgb_sum_at(&presented_rgba(gpu, &s, WIDTH, HEIGHT), CENTRE_X, SKETCH_ROW)
}

fn thick_white(gpu: &GpuContext, s: &mut PaintSession) {
    without_wgpu_errors(gpu, || s.clean_brush(gpu));
    for _ in 0..OPAQUE_PASSES {
        without_wgpu_errors(gpu, || s.dip(gpu, &white(), FULL_DIP).unwrap());
        stroke(gpu, s, SKETCH_ROW as f64, STROKE_FROM, STROKE_TO);
    }
}

fn thin_glaze(gpu: &GpuContext, s: &mut PaintSession) {
    let glaze = line(SKETCH_ROW as f64, STROKE_FROM, STROKE_TO, GLAZE_PRESSURE, GLAZE_TILT_DEG);
    drag(gpu, s, &glaze);
}

fn bare_graphite_contrast(gpu: &GpuContext) -> u32 {
    let mut s = session(gpu);
    sketch_row(gpu, &mut s, FULL_SKETCH);
    frame(gpu, &mut s);
    let rgba = presented_rgba(gpu, &s, WIDTH, HEIGHT);
    rgb_sum_at(&rgba, CENTRE_X, CLEAN_ROW) - rgb_sum_at(&rgba, CENTRE_X, SKETCH_ROW)
}

#[test]
fn opaque_wet_paint_hides_the_graphite() {
    let gpu = GpuContext::new().unwrap();
    let over_graphite = shown_under_wet_paint(&gpu, true, thick_white);
    let over_canvas = shown_under_wet_paint(&gpu, false, thick_white);
    let residual = over_graphite.abs_diff(over_canvas) as f32;
    let bare = bare_graphite_contrast(&gpu) as f32;
    assert!(
        residual <= bare * HIDDEN_SHARE_OF_BARE_CONTRAST,
        "{over_graphite} vs {over_canvas}, bare contrast {bare}"
    );
}

#[test]
fn graphite_shows_through_a_wet_glaze() {
    let gpu = GpuContext::new().unwrap();
    let over_graphite = shown_under_wet_paint(&gpu, true, thin_glaze);
    let over_canvas = shown_under_wet_paint(&gpu, false, thin_glaze);
    assert!(over_graphite < over_canvas, "{over_graphite} vs {over_canvas}");
}

#[test]
fn undoing_a_dry_brings_the_graphite_back() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    sketch_row(&gpu, &mut s, FULL_SKETCH);
    stroke(&gpu, &mut s, SKETCH_ROW as f64, STROKE_FROM, STROKE_TO);
    without_wgpu_errors(&gpu, || s.dry(&gpu));
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    let graphite = s.canvas.read_f32(&gpu, StateTexture::Graphite).unwrap();
    assert_eq!(graphite[centre()], FULL_SKETCH);
}

fn pencil_line(gpu: &GpuContext, s: &mut PaintSession, pressure: f32) {
    s.brush.tool = Tool::Pencil;
    let samples = line(SKETCH_ROW as f64, STROKE_FROM, STROKE_TO, pressure, PENCIL_TILT_DEG);
    drag(gpu, s, &samples);
}

fn graphite(gpu: &GpuContext, s: &PaintSession) -> Vec<f32> {
    s.canvas.read_f32(gpu, StateTexture::Graphite).unwrap()
}

fn total_graphite(gpu: &GpuContext, pressure: f32) -> f32 {
    let mut s = session(gpu);
    pencil_line(gpu, &mut s, pressure);
    graphite(gpu, &s).iter().sum()
}

fn mean(values: &[f32]) -> f32 {
    values.iter().sum::<f32>() / values.len() as f32
}

#[test]
fn the_pencil_draws_graphite_and_no_paint() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let relief = s.canvas.read_f32(&gpu, StateTexture::DryHeight).unwrap();
    pencil_line(&gpu, &mut s, FIRM_PRESSURE);
    assert!(graphite(&gpu, &s)[centre()] > 0.0);
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
    assert_eq!(s.canvas.read_f32(&gpu, StateTexture::DryHeight).unwrap(), relief);
}

#[test]
fn pressing_harder_draws_darker() {
    let gpu = GpuContext::new().unwrap();
    let light = total_graphite(&gpu, LIGHT_PRESSURE);
    let firm = total_graphite(&gpu, FIRM_PRESSURE);
    assert!(firm > light * DARKER_RATIO, "{firm} vs {light}");
}

#[test]
fn a_light_touch_catches_the_tooth_of_the_linen() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    pencil_line(&gpu, &mut s, LIGHT_PRESSURE);
    let density = graphite(&gpu, &s);
    let heights = weave_heights(Weave::Linen, WIDTH, HEIGHT);
    let rows = SKETCH_ROW - TOOTH_BAND_HALF_HEIGHT..=SKETCH_ROW + TOOTH_BAND_HALF_HEIGHT;
    let band: Vec<usize> = rows
        .flat_map(|y| (TOOTH_BAND_FROM_X..TOOTH_BAND_TO_X).map(move |x| y * WIDTH as usize + x))
        .collect();
    let mut band_heights: Vec<f32> = band.iter().map(|&i| heights[i]).collect();
    band_heights.sort_by(f32::total_cmp);
    let median = band_heights[band_heights.len() / 2];
    let (peaks, valleys): (Vec<usize>, Vec<usize>) =
        band.iter().partition(|&&i| heights[i] > median);
    let on = |texels: &[usize]| mean(&texels.iter().map(|&i| density[i]).collect::<Vec<_>>());
    let (peak_mean, valley_mean) = (on(&peaks), on(&valleys));
    assert!(peak_mean > valley_mean * TOOTH_RATIO, "peaks {peak_mean} vs valleys {valley_mean}");
}

#[test]
fn undo_takes_back_a_pencil_line() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    pencil_line(&gpu, &mut s, FIRM_PRESSURE);
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    assert!(graphite(&gpu, &s).iter().all(|&density| density == 0.0));
}
