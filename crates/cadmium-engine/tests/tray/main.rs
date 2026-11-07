mod cord;
mod draw_up;
mod strokes;
mod surface;
mod tube;

use cadmium_engine::{
    canvas::{CanvasSpec, Primer, StateTexture, Weave},
    gpu::GpuContext,
    session::PaintSession,
    stroke::StrokeSample,
    surface::{Surface, TRAY_WIDTH},
    tube::Nozzle,
};

const FROZEN_FRAME_CLOCK_MS: f64 = 0.0;

fn spec() -> CanvasSpec {
    CanvasSpec {
        width: 512,
        height: 256,
        primer: Primer::WhiteGesso,
        weave: Weave::Linen,
        pigment_slots: vec!["ultramarine_blue".into(), "cadmium_yellow".into()],
    }
}

fn session(gpu: &GpuContext) -> PaintSession {
    PaintSession::new(gpu, spec()).unwrap()
}

fn frame(gpu: &GpuContext, s: &mut PaintSession) {
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(gpu, FROZEN_FRAME_CLOCK_MS, &mut e);
    s.encode_tray_frame(gpu, FROZEN_FRAME_CLOCK_MS, &mut e);
    gpu.queue.submit([e.finish()]);
}

fn sample(x: f64, y: f64, time_ms: u32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure: 0.8,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms,
    }
}

fn stroke_on(gpu: &GpuContext, s: &mut PaintSession, surface: Surface, y: f64) {
    s.begin_stroke_on(surface, sample(40.0, y, 0));
    for i in 1..=40 {
        s.add_samples(&[sample(40.0 + 10.0 * f64::from(i), y, 8 * i)]);
        if i % 4 == 0 {
            frame(gpu, s);
        }
    }
    s.end_stroke();
    frame(gpu, s);
}

fn load_slot(gpu: &GpuContext, s: &mut PaintSession, slot: usize) {
    s.brush.recipe = vec![(slot, 1.0)];
    s.reload_brush(gpu);
}

fn still_nozzle(slot: usize, seconds: f32) -> Nozzle {
    Nozzle {
        from: [300.5, 300.5],
        to: [300.5, 300.5],
        radius: 12.0,
        slot,
        pressure: 1.0,
        seconds,
        still_reach_px: 0.0,
    }
}

const FRAME_SECONDS: f32 = 1.0 / 60.0;
const CORD_RADIUS: f32 = 8.0;

fn tray_heights(gpu: &GpuContext, s: &PaintSession) -> impl Fn(usize, usize) -> f32 {
    let volume = s.tray.read_f32(gpu, StateTexture::WetVol).unwrap();
    move |x, y| volume[y * TRAY_WIDTH as usize + x]
}
