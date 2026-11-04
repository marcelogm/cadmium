use std::path::PathBuf;

use oleo_engine::{
    canvas::*,
    engine::PaintEngine,
    gpu::GpuContext,
    present::{ViewTransform, readback_rgba8},
    session::PaintSession,
    stroke::{StrokeSample, Tool},
};

const PREVIEW_WIDTH: u32 = 512;
const PREVIEW_HEIGHT: u32 = 256;
const CANVAS_WIDTH: usize = 512;

fn session(gpu: &GpuContext) -> PaintSession {
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let session = PaintSession::new(
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
    let b = s.engine.brush_totals(gpu);
    let mut t = c;
    t.volume += b.volume;
    for i in 0..8 {
        t.pigment[i] += b.pigment[i];
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
    assert!(volume.abs() < 1e-4);
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
    assert!(volume.abs() < 1e-4);
    assert!(pigment.iter().all(|drift| drift.abs() < 5e-3));
}

#[test]
fn brush_stroke_builds_ridges_at_its_edges() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.engine.params_mut().set("squeeze", 1.5);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let vol = s.canvas.read_f32(&gpu, StateTexture::WetVol);
    let column: Vec<f32> = (100..156).map(|y| vol[y * CANVAS_WIDTH + 256]).collect();
    let center = column[28];
    let edge_max = column[..20]
        .iter()
        .chain(column[36..].iter())
        .cloned()
        .fold(0.0, f32::max);
    assert!(edge_max > center, "edges {edge_max} center {center}");
}

#[test]
fn upright_knife_scrapes_wet_paint_down_to_the_residual() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let before = s.canvas.wet_totals(&gpu).volume;
    s.brush.tool = Tool::Knife;
    s.clean_brush(&gpu);
    for i in 0..=60 {
        let x = 40.0 + 430.0 * i as f64 / 60.0;
        let sample = StrokeSample {
            x,
            y: 128.0,
            pressure: 1.0,
            tilt_x_deg: 0.0,
            tilt_y_deg: 0.0,
            time_ms: i * 4,
        };
        if i == 0 {
            s.begin_stroke(&gpu, sample)
        } else {
            s.add_samples(&[sample])
        }
    }
    s.end_stroke(&gpu);
    frame(&gpu, &mut s);
    let after = s.canvas.wet_totals(&gpu).volume;
    eprintln!("knife scrape {before} -> {after}");
    assert!(after < before * 0.6);
}

#[test]
fn knife_previews_show_flattened_paint() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 70.0, 40.0, 470.0);
    stroke(&gpu, &mut s, 90.0, 470.0, 40.0);
    s.brush.recipe = vec![(1, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 170.0, 40.0, 470.0);
    stroke(&gpu, &mut s, 190.0, 470.0, 40.0);
    s.brush.tool = Tool::Knife;
    s.clean_brush(&gpu);
    drag(&gpu, &mut s, &line(80.0, 60.0, 300.0, 1.0, 60.0));
    drag(&gpu, &mut s, &line(180.0, 200.0, 450.0, 1.0, 0.0));
    drag(&gpu, &mut s, &column(380.0, 30.0, 230.0, 1.0, 60.0));
    write_preview(&gpu, &s, "engine-knife.png");
    assert!(s.canvas.wet_totals(&gpu).volume > 0.0);
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
