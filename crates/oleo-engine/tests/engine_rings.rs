use std::path::PathBuf;

use oleo_engine::{
    canvas::*,
    gpu::GpuContext,
    present::{ViewTransform, readback_rgba8},
    session::PaintSession,
    stroke::StrokeSample,
};

const SIDE: u32 = 1024;
const ARC_CENTRE: [f64; 2] = [512.0, 560.0];
const ARC_RADIUS: f64 = 330.0;
const ARC_FROM_RAD: f64 = 3.6;
const ARC_TO_RAD: f64 = 6.0;
const ARC_SAMPLES: u32 = 220;
const SAMPLE_INTERVAL_MS: u32 = 4;
const FRAME_EVERY_SAMPLES: usize = 4;
const MAX_DIAMETER_PX: f32 = 110.0;
const TILT_DEG: f32 = 25.0;
const PRESSURE_WAVES: f64 = 9.0;
const INTERIOR_HALF_WIDTH_PX: f64 = 19.0;
const END_MARGIN_RAD: f64 = 0.35;
const ALONG_REACH_PX: i32 = 6;
const MAX_RELATIVE_HEIGHT_RIPPLE: f64 = 0.02;
const MAX_LUMINANCE_RIPPLE: f64 = 0.005;
const CONSERVATION_TOLERANCE: f64 = 1e-6;

struct Ripple {
    height_um: f64,
    mean_height_um: f64,
    luminance: f64,
}

fn session(gpu: &GpuContext) -> PaintSession {
    let mut s = PaintSession::new(
        gpu,
        CanvasSpec {
            width: SIDE,
            height: SIDE,
            primer: Primer::WhiteGesso,
            weave: Weave::Smooth,
            pigment_slots: vec!["ultramarine_blue".into(), "titanium_white".into()],
        },
    );
    s.brush.geometry.max_diameter_px = MAX_DIAMETER_PX;
    s.brush.recipe = vec![(0, 0.6), (1, 0.4)];
    s.reload_brush(gpu);
    s
}

fn frame(gpu: &GpuContext, s: &mut PaintSession) {
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(gpu, &mut e);
    gpu.queue.submit([e.finish()]);
}

fn arc_samples() -> Vec<StrokeSample> {
    (0..=ARC_SAMPLES)
        .map(|i| {
            let t = f64::from(i) / f64::from(ARC_SAMPLES);
            let angle = ARC_FROM_RAD + (ARC_TO_RAD - ARC_FROM_RAD) * t;
            StrokeSample {
                x: ARC_CENTRE[0] + ARC_RADIUS * angle.cos(),
                y: ARC_CENTRE[1] + ARC_RADIUS * angle.sin(),
                pressure: (0.8 + 0.2 * (t * PRESSURE_WAVES).sin()) as f32,
                tilt_x_deg: TILT_DEG,
                tilt_y_deg: 0.0,
                time_ms: i * SAMPLE_INTERVAL_MS,
            }
        })
        .collect()
}

fn paint_arc(gpu: &GpuContext, s: &mut PaintSession) {
    for (i, sample) in arc_samples().into_iter().enumerate() {
        if i == 0 {
            s.begin_stroke(gpu, sample)
        } else {
            s.add_samples(&[sample])
        }
        if i % FRAME_EVERY_SAMPLES == 0 {
            frame(gpu, s);
        }
    }
    s.end_stroke(gpu);
    frame(gpu, s);
}

fn colour_view(gpu: &GpuContext, s: &PaintSession) -> Vec<u8> {
    let target = s.presenter.create_target(gpu, SIDE, SIDE);
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
    readback_rgba8(gpu, &target)
}

fn srgb_to_linear(channel: u8) -> f32 {
    let v = f32::from(channel) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(rgba: &[u8]) -> Vec<f32> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .map(|p| {
            0.2126 * srgb_to_linear(p[0])
                + 0.7152 * srgb_to_linear(p[1])
                + 0.0722 * srgb_to_linear(p[2])
        })
        .collect()
}

fn bilinear(field: &[f32], x: f64, y: f64) -> f64 {
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let fx = x - x0 as f64;
    let fy = y - y0 as f64;
    let at = |xx: usize, yy: usize| f64::from(field[yy * SIDE as usize + xx]);
    (1.0 - fy) * ((1.0 - fx) * at(x0, y0) + fx * at(x0 + 1, y0))
        + fy * ((1.0 - fx) * at(x0, y0 + 1) + fx * at(x0 + 1, y0 + 1))
}

fn along_stroke_high_pass(field: &[f32], [x, y]: [f64; 2]) -> f64 {
    let radial = [x - ARC_CENTRE[0], y - ARC_CENTRE[1]];
    let length = radial[0].hypot(radial[1]);
    let tangent = [-radial[1] / length, radial[0] / length];
    let neighbourhood = (-ALONG_REACH_PX..=ALONG_REACH_PX)
        .map(|step| {
            let step = f64::from(step);
            bilinear(field, x + step * tangent[0], y + step * tangent[1])
        })
        .sum::<f64>()
        / f64::from(2 * ALONG_REACH_PX + 1);
    (bilinear(field, x, y) - neighbourhood).abs()
}

fn stroke_interior() -> Vec<[f64; 2]> {
    (0..SIDE)
        .flat_map(|y| (0..SIDE).map(move |x| [f64::from(x), f64::from(y)]))
        .filter(|[x, y]| {
            let dx = x - ARC_CENTRE[0];
            let dy = y - ARC_CENTRE[1];
            let angle = dy.atan2(dx).rem_euclid(std::f64::consts::TAU);
            let along_body =
                angle > ARC_FROM_RAD + END_MARGIN_RAD && angle < ARC_TO_RAD - END_MARGIN_RAD;
            along_body && (dx.hypot(dy) - ARC_RADIUS).abs() < INTERIOR_HALF_WIDTH_PX
        })
        .collect()
}

fn ripple(height: &[f32], lum: &[f32]) -> Ripple {
    let interior = stroke_interior();
    let count = interior.len() as f64;
    let mean_high_pass = |field: &[f32]| {
        interior
            .iter()
            .map(|pixel| along_stroke_high_pass(field, *pixel))
            .sum::<f64>()
            / count
    };
    Ripple {
        height_um: mean_high_pass(height),
        mean_height_um: interior
            .iter()
            .map(|[x, y]| f64::from(height[*y as usize * SIDE as usize + *x as usize]))
            .sum::<f64>()
            / count,
        luminance: mean_high_pass(lum),
    }
}

fn write_png(name: &str, rgba: &[u8]) {
    let target_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .expect("cargo target tmpdir lives inside the target directory")
        .to_path_buf();
    let file = std::fs::File::create(target_dir.join(name)).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), SIDE, SIDE);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(rgba)
        .unwrap();
}

fn height_view(height: &[f32]) -> Vec<u8> {
    let top = height.iter().copied().fold(f32::MIN_POSITIVE, f32::max);
    height
        .iter()
        .flat_map(|h| {
            let grey = (h / top * 255.0).round() as u8;
            [grey, grey, grey, u8::MAX]
        })
        .collect()
}

fn total_volume(gpu: &GpuContext, s: &PaintSession) -> f64 {
    s.canvas.wet_totals(gpu).volume + s.engine.brush_totals(gpu).volume
}

#[test]
fn a_thick_stroke_has_no_dab_spaced_ridges_inside() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let before = total_volume(&gpu, &s);
    paint_arc(&gpu, &mut s);
    let drift = (total_volume(&gpu, &s) - before) / before;
    let height = s.canvas.read_f32(&gpu, StateTexture::WetVol);
    let rgba = colour_view(&gpu, &s);
    write_png("engine-rings.png", &rgba);
    write_png("engine-rings-height.png", &height_view(&height));
    let found = ripple(&height, &luminance(&rgba));
    let relative = found.height_um / found.mean_height_um;
    eprintln!(
        "along-stroke ripple: height {:.3} um of {:.1} um ({relative:.4}), luminance {:.5}, volume drift {drift:e}",
        found.height_um, found.mean_height_um, found.luminance
    );
    assert!(relative < MAX_RELATIVE_HEIGHT_RIPPLE, "{relative}");
    assert!(
        found.luminance < MAX_LUMINANCE_RIPPLE,
        "{}",
        found.luminance
    );
    assert!(drift.abs() < CONSERVATION_TOLERANCE, "{drift:e}");
}
