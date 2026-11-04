pub mod support;

use cadmium_engine::{canvas::*, gpu::GpuContext, session::PaintSession, stroke::StrokeSample};
use support::{encode_frame, presented_rgba, total_volume};

const WIDTH: u32 = 2048;
const HEIGHT: u32 = 256;
const ROW: f64 = 128.0;
const FROM_X: f64 = 60.0;
const TO_X: f64 = 1990.0;
const SAMPLE_SPACING_PX: f64 = 3.6;
const SAMPLE_INTERVAL_MS: u32 = 4;
const FRAME_EVERY_SAMPLES: usize = 4;
const MAX_DIAMETER_PX: f32 = 70.0;
const TILT_DEG: f32 = 25.0;
const PRESSURE: f32 = 0.8;
const BAND_HALF_WIDTH_PX: f64 = 12.0;
const WINDOW_PX: usize = 48;
const ALONG_REACH_PX: i32 = 6;
const FIRST_WINDOW_X: usize = 160;
const LOAD: f32 = 0.2;
const MIN_PAINT_DEPTH: f64 = 0.05;
const FADED_SHARE: f64 = 0.25;
const MAX_RELATIVE_LUMINANCE_RIPPLE: f64 = 0.06;
const CONSERVATION_TOLERANCE: f64 = 1e-6;

fn session(gpu: &GpuContext) -> PaintSession {
    let mut s = PaintSession::new(
        gpu,
        CanvasSpec {
            width: WIDTH,
            height: HEIGHT,
            primer: Primer::WhiteGesso,
            weave: Weave::Smooth,
            pigment_slots: vec!["ultramarine_blue".into(), "titanium_white".into()],
        },
    )
    .unwrap();
    s.brush.geometry.max_diameter_px = MAX_DIAMETER_PX;
    s.brush.recipe = vec![(0, 0.8), (1, 0.2)];
    s.brush.load = LOAD;
    s.reload_brush(gpu);
    s
}

fn line_samples() -> Vec<StrokeSample> {
    let count = ((TO_X - FROM_X) / SAMPLE_SPACING_PX) as u32;
    (0..=count)
        .map(|i| StrokeSample {
            x: FROM_X + f64::from(i) * SAMPLE_SPACING_PX,
            y: ROW,
            pressure: PRESSURE,
            tilt_x_deg: TILT_DEG,
            tilt_y_deg: 0.0,
            time_ms: i * SAMPLE_INTERVAL_MS,
        })
        .collect()
}

fn paint_line(gpu: &GpuContext, s: &mut PaintSession) {
    for (i, sample) in line_samples().into_iter().enumerate() {
        if i == 0 {
            s.begin_stroke(gpu, sample)
        } else {
            s.add_samples(&[sample])
        }
        if i % FRAME_EVERY_SAMPLES == 0 {
            encode_frame(gpu, s);
        }
    }
    s.end_stroke();
    encode_frame(gpu, s);
}

fn colour_view(gpu: &GpuContext, s: &PaintSession) -> Vec<u8> {
    presented_rgba(gpu, s, WIDTH, HEIGHT)
}

fn srgb_to_linear(channel: u8) -> f64 {
    let v = f64::from(channel) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(rgba: &[u8]) -> Vec<f64> {
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

fn write_png(name: &str, rgba: &[u8]) {
    support::write_png(name, WIDTH, HEIGHT, rgba);
}

struct Window {
    start_x: usize,
    paint_depth: f64,
    relative_ripple: f64,
}

fn windows(lum: &[f64]) -> Vec<Window> {
    (FIRST_WINDOW_X..(TO_X as usize - WINDOW_PX))
        .step_by(WINDOW_PX)
        .map(|start_x| measure_window(lum, start_x))
        .collect()
}

fn measure_window(lum: &[f64], start_x: usize) -> Window {
    let at = |x: usize, y: usize| lum[y * WIDTH as usize + x];
    let primer = at(4, 4);
    let rows = ((ROW - BAND_HALF_WIDTH_PX) as usize)..=((ROW + BAND_HALF_WIDTH_PX) as usize);
    let mut depth = 0.0;
    let mut ripple = 0.0;
    let mut count = 0.0;
    for y in rows {
        for x in start_x..start_x + WINDOW_PX {
            let neighbourhood = (-ALONG_REACH_PX..=ALONG_REACH_PX)
                .map(|step| at((x as i32 + step) as usize, y))
                .sum::<f64>()
                / f64::from(2 * ALONG_REACH_PX + 1);
            depth += primer - at(x, y);
            ripple += (at(x, y) - neighbourhood).abs();
            count += 1.0;
        }
    }
    let paint_depth = depth / count;
    Window {
        start_x,
        paint_depth,
        relative_ripple: ripple / count / paint_depth.max(f64::EPSILON),
    }
}

#[test]
fn a_fading_stroke_has_no_dab_spaced_rings() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let before = total_volume(&gpu, &s);
    paint_line(&gpu, &mut s);
    let drift = (total_volume(&gpu, &s) - before) / before;
    eprintln!("volume drift {drift:e}");
    assert!(drift.abs() < CONSERVATION_TOLERANCE, "{drift:e}");
    let rgba = colour_view(&gpu, &s);
    write_png("engine-fading.png", &rgba);
    let found = windows(&luminance(&rgba));
    for window in &found {
        eprintln!(
            "x {:4}: paint depth {:.4}, relative ripple {:.4}",
            window.start_x, window.paint_depth, window.relative_ripple
        );
    }
    let full_depth = found[0].paint_depth;
    let painted: Vec<&Window> = found
        .iter()
        .filter(|w| w.paint_depth > MIN_PAINT_DEPTH)
        .collect();
    assert!(
        painted
            .iter()
            .any(|w| w.paint_depth < FADED_SHARE * full_depth),
        "the stroke never fades"
    );
    let worst = painted
        .iter()
        .map(|w| w.relative_ripple)
        .fold(0.0, f64::max);
    assert!(worst < MAX_RELATIVE_LUMINANCE_RIPPLE, "{worst}");
}
