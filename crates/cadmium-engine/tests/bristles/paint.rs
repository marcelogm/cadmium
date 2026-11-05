use cadmium_engine::canvas::StateTexture;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::{StrokeSample, Tool};

use crate::hull::{convex_hull, distance_to_hull, touching_segment_ends};
use crate::support::{drag, frame, session, write_preview};

const CANVAS_WIDTH: usize = 512;
const PAINTED_UM: f32 = 1e-3;
const HULL_MARGIN_PX: f32 = 1.0;
const PIXEL_HALF_DIAGONAL_PX: f32 = std::f32::consts::FRAC_1_SQRT_2;
const PRESS_X: f64 = 256.0;
const PRESS_Y: f64 = 128.0;
const START_X: f64 = 160.0;
const STROKE_STEP_PX: f64 = 2.0;
const STROKE_SAMPLES: u32 = 60;
const SAMPLE_INTERVAL_MS: u32 = 8;
const CAP_RAYS: u32 = 36;
const CAP_RAY_STEP_PX: f32 = 0.25;
const CAP_REACH_PX: f32 = 120.0;
const CAP_EDGE_UM: f32 = 1.0;
const MIN_CAP_VARIATION_PX: f32 = 1.0;
const PRESS_TILT_DEG: f32 = 40.0;
const PRESS_PRESSURE: f32 = 0.7;
const FIELD_ROWS: [f64; 9] = [72.0, 84.0, 96.0, 108.0, 120.0, 132.0, 144.0, 156.0, 168.0];
const BLENDER_X: f64 = 256.0;
const BLENDER_MEASURED_ROW: usize = 120;
const CHANGED_UM: f32 = 1e-2;
const FERRULE_DIAMETER_PX: f32 = 40.0;
const SPLAYED_WIDTH_SHARE: f32 = 1.1;
const NIB_OFFSET_SHARE: f32 = 0.2;

fn pen(x: f64, y: f64, pressure: f32, time_ms: u32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms,
    }
}

fn leaning_pen(x: f64, y: f64) -> StrokeSample {
    StrokeSample {
        tilt_x_deg: PRESS_TILT_DEG,
        ..pen(x, y, PRESS_PRESSURE, 0)
    }
}

fn wet_volume(gpu: &GpuContext, s: &PaintSession) -> Vec<f32> {
    s.canvas.read_f32(gpu, StateTexture::WetVol).unwrap()
}

fn pixel_centre(index: usize) -> [f32; 2] {
    [
        (index % CANVAS_WIDTH) as f32 + 0.5,
        (index / CANVAS_WIDTH) as f32 + 0.5,
    ]
}

#[test]
fn a_pressed_dab_paints_only_where_bristles_touch() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    drag(&gpu, &mut s, &[leaning_pen(PRESS_X, PRESS_Y)]);
    let snapshot = s.engine.bristle_snapshot(&gpu).unwrap();
    let hull = convex_hull(touching_segment_ends(&snapshot));
    let widest_px = snapshot.radii_px.iter().copied().fold(0.0, f32::max);
    let allowed_px = widest_px + HULL_MARGIN_PX + PIXEL_HALF_DIAGONAL_PX;
    let volume = wet_volume(&gpu, &s);
    let painted: Vec<usize> = (0..volume.len())
        .filter(|&index| volume[index] > PAINTED_UM)
        .collect();
    write_preview(&gpu, &s, "bristle-pressed-dab.png");
    assert!(!painted.is_empty(), "the pressed dab laid no paint");
    let strays = painted
        .iter()
        .filter(|&&index| distance_to_hull(pixel_centre(index), &hull) > allowed_px)
        .count();
    assert_eq!(
        strays,
        0,
        "{strays} of {} painted pixels lie off the bristles",
        painted.len()
    );
}

fn rightward_stroke(gpu: &GpuContext, s: &mut PaintSession) {
    let samples: Vec<StrokeSample> = (0..STROKE_SAMPLES)
        .map(|index| {
            let x = START_X + STROKE_STEP_PX * f64::from(index);
            pen(x, PRESS_Y, 0.9, index * SAMPLE_INTERVAL_MS)
        })
        .collect();
    drag(gpu, s, &samples);
}

fn bilinear(volume: &[f32], point: [f32; 2]) -> f32 {
    let x = point[0] - 0.5;
    let y = point[1] - 0.5;
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let at = |dx: f32, dy: f32| volume[(y0 + dy) as usize * CANVAS_WIDTH + (x0 + dx) as usize];
    (at(0.0, 0.0) * (1.0 - fx) + at(1.0, 0.0) * fx) * (1.0 - fy)
        + (at(0.0, 1.0) * (1.0 - fx) + at(1.0, 1.0) * fx) * fy
}

fn cap_edge(volume: &[f32], angle: f32) -> [f32; 2] {
    let centre = [START_X as f32, PRESS_Y as f32];
    let direction = [angle.cos(), angle.sin()];
    let steps = (CAP_REACH_PX / CAP_RAY_STEP_PX) as u32;
    let reach = (0..steps)
        .map(|step| step as f32 * CAP_RAY_STEP_PX)
        .filter(|&reach| {
            let point = [
                centre[0] + direction[0] * reach,
                centre[1] + direction[1] * reach,
            ];
            bilinear(volume, point) > CAP_EDGE_UM
        })
        .fold(0.0, f32::max);
    [
        centre[0] + direction[0] * reach,
        centre[1] + direction[1] * reach,
    ]
}

fn fitted_circle(points: &[[f64; 2]]) -> ([f64; 2], f64) {
    let count = points.len() as f64;
    let mean = |axis: usize| points.iter().map(|point| point[axis]).sum::<f64>() / count;
    let centre = [mean(0), mean(1)];
    let mut sums = [0.0f64; 5];
    for point in points {
        let (u, v) = (point[0] - centre[0], point[1] - centre[1]);
        let squared = u * u + v * v;
        sums = [
            sums[0] + u * u,
            sums[1] + u * v,
            sums[2] + v * v,
            sums[3] + u * squared,
            sums[4] + v * squared,
        ];
    }
    let [uu, uv, vv, uuu, vvv] = sums;
    let determinant = uu * vv - uv * uv;
    let uc = (uuu * vv - vvv * uv) / (2.0 * determinant);
    let vc = (vvv * uu - uuu * uv) / (2.0 * determinant);
    let radius = (uc * uc + vc * vc + (uu + vv) / count).sqrt();
    ([centre[0] + uc, centre[1] + vc], radius)
}

fn largest_departure_from_a_circle_px(points: &[[f64; 2]]) -> f64 {
    let (centre, radius) = fitted_circle(points);
    points
        .iter()
        .map(|point| ((point[0] - centre[0]).hypot(point[1] - centre[1]) - radius).abs())
        .fold(0.0, f64::max)
}

#[test]
fn a_bristle_brush_never_paints_a_perfect_semicircle_at_the_stroke_start() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    rightward_stroke(&gpu, &mut s);
    write_preview(&gpu, &s, "bristle-stroke-start.png");
    let volume = wet_volume(&gpu, &s);
    let edges: Vec<[f64; 2]> = (0..=CAP_RAYS)
        .map(|ray| {
            let angle =
                std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * ray as f32 / CAP_RAYS as f32;
            cap_edge(&volume, angle).map(f64::from)
        })
        .collect();
    let departure = largest_departure_from_a_circle_px(&edges);
    eprintln!("start cap edges {edges:?}, off a circle by up to {departure} px");
    assert!(
        departure >= f64::from(MIN_CAP_VARIATION_PX),
        "the start cap is a semicircle within {departure} px"
    );
}

fn paint_wet_field(gpu: &GpuContext, s: &mut PaintSession) {
    for (row, &y) in FIELD_ROWS.iter().enumerate() {
        let samples: Vec<StrokeSample> = (0..STROKE_SAMPLES)
            .map(|index| {
                let x = 120.0 + 4.5 * f64::from(index);
                pen(x, y, 1.0, (row as u32 * 1000) + index * SAMPLE_INTERVAL_MS)
            })
            .collect();
        drag(gpu, s, &samples);
    }
}

fn blender_stroke(gpu: &GpuContext, s: &mut PaintSession) {
    let samples: Vec<StrokeSample> = (0..STROKE_SAMPLES)
        .map(|index| {
            let y = 60.0 + 2.0 * f64::from(index);
            pen(BLENDER_X, y, 1.0, index * SAMPLE_INTERVAL_MS)
        })
        .collect();
    drag(gpu, s, &samples);
}

fn changed_span_px(before: &[f32], after: &[f32], row: usize) -> f32 {
    let changed: Vec<usize> = (0..CANVAS_WIDTH)
        .filter(|&x| {
            let index = row * CANVAS_WIDTH + x;
            (after[index] - before[index]).abs() > CHANGED_UM
        })
        .collect();
    match (changed.first(), changed.last()) {
        (Some(first), Some(last)) => (last - first + 1) as f32,
        _ => 0.0,
    }
}

#[test]
fn the_blender_moves_paint_through_its_bristles_too() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    paint_wet_field(&gpu, &mut s);
    let before = wet_volume(&gpu, &s);
    s.brush.tool = Tool::Blender;
    s.clean_brush(&gpu);
    blender_stroke(&gpu, &mut s);
    frame(&gpu, &mut s);
    write_preview(&gpu, &s, "bristle-blender-field.png");
    let after = wet_volume(&gpu, &s);
    let picked = s.engine.brush_totals(&gpu).unwrap();
    assert!(
        picked.volume > 0.0,
        "the blender's bristles picked up nothing"
    );
    let span = changed_span_px(&before, &after, BLENDER_MEASURED_ROW);
    eprintln!("the blender changed a {span} px span");
    assert!(
        span > SPLAYED_WIDTH_SHARE * FERRULE_DIAMETER_PX,
        "the blender reached only {span} px across, no wider than its ferrule"
    );
}

fn painted_centroid_px(volume: &[f32]) -> [f64; 2] {
    let (mut sum, mut weight) = ([0.0f64; 2], 0.0f64);
    for (index, &um) in volume.iter().enumerate() {
        if um > PAINTED_UM {
            let [x, y] = pixel_centre(index);
            sum[0] += f64::from(x) * f64::from(um);
            sum[1] += f64::from(y) * f64::from(um);
            weight += f64::from(um);
        }
    }
    [sum[0] / weight, sum[1] / weight]
}

#[test]
fn a_leaning_brush_paints_under_the_pen_nib() {
    let gpu = GpuContext::new().unwrap();
    let leans = [
        (PRESS_TILT_DEG, 0.0),
        (-PRESS_TILT_DEG, 0.0),
        (0.0, PRESS_TILT_DEG),
        (0.0, -PRESS_TILT_DEG),
    ];
    for (tilt_x_deg, tilt_y_deg) in leans {
        let mut s = session(&gpu);
        let nib = StrokeSample {
            tilt_x_deg,
            tilt_y_deg,
            ..pen(PRESS_X, PRESS_Y, PRESS_PRESSURE, 0)
        };
        drag(&gpu, &mut s, &[nib]);
        let [x, y] = painted_centroid_px(&wet_volume(&gpu, &s));
        let off_px = (x - PRESS_X).hypot(y - PRESS_Y);
        assert!(
            off_px < f64::from(NIB_OFFSET_SHARE * FERRULE_DIAMETER_PX),
            "tilt ({tilt_x_deg}, {tilt_y_deg}) painted {off_px:.1} px from the nib"
        );
    }
}
