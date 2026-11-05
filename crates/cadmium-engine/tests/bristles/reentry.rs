use cadmium_engine::canvas::StateTexture;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::stroke::StrokeSample;

use crate::support::{drag, session, write_preview};

const CANVAS_WIDTH: usize = 512;
const SAMPLE_STEP_PX: f64 = 4.0;
const SAMPLE_INTERVAL_MS: u32 = 8;
const PRESSURE: f32 = 0.8;
const PAINTED_UM: f32 = 1e-3;
const LOOP_AROUND_THE_CORNER: [(f64, f64); 5] = [
    (300.0, 100.0),
    (300.0, -250.0),
    (-250.0, -250.0),
    (-250.0, 200.0),
    (100.0, 200.0),
];
const UNTOUCHED_COLUMNS: std::ops::Range<usize> = 0..8;
const UNTOUCHED_ROWS: std::ops::Range<usize> = 100..165;

fn along(corners: &[(f64, f64)]) -> Vec<StrokeSample> {
    let mut points = vec![corners[0]];
    for pair in corners.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        let steps = ((x1 - x0).hypot(y1 - y0) / SAMPLE_STEP_PX).ceil() as u32;
        points.extend((1..=steps).map(|step| {
            let share = f64::from(step) / f64::from(steps);
            (x0 + (x1 - x0) * share, y0 + (y1 - y0) * share)
        }));
    }
    (0u32..)
        .zip(points)
        .map(|(index, (x, y))| StrokeSample {
            x,
            y,
            pressure: PRESSURE,
            tilt_x_deg: 0.0,
            tilt_y_deg: 0.0,
            time_ms: index * SAMPLE_INTERVAL_MS,
        })
        .collect()
}

#[test]
fn a_stroke_that_comes_back_onto_the_canvas_does_not_streak_from_where_it_left() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    drag(&gpu, &mut s, &along(&LOOP_AROUND_THE_CORNER));
    write_preview(&gpu, &s, "bristle-reentry.png");
    let volume = s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
    let streaked = UNTOUCHED_ROWS
        .flat_map(|y| UNTOUCHED_COLUMNS.map(move |x| y * CANVAS_WIDTH + x))
        .filter(|&texel| volume[texel] > PAINTED_UM)
        .count();
    assert!(
        volume.iter().any(|&amount| amount > PAINTED_UM),
        "the stroke laid paint"
    );
    assert_eq!(
        streaked, 0,
        "{streaked} pixels painted between the exit and the re-entry"
    );
}
