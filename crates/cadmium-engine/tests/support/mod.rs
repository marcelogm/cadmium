use cadmium_engine::{
    canvas::{CanvasSpec, Primer, Weave},
    gpu::GpuContext,
    session::PaintSession,
    stroke::StrokeSample,
};

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
