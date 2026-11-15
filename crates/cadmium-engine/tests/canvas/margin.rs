use cadmium_engine::{
    canvas::{CanvasSpec, MARGIN_PX, StateTexture, TILE_SIZE, TileCoord},
    gpu::GpuContext,
    session::{ClearOutcome, PaintSession},
    stroke::{StrokeSample, Tool},
};

use crate::support;
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

fn state_snapshot(gpu: &GpuContext, s: &PaintSession) -> Vec<Vec<u8>> {
    StateTexture::PAINT_STATE
        .iter()
        .map(|which| s.canvas.read_bytes(gpu, *which).unwrap())
        .collect()
}

fn texels_where(s: &PaintSession, snapshot: &[Vec<u8>], in_margin: bool) -> Vec<Vec<u8>> {
    let spec = &s.canvas.spec;
    StateTexture::PAINT_STATE
        .iter()
        .zip(snapshot)
        .map(|(which, bytes)| {
            bytes
                .chunks_exact(which.bytes_per_texel() as usize)
                .enumerate()
                .filter(|(index, _)| {
                    let index = *index as u32;
                    spec.is_in_margin(index % spec.width, index / spec.width) == in_margin
                })
                .flat_map(|(_, texel)| texel.iter().copied())
                .collect()
        })
        .collect()
}

fn margin_texels(s: &PaintSession, snapshot: &[Vec<u8>]) -> Vec<Vec<u8>> {
    texels_where(s, snapshot, true)
}

fn painting_texels(s: &PaintSession, snapshot: &[Vec<u8>]) -> Vec<Vec<u8>> {
    texels_where(s, snapshot, false)
}

fn cut_by_the_edge() -> TileCoord {
    TileCoord {
        x: (MARGIN_PX + 299) / TILE_SIZE,
        y: (MARGIN_PX + 100) / TILE_SIZE,
    }
}

fn paint_out_across_the_right_edge(gpu: &GpuContext, s: &mut PaintSession) -> Vec<Vec<u8>> {
    let margin = f64::from(MARGIN_PX);
    load(gpu, s, BLUE);
    stroke(gpu, s, margin + 100.0, margin + 200.0, margin + 340.0);
    assert!(margin_wet_volume(gpu, s) > 0.0 && painting_wet_volume(gpu, s) > 0.0);
    state_snapshot(gpu, s)
}

#[test]
fn clearing_the_margin_resets_it_and_leaves_every_texel_of_the_painting_alone() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 300, 200);
    let blank = state_snapshot(&gpu, &s);
    let cut_by_the_edge = cut_by_the_edge();
    let painted = paint_out_across_the_right_edge(&gpu, &mut s);
    let outcome = without_wgpu_errors(&gpu, || s.clear_margin(&gpu));
    assert_eq!(outcome, ClearOutcome::Cleared { undoable: true });
    let cleared = state_snapshot(&gpu, &s);
    assert_eq!(margin_texels(&s, &cleared), margin_texels(&s, &blank));
    assert_eq!(painting_texels(&s, &cleared), painting_texels(&s, &painted));
    assert!(s.canvas.wet_tiles.contains(cut_by_the_edge));
    assert!(s.undo(&gpu));
    assert_eq!(state_snapshot(&gpu, &s), painted);
    assert!(s.redo(&gpu));
    assert_eq!(state_snapshot(&gpu, &s), cleared);
    assert!(s.canvas.wet_tiles.contains(cut_by_the_edge));
}

#[test]
fn clearing_a_margin_with_only_dry_paint_still_clears_it() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 300, 200);
    let blank = state_snapshot(&gpu, &s);
    load(&gpu, &mut s, BLUE);
    stroke(&gpu, &mut s, margin_row(), 40.0, 400.0);
    s.dry(&gpu);
    assert!(matches!(s.clear_margin(&gpu), ClearOutcome::Cleared { .. }));
    assert_eq!(
        margin_texels(&s, &state_snapshot(&gpu, &s)),
        margin_texels(&s, &blank)
    );
}

#[test]
fn clearing_the_margin_rubs_out_a_pencil_line_there_and_keeps_the_one_on_the_painting() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 300, 200);
    let margin = f64::from(MARGIN_PX);
    s.brush.tool = Tool::Pencil;
    drag(&gpu, &mut s, &line(margin_row(), 40.0, 400.0, 0.9, 0.0));
    drag(
        &gpu,
        &mut s,
        &line(margin + 100.0, margin + 20.0, margin + 280.0, 0.9, 0.0),
    );
    assert!(total_where(&gpu, &s, StateTexture::Graphite, true) > 0.0);
    assert!(total_where(&gpu, &s, StateTexture::Graphite, false) > 0.0);
    assert!(s.canvas.wet_tiles.is_empty());
    let sketched = state_snapshot(&gpu, &s);
    assert_eq!(
        s.clear_margin(&gpu),
        ClearOutcome::Cleared { undoable: true }
    );
    assert_eq!(total_where(&gpu, &s, StateTexture::Graphite, true), 0.0);
    let cleared = state_snapshot(&gpu, &s);
    assert_eq!(
        painting_texels(&s, &cleared),
        painting_texels(&s, &sketched)
    );
}

#[test]
fn there_is_nothing_to_clear_on_a_clean_margin_or_without_one() {
    let gpu = GpuContext::new().unwrap();
    let mut with_margin = painting_session(&gpu, 300, 200);
    assert_eq!(with_margin.clear_margin(&gpu), ClearOutcome::NothingToClear);
    let mut without = support::session(&gpu);
    stroke(&gpu, &mut without, 128.0, 40.0, 470.0);
    assert_eq!(without.clear_margin(&gpu), ClearOutcome::NothingToClear);
}

#[test]
fn redoing_a_margin_clear_leaves_the_margin_dry_and_the_edge_of_the_painting_wet() {
    let gpu = GpuContext::new().unwrap();
    let mut s = painting_session(&gpu, 300, 200);
    let wholly_in_margin = TileCoord { x: 3, y: 1 };
    load(&gpu, &mut s, BLUE);
    stroke(&gpu, &mut s, margin_row(), 40.0, 400.0);
    paint_out_across_the_right_edge(&gpu, &mut s);
    assert!(s.canvas.wet_tiles.contains(wholly_in_margin));
    s.clear_margin(&gpu);
    assert!(s.undo(&gpu));
    assert!(s.canvas.wet_tiles.contains(wholly_in_margin));
    assert!(s.redo(&gpu));
    assert!(!s.canvas.wet_tiles.contains(wholly_in_margin));
    assert!(s.canvas.wet_tiles.contains(cut_by_the_edge()));
}
