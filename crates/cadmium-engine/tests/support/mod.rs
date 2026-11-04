use std::path::PathBuf;

use cadmium_engine::{
    canvas::{CanvasSpec, Primer, Weave},
    gpu::GpuContext,
    present::{PresentRequest, ViewTransform, readback_rgba8},
    session::PaintSession,
    stroke::StrokeSample,
};

const PREVIEW_WIDTH: u32 = 512;
const PREVIEW_HEIGHT: u32 = 256;

pub fn without_wgpu_errors<T>(gpu: &GpuContext, work: impl FnOnce() -> T) -> T {
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let result = work();
    let error = pollster::block_on(scope.pop());
    assert!(error.is_none(), "wgpu error: {error:?}");
    result
}

pub fn spec(width: u32, height: u32) -> CanvasSpec {
    CanvasSpec {
        width,
        height,
        primer: Primer::WhiteGesso,
        weave: Weave::Linen,
        pigment_slots: vec![
            "ultramarine_blue".into(),
            "cadmium_yellow".into(),
            "titanium_white".into(),
        ],
    }
}

pub fn session(gpu: &GpuContext) -> PaintSession {
    session_of_width(gpu, 512)
}

pub fn session_of_width(gpu: &GpuContext, width: u32) -> PaintSession {
    without_wgpu_errors(gpu, || PaintSession::new(gpu, spec(width, 256)).unwrap())
}

pub fn encode_frame(gpu: &GpuContext, s: &mut PaintSession) {
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(gpu, &mut e);
    gpu.queue.submit([e.finish()]);
}

pub fn frame(gpu: &GpuContext, s: &mut PaintSession) {
    without_wgpu_errors(gpu, || encode_frame(gpu, s));
}

pub fn drag(gpu: &GpuContext, s: &mut PaintSession, samples: &[StrokeSample]) {
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
    s.end_stroke();
    frame(gpu, s);
}

pub fn line(y: f64, from: f64, to: f64, pressure: f32, tilt_x_deg: f32) -> Vec<StrokeSample> {
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

pub fn stroke(gpu: &GpuContext, s: &mut PaintSession, y: f64, from: f64, to: f64) {
    drag(gpu, s, &line(y, from, to, 0.8, 20.0));
}

pub fn total_volume(gpu: &GpuContext, s: &PaintSession) -> f64 {
    s.canvas.wet_totals(gpu).unwrap().volume + s.engine.brush_totals(gpu).unwrap().volume
}

pub fn presented_rgba(gpu: &GpuContext, s: &PaintSession, width: u32, height: u32) -> Vec<u8> {
    let target = s.presenter.create_target(gpu, width, height);
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.presenter.encode_present(
        gpu,
        PresentRequest {
            canvas: &s.canvas,
            view: ViewTransform {
                origin_x: 0.0,
                origin_y: 0.0,
                zoom: 1.0,
            },
        },
        &target,
        &mut e,
    );
    gpu.queue.submit([e.finish()]);
    readback_rgba8(gpu, &target).unwrap()
}

pub fn write_png(name: &str, width: u32, height: u32, rgba: &[u8]) {
    let target_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .expect("cargo target tmpdir lives inside the target directory")
        .to_path_buf();
    let file = std::fs::File::create(target_dir.join(name)).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(rgba)
        .unwrap();
}

pub fn write_preview(gpu: &GpuContext, s: &PaintSession, name: &str) {
    let rgba = presented_rgba(gpu, s, PREVIEW_WIDTH, PREVIEW_HEIGHT);
    write_png(name, PREVIEW_WIDTH, PREVIEW_HEIGHT, &rgba);
}
