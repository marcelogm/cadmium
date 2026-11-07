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
const BAND_HALF_WIDTH_PX: usize = 12;
const WINDOW_PX: usize = 48;
const FIRST_WINDOW_X: usize = 160;
const LOAD: f32 = 0.2;
const TAIL_START_SHARE: f64 = 0.5;
const TAIL_END_SHARE: f64 = 0.05;
const MIN_TAIL_WINDOWS: usize = 3;
const MIN_TAIL_VARIATION: f64 = 0.3;
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
        .map(|index| StrokeSample {
            x: FROM_X + f64::from(index) * SAMPLE_SPACING_PX,
            y: ROW,
            pressure: PRESSURE,
            tilt_x_deg: TILT_DEG,
            tilt_y_deg: 0.0,
            time_ms: index * SAMPLE_INTERVAL_MS,
        })
        .collect()
}

fn paint_line(gpu: &GpuContext, s: &mut PaintSession) {
    for (i, sample) in line_samples().into_iter().enumerate() {
        if i == 0 {
            s.begin_stroke(sample)
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

fn write_fading_preview(gpu: &GpuContext, s: &PaintSession) {
    let rgba = presented_rgba(gpu, s, WIDTH, HEIGHT);
    support::write_png("engine-fading.png", WIDTH, HEIGHT, &rgba);
}

struct Window {
    start_x: usize,
    mean_um: f64,
    variation: f64,
}

fn band_rows() -> std::ops::RangeInclusive<usize> {
    let row = ROW as usize;
    row - BAND_HALF_WIDTH_PX..=row + BAND_HALF_WIDTH_PX
}

fn column_variation(volume: &[f32], x: usize) -> (f64, f64) {
    let across: Vec<f64> = band_rows()
        .map(|y| f64::from(volume[y * WIDTH as usize + x]).max(0.0))
        .collect();
    let mean = across.iter().sum::<f64>() / across.len() as f64;
    let spread = across
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / across.len() as f64;
    (mean, spread.sqrt() / mean.max(f64::EPSILON))
}

fn measure_window(volume: &[f32], start_x: usize) -> Window {
    let columns: Vec<(f64, f64)> = (start_x..start_x + WINDOW_PX)
        .map(|x| column_variation(volume, x))
        .collect();
    let count = columns.len() as f64;
    Window {
        start_x,
        mean_um: columns.iter().map(|column| column.0).sum::<f64>() / count,
        variation: columns.iter().map(|column| column.1).sum::<f64>() / count,
    }
}

fn windows(volume: &[f32]) -> Vec<Window> {
    (FIRST_WINDOW_X..(TO_X as usize - WINDOW_PX))
        .step_by(WINDOW_PX)
        .map(|start_x| measure_window(volume, start_x))
        .collect()
}

#[test]
fn a_brush_that_runs_dry_fades_in_threads_not_evenly() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let before = total_volume(&gpu, &s);
    paint_line(&gpu, &mut s);
    let drift = (total_volume(&gpu, &s) - before) / before;
    eprintln!("volume drift {drift:e}");
    assert!(drift.abs() < CONSERVATION_TOLERANCE, "{drift:e}");
    write_fading_preview(&gpu, &s);
    let volume = s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
    let found = windows(&volume);
    for window in &found {
        eprintln!(
            "x {:4}: mean {:.2} um, variation across the stroke {:.3}",
            window.start_x, window.mean_um, window.variation
        );
    }
    let full_um = found[0].mean_um;
    let tail: Vec<&Window> = found
        .iter()
        .filter(|window| {
            window.mean_um < TAIL_START_SHARE * full_um && window.mean_um > TAIL_END_SHARE * full_um
        })
        .collect();
    assert!(tail.len() >= MIN_TAIL_WINDOWS, "the stroke never fades");
    let variation = tail.iter().map(|window| window.variation).sum::<f64>() / tail.len() as f64;
    eprintln!("tail variation across the stroke {variation:.3}");
    assert!(
        variation >= MIN_TAIL_VARIATION,
        "the tail fades evenly: variation {variation:.3}"
    );
}
