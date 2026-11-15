use cadmium_engine::{
    canvas::{CanvasSpec, MARGIN_PX, StateTexture},
    gpu::GpuContext,
    session::PaintSession,
    stroke::StrokeSample,
};

use crate::support::{
    drag, frame, line, load, painting_request, painting_session, presented_rgba, stroke,
    without_wgpu_errors,
};

const GROUNDS_DIFFER_BY: u8 = 40;
const SAME_GROUND_WITHIN: u8 = 8;
const BLUE: usize = 0;
const YELLOW: usize = 1;

fn furthest_channel(a: &[u8], b: &[u8]) -> u8 {
    a.iter()
        .zip(b)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or_default()
}

fn assert_margin_has_its_own_ground(gpu: &GpuContext, s: &mut PaintSession) {
    let (width, height) = (s.canvas.spec.width, s.canvas.spec.height);
    let painting = s.canvas.spec.painting_rect();
    frame(gpu, s);
    let rgba = presented_rgba(gpu, s, width, height);
    let pixel = |x: u32, y: u32| &rgba[((y * width + x) * 4) as usize..][..3];
    let in_margin = pixel(MARGIN_PX / 2, MARGIN_PX / 2);
    let in_painting = pixel(
        painting.x + painting.width / 2,
        painting.y + painting.height / 2,
    );
    assert!(furthest_channel(in_margin, in_painting) > GROUNDS_DIFFER_BY);
    let across = pixel(width - MARGIN_PX / 2, height - MARGIN_PX / 2);
    assert!(furthest_channel(across, in_margin) <= SAME_GROUND_WITHIN);
}

#[test]
fn the_margin_shows_a_ground_of_its_own_around_the_painting() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 256, 128);
    assert_margin_has_its_own_ground(&gpu, &mut s);
}

#[test]
fn a_session_swaps_one_margined_painting_for_another() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 256, 128);
    let next = CanvasSpec::for_painting(painting_request(192, 192));
    without_wgpu_errors(&gpu, || s.replace_document(&gpu, next.clone()).unwrap());
    assert_eq!(s.canvas.spec, next);
    assert_margin_has_its_own_ground(&gpu, &mut s);
}

fn margin_row() -> f64 {
    f64::from(MARGIN_PX) / 2.0
}

fn column(x: f64, from_y: f64, to_y: f64) -> Vec<StrokeSample> {
    line(x, from_y, to_y, 0.8, 20.0)
        .into_iter()
        .map(|sample| StrokeSample {
            x,
            y: sample.x,
            ..sample
        })
        .collect()
}

fn total_where(gpu: &GpuContext, s: &PaintSession, which: StateTexture, in_margin: bool) -> f64 {
    let spec = &s.canvas.spec;
    s.canvas
        .read_f32(gpu, which)
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            let index = *index as u32;
            spec.is_in_margin(index % spec.width, index / spec.width) == in_margin
        })
        .map(|(_, value)| f64::from(*value))
        .sum()
}

fn margin_wet_volume(gpu: &GpuContext, s: &PaintSession) -> f64 {
    total_where(gpu, s, StateTexture::WetVol, true)
}

fn painting_wet_volume(gpu: &GpuContext, s: &PaintSession) -> f64 {
    total_where(gpu, s, StateTexture::WetVol, false)
}

#[test]
fn a_brush_dragged_through_wet_paint_on_the_margin_picks_it_up() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 384, 256);
    load(&gpu, &mut s, YELLOW);
    stroke(&gpu, &mut s, margin_row(), 40.0, 400.0);
    load(&gpu, &mut s, BLUE);
    stroke(&gpu, &mut s, margin_row(), 400.0, 40.0);
    assert!(s.engine.brush_totals(&gpu).unwrap().pigment[YELLOW] > 0.0);
}

#[test]
fn wiping_the_brush_on_the_margin_leaves_its_paint_there() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 384, 256);
    load(&gpu, &mut s, BLUE);
    let held = s.engine.brush_totals(&gpu).unwrap().volume;
    stroke(&gpu, &mut s, margin_row(), 40.0, 400.0);
    assert!(s.engine.brush_totals(&gpu).unwrap().volume < held);
    assert!(margin_wet_volume(&gpu, &s) > 0.0);
    assert_eq!(painting_wet_volume(&gpu, &s), 0.0);
}

#[test]
fn one_stroke_from_the_painting_into_the_margin_is_one_undo_step() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 384, 256);
    load(&gpu, &mut s, BLUE);
    let inside = f64::from(MARGIN_PX) + 100.0;
    drag(&gpu, &mut s, &column(inside, inside, 20.0));
    assert!(margin_wet_volume(&gpu, &s) > 0.0 && painting_wet_volume(&gpu, &s) > 0.0);
    assert!(s.undo(&gpu));
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
    assert!(!s.undo(&gpu));
}
