use std::ops::Range;

use cadmium_engine::{
    canvas::{PaintTotals, StateTexture},
    gpu::GpuContext,
    present::{PresentRequest, ViewTransform, readback_rgba8},
    session::PaintSession,
    stroke::{StrokeSample, Tool},
    surface::{Surface, TRAY_WIDTH},
    tube::Nozzle,
};

use crate::{CORD_RADIUS, FRAME_SECONDS, frame, session};

pub(super) const BLADE_DIAMETER_PX: f32 = 80.0;
pub(super) const CORD_A_Y: f32 = 300.0;
pub(super) const CORD_B_Y: f32 = 318.0;
pub(super) const CORD_MIDDLE_Y: f64 = 309.0;
pub(super) const CORD_FROM_X: f32 = 220.0;
pub(super) const CORD_TO_X: f32 = 480.0;
pub(super) const CORD_FRAMES: usize = 90;
pub(super) const PASS_X: f64 = 350.0;
pub(super) const PASS_TOP_Y: f64 = 250.0;
pub(super) const PASS_BOTTOM_Y: f64 = 370.0;
pub(super) const PASS_SAMPLES: u32 = 40;
pub(super) const LAID_TILT_DEG: f32 = 60.0;
pub(super) const MIXING_PRESSURE: f32 = 0.7;
pub(super) const REGION_X: Range<usize> = 320..381;
pub(super) const REGION_Y: Range<usize> = 230..391;
pub(super) const PAINTED_UM: f32 = 20.0;
pub(super) const PREVIEW_DIR: Option<&str> = option_env!("CADMIUM_MIXING_PREVIEWS");

pub(super) fn mixing_session(gpu: &GpuContext) -> PaintSession {
    let mut s = session(gpu);
    s.brush.tool = Tool::MixingKnife;
    s.brush.geometry.max_diameter_px = BLADE_DIAMETER_PX;
    s.resize_tip();
    s
}

pub(super) fn cord(gpu: &GpuContext, s: &mut PaintSession, slot: usize, y: f32) {
    let at = |frame: usize| {
        let t = frame as f32 / CORD_FRAMES as f32;
        [CORD_FROM_X + t * (CORD_TO_X - CORD_FROM_X), y]
    };
    s.begin_hold(gpu, Surface::Tray);
    for frame in 0..CORD_FRAMES {
        s.squeeze(
            gpu,
            Nozzle {
                from: at(frame),
                to: at(frame + 1),
                radius: CORD_RADIUS,
                slot,
                pressure: 1.0,
                seconds: FRAME_SECONDS,
                still_reach_px: 0.0,
            },
        );
    }
    s.end_hold();
}

pub(super) fn two_cords(gpu: &GpuContext, s: &mut PaintSession) {
    cord(gpu, s, 0, CORD_A_Y);
    cord(gpu, s, 1, CORD_B_Y);
}

pub(super) struct Pass {
    pub(super) from: [f64; 2],
    pub(super) to: [f64; 2],
    pub(super) tilt_x_deg: f32,
    pub(super) pressure: f32,
}

pub(super) fn knife_pass(gpu: &GpuContext, s: &mut PaintSession, pass: Pass) {
    let at = |i: u32| {
        let t = f64::from(i) / f64::from(PASS_SAMPLES);
        StrokeSample {
            x: pass.from[0] + t * (pass.to[0] - pass.from[0]),
            y: pass.from[1] + t * (pass.to[1] - pass.from[1]),
            pressure: pass.pressure,
            tilt_x_deg: pass.tilt_x_deg,
            tilt_y_deg: 0.0,
            time_ms: 8 * i,
        }
    };
    s.begin_stroke_on(Surface::Tray, at(0));
    for i in 1..=PASS_SAMPLES {
        s.add_samples(&[at(i)]);
        if i % 4 == 0 {
            frame(gpu, s);
        }
    }
    s.end_stroke();
    frame(gpu, s);
}

/// The `index`th pass of a mixing session: laid flat across both cords,
/// alternately down and back up.
pub(super) fn mixing_pass(index: usize) -> Pass {
    let (from, to) = if index.is_multiple_of(2) {
        (PASS_TOP_Y, PASS_BOTTOM_Y)
    } else {
        (PASS_BOTTOM_Y, PASS_TOP_Y)
    };
    Pass {
        from: [PASS_X, from],
        to: [PASS_X, to],
        tilt_x_deg: LAID_TILT_DEG,
        pressure: MIXING_PRESSURE,
    }
}

pub(super) fn along_x(y: f64, from: f64, to: f64, tilt_x_deg: f32) -> Pass {
    Pass {
        from: [from, y],
        to: [to, y],
        tilt_x_deg,
        pressure: MIXING_PRESSURE,
    }
}

pub(super) fn face_totals(gpu: &GpuContext, s: &PaintSession) -> PaintTotals {
    s.engine.face_totals(gpu).unwrap()
}

pub(super) fn tray_and_tools(gpu: &GpuContext, s: &PaintSession) -> PaintTotals {
    let mut total = s.tray.wet_totals(gpu).unwrap();
    for tool in [
        s.engine.brush_totals(gpu).unwrap(),
        s.engine.knife_totals(gpu).unwrap(),
        face_totals(gpu, s),
    ] {
        total.volume += tool.volume;
        for slot in 0..2 {
            total.pigment[slot] += tool.pigment[slot];
        }
    }
    total
}

pub(super) struct TrayPaint {
    volume: Vec<f32>,
    pig0: Vec<f32>,
}

impl TrayPaint {
    pub(super) fn read(gpu: &GpuContext, s: &PaintSession) -> TrayPaint {
        TrayPaint {
            volume: s.tray.read_f32(gpu, StateTexture::WetVol).unwrap(),
            pig0: s.tray.read_f32(gpu, StateTexture::WetPig0).unwrap(),
        }
    }

    pub(super) fn volume_at(&self, x: usize, y: usize) -> f32 {
        self.volume[y * TRAY_WIDTH as usize + x]
    }

    /// The share of the first pigment among the two, or `None` where bare.
    pub(super) fn share_of_a(&self, x: usize, y: usize) -> Option<f64> {
        let texel = (y * TRAY_WIDTH as usize + x) * 4;
        let a = f64::from(self.pig0[texel]);
        let b = f64::from(self.pig0[texel + 1]);
        (self.volume_at(x, y) > PAINTED_UM && a + b > 0.0).then(|| a / (a + b))
    }

    pub(super) fn shares_in(&self, xs: Range<usize>, ys: Range<usize>) -> Vec<f64> {
        ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
            .filter_map(|(x, y)| self.share_of_a(x, y))
            .collect()
    }

    pub(super) fn volume_in(&self, xs: Range<usize>, ys: Range<usize>) -> f64 {
        ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
            .map(|(x, y)| f64::from(self.volume_at(x, y)))
            .sum()
    }
}

pub(super) fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/// The variance of the first pigment's share over the paint in a region, each
/// pixel weighted by its volume: how much of the paint there is still unmixed,
/// whatever thin smears a pass leaves around it.
pub(super) fn weighted_variance(paint: &TrayPaint, xs: Range<usize>, ys: Range<usize>) -> f64 {
    let shares: Vec<(f64, f64)> = ys
        .flat_map(|y| xs.clone().map(move |x| (x, y)))
        .filter_map(|(x, y)| Some((paint.share_of_a(x, y)?, f64::from(paint.volume_at(x, y)))))
        .collect();
    let volume: f64 = shares.iter().map(|(_, v)| v).sum();
    let centre = shares.iter().map(|(a, v)| a * v).sum::<f64>() / volume;
    shares
        .iter()
        .map(|(a, v)| (a - centre).powi(2) * v)
        .sum::<f64>()
        / volume
}

pub(super) fn mixed_region_variance(gpu: &GpuContext, s: &PaintSession) -> f64 {
    weighted_variance(&TrayPaint::read(gpu, s), REGION_X, REGION_Y)
}

pub(super) fn write_tray_preview(gpu: &GpuContext, s: &PaintSession, name: &str) {
    let Some(dir) = PREVIEW_DIR else {
        return;
    };
    let (width, height) = (400, 420);
    let target = s.presenter.create_target(gpu, width, height);
    let mut e = gpu.device.create_command_encoder(&Default::default());
    let view = ViewTransform {
        origin_x: 150.0,
        origin_y: 220.0,
        zoom: 1.0,
        rotation: 0.0,
    };
    s.presenter.encode_present(
        gpu,
        PresentRequest::of_canvas(&s.tray, view),
        &target,
        &mut e,
    );
    gpu.queue.submit([e.finish()]);
    let rgba = readback_rgba8(gpu, &target).unwrap();
    let file = std::fs::File::create(std::path::Path::new(dir).join(name)).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&rgba)
        .unwrap();
}
