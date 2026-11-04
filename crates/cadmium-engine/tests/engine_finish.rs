pub mod support;

use cadmium_engine::{
    canvas::*, engine::PaintEngine, gpu::GpuContext, pigments::Mixture, session::PaintSession,
    stroke::Tool,
};
use support::{frame, line, session, stroke, without_wgpu_errors};

const CANVAS_WIDTH: usize = 512;

fn dry(gpu: &GpuContext, s: &mut PaintSession) {
    without_wgpu_errors(gpu, || s.dry(gpu));
}

fn pick(gpu: &GpuContext, s: &mut PaintSession, x: f32, y: f32) -> Option<Mixture> {
    without_wgpu_errors(gpu, || s.pick_colour(gpu, x, y).unwrap())
}

fn shaded_display(gpu: &GpuContext, s: &mut PaintSession) -> Vec<f32> {
    s.canvas.dirty.mark_all();
    frame(gpu, s);
    s.canvas.read_f32(gpu, StateTexture::Display).unwrap()
}

#[test]
fn rag_only_removes_paint() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let before = s.canvas.wet_totals(&gpu).unwrap().volume;
    let brush_before = s.engine.brush_totals(&gpu).unwrap().volume;
    let dry_before = s.canvas.read_f32(&gpu, StateTexture::DryRefl0).unwrap();
    s.brush.tool = Tool::Rag;
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    assert!(s.canvas.wet_totals(&gpu).unwrap().volume < before);
    assert!(
        (s.engine.brush_totals(&gpu).unwrap().volume - brush_before).abs()
            < 1e-3 * brush_before.max(1.0)
    );
    assert_eq!(
        s.canvas.read_f32(&gpu, StateTexture::DryRefl0).unwrap(),
        dry_before
    );
}

#[test]
fn rag_keeps_the_wet_mixture_proportions() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let before = s.canvas.read_f32(&gpu, StateTexture::WetPig0).unwrap();
    s.brush.tool = Tool::Rag;
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    assert_eq!(
        s.canvas.read_f32(&gpu, StateTexture::WetPig0).unwrap(),
        before
    );
}

#[test]
fn eyedropper_finds_the_mixture_under_it() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let m = pick(&gpu, &mut s, 256.0, 128.0).unwrap();
    assert!(m.fractions[0] > 0.5, "{:?}", m.fractions);
}

#[test]
fn a_brush_loaded_with_the_picked_mixture_paints_the_picked_colour() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.recipe = vec![(1, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 64.0, 40.0, 470.0);
    s.brush.recipe = vec![(0, 1.0)];
    s.reload_brush(&gpu);
    let picked = pick(&gpu, &mut s, 256.0, 64.0).unwrap();
    s.brush.recipe = picked.fractions.iter().copied().enumerate().collect();
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 192.0, 40.0, 470.0);
    let low = s.canvas.read_f32(&gpu, StateTexture::WetPig0).unwrap();
    let texel = 4 * (192 * CANVAS_WIDTH + 256);
    assert!(low[texel + 1] > 0.5, "{:?}", &low[texel..texel + 4]);
    assert!(low[texel] < 0.05, "{:?}", &low[texel..texel + 4]);
}

#[test]
fn eyedropper_over_paint_finds_it_without_touching_the_brush() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    s.clean_brush(&gpu);
    let m = pick(&gpu, &mut s, 256.0, 128.0).unwrap();
    assert!(m.fractions[0] > 0.5, "{:?}", m.fractions);
    assert_eq!(s.engine.brush_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn eyedropper_over_dried_paint_picks_the_top_dry_layer() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.recipe = vec![(1, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    dry(&gpu, &mut s);
    let m = pick(&gpu, &mut s, 256.0, 128.0).unwrap();
    assert!(m.fractions[1] > 0.5, "{:?}", m.fractions);
}

#[test]
fn drying_keeps_the_displayed_colour_and_empties_the_wet_layer() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.engine.params_mut().set("wet_gloss", 0.0);
    s.engine.params_mut().set("dry_roughness", 1.0);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let before = shaded_display(&gpu, &mut s);
    dry(&gpu, &mut s);
    let after = shaded_display(&gpu, &mut s);
    let worst = before
        .iter()
        .zip(&after)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(worst < 0.01, "worst {worst}");
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn drying_moves_the_wet_thickness_into_the_dry_height() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let wet = s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
    let height = s.canvas.read_f32(&gpu, StateTexture::DryHeight).unwrap();
    dry(&gpu, &mut s);
    let dried = s.canvas.read_f32(&gpu, StateTexture::DryHeight).unwrap();
    let worst = wet
        .iter()
        .zip(&height)
        .zip(&dried)
        .map(|((w, h), d)| (h + w - d).abs())
        .fold(0.0, f32::max);
    assert!(worst < 1e-3, "worst {worst}");
}

#[test]
fn drying_clears_the_wet_tiles_and_marks_them_for_shading() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let wet = s.canvas.wet_tiles.tiles();
    assert!(!wet.is_empty());
    assert!(s.canvas.dirty.is_empty());
    dry(&gpu, &mut s);
    assert!(s.canvas.wet_tiles.is_empty());
    assert_eq!(s.canvas.dirty.tiles(), wet);
}

#[test]
fn drying_a_large_canvas_in_chunks_dries_every_wet_tile() {
    let gpu = GpuContext::new().unwrap();
    let mut s = without_wgpu_errors(&gpu, || {
        PaintSession::new(
            &gpu,
            CanvasSpec {
                width: 1024,
                height: 1024,
                primer: Primer::WhiteGesso,
                weave: Weave::Smooth,
                pigment_slots: vec!["ultramarine_blue".into()],
            },
        )
        .unwrap()
    });
    s.canvas
        .write_f32(&gpu, StateTexture::WetVol, &vec![10.0; 1024 * 1024]);
    s.canvas.wet_tiles.mark_all();
    dry(&gpu, &mut s);
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
    let height = s.canvas.read_f32(&gpu, StateTexture::DryHeight).unwrap();
    assert!(height.iter().all(|h| *h >= 10.0));
}

#[test]
fn clean_brush_empties_tip_and_reservoir() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.clean_brush(&gpu);
    assert_eq!(s.engine.brush_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn dabs_with_non_finite_geometry_are_skipped() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let dab = cadmium_engine::stroke::Dab {
        x: 100.0,
        y: 100.0,
        radius: f32::NAN,
        pressure: 1.0,
        altitude: 1.0,
        azimuth: 0.0,
        direction: 0.0,
        speed: 0.0,
    };
    let mut broken = [dab; 3];
    broken[1].x = f32::INFINITY;
    broken[1].radius = 10.0;
    broken[2].radius = 10.0;
    broken[2].pressure = f32::NAN;
    without_wgpu_errors(&gpu, || {
        let mut e = gpu.device.create_command_encoder(&Default::default());
        let brush = s.brush.clone();
        s.engine
            .encode_dabs(&gpu, &mut s.canvas, &brush, &broken, &mut e);
        gpu.queue.submit([e.finish()]);
    });
    assert!(s.canvas.wet_tiles.is_empty());
}

fn assert_brush_untouched_by_pick(x: f32, y: f32) {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let before = s.engine.brush_totals(&gpu).unwrap();
    let mixture_before = s.engine.reservoir_mixture(&gpu).unwrap();
    assert!(pick(&gpu, &mut s, x, y).is_none());
    assert_eq!(s.engine.brush_totals(&gpu).unwrap(), before);
    assert_eq!(
        s.engine.reservoir_mixture(&gpu).unwrap().fractions,
        mixture_before.fractions
    );
}

#[test]
fn eyedropper_off_the_canvas_leaves_the_brush_untouched() {
    assert_brush_untouched_by_pick(-200.0, -200.0);
}

#[test]
fn a_pick_off_the_canvas_finds_nothing_even_right_after_a_pick_that_found_paint() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    assert!(pick(&gpu, &mut s, 256.0, 128.0).is_some());
    assert!(pick(&gpu, &mut s, -200.0, -200.0).is_none());
}

#[test]
fn eyedropper_over_bare_primer_leaves_the_brush_untouched() {
    assert_brush_untouched_by_pick(256.0, 20.0);
}

#[test]
fn eyedropper_at_the_edge_of_dried_paint_ignores_the_bare_primer_around_it() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.canvas.write_f32(&gpu, StateTexture::DryTopPig0, &{
        let mut low = vec![0.0; 4 * 512 * 256];
        let texel = 4 * (128 * CANVAS_WIDTH + 256);
        low[texel + 1] = 1.0;
        low
    });
    let m = pick(&gpu, &mut s, 256.5, 128.5).unwrap();
    assert!((m.fractions[1] - 1.0).abs() < 1e-3, "{:?}", m.fractions);
}

#[test]
fn a_queued_stroke_tail_is_painted_with_the_tool_it_was_drawn_with() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let tail = line(64.0, 40.0, 470.0, 0.8, 20.0);
    without_wgpu_errors(&gpu, || {
        s.begin_stroke(&gpu, tail[0]);
        s.add_samples(&tail[1..]);
        s.end_stroke();
    });
    s.brush.tool = Tool::Rag;
    let elsewhere = line(200.0, 40.0, 470.0, 0.8, 20.0);
    without_wgpu_errors(&gpu, || s.begin_stroke(&gpu, elsewhere[0]));
    let volume = s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
    assert!(volume[64 * CANVAS_WIDTH + 256] > 0.0);
}
