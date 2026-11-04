use oleo_engine::{
    canvas::*,
    engine::PaintEngine,
    gpu::GpuContext,
    pigments::Mixture,
    session::PaintSession,
    stroke::{StrokeSample, Tool},
};

const CANVAS_WIDTH: usize = 512;

fn without_wgpu_errors<T>(gpu: &GpuContext, work: impl FnOnce() -> T) -> T {
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let result = work();
    let error = pollster::block_on(scope.pop());
    assert!(error.is_none(), "wgpu error: {error:?}");
    result
}

fn session(gpu: &GpuContext) -> PaintSession {
    without_wgpu_errors(gpu, || {
        PaintSession::new(
            gpu,
            CanvasSpec {
                width: 512,
                height: 256,
                primer: Primer::WhiteGesso,
                weave: Weave::Linen,
                pigment_slots: vec![
                    "ultramarine_blue".into(),
                    "cadmium_yellow".into(),
                    "titanium_white".into(),
                ],
            },
        )
    })
}

fn frame(gpu: &GpuContext, s: &mut PaintSession) {
    without_wgpu_errors(gpu, || {
        let mut e = gpu.device.create_command_encoder(&Default::default());
        s.encode_frame(gpu, &mut e);
        gpu.queue.submit([e.finish()]);
    });
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

fn stroke(gpu: &GpuContext, s: &mut PaintSession, y: f64, from: f64, to: f64) {
    drag(gpu, s, &line(y, from, to, 0.8, 20.0));
}

fn dry(gpu: &GpuContext, s: &mut PaintSession) {
    without_wgpu_errors(gpu, || s.dry(gpu));
}

fn pick(gpu: &GpuContext, s: &mut PaintSession, x: f32, y: f32) -> Mixture {
    without_wgpu_errors(gpu, || s.pick_color(gpu, x, y))
}

fn shaded_display(gpu: &GpuContext, s: &mut PaintSession) -> Vec<f32> {
    s.canvas.dirty.mark_all();
    frame(gpu, s);
    s.canvas.read_f32(gpu, StateTexture::Display)
}

#[test]
fn rag_only_removes_paint() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let before = s.canvas.wet_totals(&gpu).volume;
    let brush_before = s.engine.brush_totals(&gpu).volume;
    let dry_before = s.canvas.read_f32(&gpu, StateTexture::DryRefl0);
    s.brush.tool = Tool::Rag;
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    assert!(s.canvas.wet_totals(&gpu).volume < before);
    assert!(
        (s.engine.brush_totals(&gpu).volume - brush_before).abs() < 1e-3 * brush_before.max(1.0)
    );
    assert_eq!(s.canvas.read_f32(&gpu, StateTexture::DryRefl0), dry_before);
}

#[test]
fn rag_keeps_the_wet_mixture_proportions() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let before = s.canvas.read_f32(&gpu, StateTexture::WetPig0);
    s.brush.tool = Tool::Rag;
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    assert_eq!(s.canvas.read_f32(&gpu, StateTexture::WetPig0), before);
}

#[test]
fn eyedropper_loads_the_brush_with_the_mixture_under_it() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let m = pick(&gpu, &mut s, 256.0, 128.0);
    assert!(m.fractions[0] > 0.5, "{:?}", m.fractions);
}

#[test]
fn eyedropper_reloads_the_tip_so_the_next_stroke_paints_the_picked_colour() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.recipe = vec![(1, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 64.0, 40.0, 470.0);
    s.brush.recipe = vec![(0, 1.0)];
    s.reload_brush(&gpu);
    pick(&gpu, &mut s, 256.0, 64.0);
    stroke(&gpu, &mut s, 192.0, 40.0, 470.0);
    let low = s.canvas.read_f32(&gpu, StateTexture::WetPig0);
    let texel = 4 * (192 * CANVAS_WIDTH + 256);
    assert!(low[texel + 1] > 0.5, "{:?}", &low[texel..texel + 4]);
    assert!(low[texel] < 0.05, "{:?}", &low[texel..texel + 4]);
}

#[test]
fn eyedropper_on_an_empty_brush_gives_it_a_full_body() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    s.clean_brush(&gpu);
    let m = pick(&gpu, &mut s, 256.0, 128.0);
    assert!(m.fractions[0] > 0.5, "{:?}", m.fractions);
    assert!(s.engine.brush_totals(&gpu).volume > 0.0);
}

#[test]
fn eyedropper_over_dried_paint_picks_the_top_dry_layer() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.recipe = vec![(1, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    dry(&gpu, &mut s);
    let m = pick(&gpu, &mut s, 256.0, 128.0);
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
    assert_eq!(s.canvas.wet_totals(&gpu).volume, 0.0);
}

#[test]
fn drying_moves_the_wet_thickness_into_the_dry_height() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let wet = s.canvas.read_f32(&gpu, StateTexture::WetVol);
    let height = s.canvas.read_f32(&gpu, StateTexture::DryHeight);
    dry(&gpu, &mut s);
    let dried = s.canvas.read_f32(&gpu, StateTexture::DryHeight);
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
    });
    s.canvas
        .write_f32(&gpu, StateTexture::WetVol, &vec![10.0; 1024 * 1024]);
    s.canvas.wet_tiles.mark_all();
    dry(&gpu, &mut s);
    assert_eq!(s.canvas.wet_totals(&gpu).volume, 0.0);
    let height = s.canvas.read_f32(&gpu, StateTexture::DryHeight);
    assert!(height.iter().all(|h| *h >= 10.0));
}

#[test]
fn clean_brush_empties_tip_and_reservoir() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.clean_brush(&gpu);
    assert_eq!(s.engine.brush_totals(&gpu).volume, 0.0);
}

#[test]
fn dabs_with_non_finite_geometry_are_skipped() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let dab = oleo_engine::stroke::Dab {
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
