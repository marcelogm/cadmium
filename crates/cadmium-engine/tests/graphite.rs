pub mod support;

use cadmium_engine::{canvas::StateTexture, gpu::GpuContext, session::PaintSession};
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
const SHOWS_THROUGH_RATIO: f32 = 0.9;

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

fn luminance_at(rgba: &[u8], x: usize, y: usize) -> u32 {
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
        luminance_at(&rgba, CENTRE_X, SKETCH_ROW) < luminance_at(&rgba, CENTRE_X, CLEAN_ROW) / 2
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
    let glaze = line(SKETCH_ROW as f64, STROKE_FROM, STROKE_TO, GLAZE_PRESSURE, GLAZE_TILT_DEG);
    drag(gpu, &mut s, &glaze);
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
