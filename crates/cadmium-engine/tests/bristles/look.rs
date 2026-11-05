use cadmium_engine::canvas::StateTexture;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::{StrokeSample, Tool};

use crate::support::{drag, frame, median, session, write_preview};

pub(super) const WIDTH: usize = 512;
pub(super) const HEIGHT: usize = 256;
pub(super) const ROW: f64 = 128.0;
const FROM_X: f64 = 40.0;
const TO_X: f64 = 470.0;
const SAMPLES: u32 = 144;
const SAMPLE_INTERVAL_MS: u32 = 6;
const TILT_DEG: f32 = 20.0;
const PAINTED_UM: f32 = 1.0;
const BLENDED_UM: f32 = 10.0;
const MIN_SPLAY_SHARE: f32 = 1.1;
const ULTRAMARINE: usize = 0;
const CADMIUM_YELLOW: usize = 1;

const WAVE_MEAN_PRESSURE: f32 = 0.55;
const WAVE_PRESSURE_SWING: f32 = 0.4;
const WAVES: f64 = 2.0;
const WAVE_BODY_X: std::ops::Range<usize> = 90..420;
const MIN_CENTRE_WOBBLE_PX: f64 = 0.25;
const MIN_PRESSURE_WIDTH_RATIO: f64 = 2.0;

const CURVE_SWING_PX: f64 = 60.0;
const CURVE_PRESSURE: f32 = 0.7;
const MIN_APEX_ASYMMETRY_PX: f64 = 2.0;

const DRY_RUN_DIAMETER_PX: f32 = 30.0;
const DRY_RUN_LOAD: f32 = 0.08;
const DRY_RUN_PRESSURE: f32 = 0.8;
const MIN_END_SPREAD_SHARE: f64 = 0.1;

const FIELD_ROWS: std::ops::RangeInclusive<u32> = 0..=12;
const FIELD_ROW_STEP_PX: f64 = 10.0;
const FIELD_TOP_PX: f64 = 68.0;
const BOUNDARY_X: f64 = 256.0;

fn sample(x: f64, y: f64, pressure: f32, index: u32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure,
        tilt_x_deg: TILT_DEG,
        tilt_y_deg: 0.0,
        time_ms: index * SAMPLE_INTERVAL_MS,
    }
}

fn along(index: u32) -> f64 {
    f64::from(index) / f64::from(SAMPLES)
}

pub(super) fn straight(y: f64, pressure: impl Fn(f64) -> f32) -> Vec<StrokeSample> {
    (0..=SAMPLES)
        .map(|index| {
            let progress = along(index);
            sample(
                FROM_X + (TO_X - FROM_X) * progress,
                y,
                pressure(progress),
                index,
            )
        })
        .collect()
}

pub(super) fn volume(gpu: &GpuContext, painting: &PaintSession) -> Vec<f32> {
    painting.canvas.read_f32(gpu, StateTexture::WetVol).unwrap()
}

fn painted(volume: &[f32]) -> Vec<bool> {
    volume.iter().map(|&um| um > PAINTED_UM).collect()
}

fn column(marked: &[bool], x: usize) -> Vec<bool> {
    (0..HEIGHT).map(|y| marked[y * WIDTH + x]).collect()
}

fn root_mean_square(values: &[f64]) -> f64 {
    (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt()
}

fn deviations_from_mean(values: &[f64]) -> Vec<f64> {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values.iter().map(|value| value - mean).collect()
}

fn centre_wobble_px(marked: &[bool]) -> f64 {
    let centres: Vec<f64> = WAVE_BODY_X
        .map(|x| {
            let marked = column(marked, x);
            let top = marked.iter().position(|&is_marked| is_marked).unwrap_or(0);
            let bottom = marked.iter().rposition(|&is_marked| is_marked).unwrap_or(0);
            (top + bottom) as f64 / 2.0
        })
        .collect();
    root_mean_square(&deviations_from_mean(&centres))
}

#[test]
fn a_pressure_wave_paints_edges_that_waver_independently() {
    let gpu = GpuContext::new().unwrap();
    let mut painting = session(&gpu);
    let wave = |progress: f64| {
        WAVE_MEAN_PRESSURE
            + WAVE_PRESSURE_SWING * (std::f64::consts::TAU * WAVES * progress).sin() as f32
    };
    drag(&gpu, &mut painting, &straight(ROW, wave));
    write_preview(&gpu, &painting, "bristle-pressure-wave.png");
    let painted = painted(&volume(&gpu, &painting));
    let widths: Vec<f64> = WAVE_BODY_X
        .map(|x| {
            column(&painted, x)
                .iter()
                .filter(|&&is_painted| is_painted)
                .count() as f64
        })
        .collect();
    let widest = widths.iter().cloned().fold(0.0, f64::max);
    let narrowest = widths.iter().cloned().fold(f64::INFINITY, f64::min);
    let wobble = centre_wobble_px(&painted);
    eprintln!("width {narrowest}..{widest} px, centre wobble {wobble:.2} px");
    assert!(
        widest >= MIN_PRESSURE_WIDTH_RATIO * narrowest,
        "pressure barely changes the width: {narrowest}..{widest} px"
    );
    assert!(
        wobble >= MIN_CENTRE_WOBBLE_PX,
        "the edges move together like a stamp's: wobble {wobble:.2} px"
    );
}

fn curve_y(progress: f64) -> f64 {
    ROW + CURVE_SWING_PX * (std::f64::consts::TAU * progress).sin()
}

fn apex_extents(volume: &[f32], progress: f64) -> [f64; 2] {
    let x = (FROM_X + (TO_X - FROM_X) * progress).round() as usize;
    let y = curve_y(progress).round() as usize;
    let painted = column(&painted(volume), x);
    let above = (0..y).rev().take_while(|&row| painted[row]).count();
    let below = (y..HEIGHT).take_while(|&row| painted[row]).count();
    [above as f64, below as f64]
}

#[test]
fn an_s_curve_leaves_different_tracks_inside_and_outside_each_bend() {
    let gpu = GpuContext::new().unwrap();
    let mut painting = session(&gpu);
    let samples: Vec<StrokeSample> = (0..=SAMPLES)
        .map(|index| {
            let progress = along(index);
            sample(
                FROM_X + (TO_X - FROM_X) * progress,
                curve_y(progress),
                CURVE_PRESSURE,
                index,
            )
        })
        .collect();
    drag(&gpu, &mut painting, &samples);
    write_preview(&gpu, &painting, "bristle-s-curve.png");
    let volume = volume(&gpu, &painting);
    let lower = apex_extents(&volume, 0.25);
    let upper = apex_extents(&volume, 0.75);
    eprintln!("lower apex above/below {lower:?} px, upper apex above/below {upper:?} px");
    for [above, below] in [lower, upper] {
        let asymmetry = (above - below).abs();
        assert!(
            asymmetry >= MIN_APEX_ASYMMETRY_PX,
            "the bend is painted the same inside and out: {above} and {below} px"
        );
    }
}

fn row_ends_px(volume: &[f32], rows: &[usize]) -> Vec<f64> {
    rows.iter()
        .filter_map(|&y| {
            (0..WIDTH)
                .rev()
                .find(|&x| volume[y * WIDTH + x] > PAINTED_UM)
                .map(|x| x as f64)
        })
        .collect()
}

fn painted_rows(volume: &[f32], x: usize) -> Vec<usize> {
    let painted = column(&painted(volume), x);
    (0..HEIGHT).filter(|&y| painted[y]).collect()
}

#[test]
fn a_stroke_running_dry_ends_in_threads_of_different_lengths() {
    let gpu = GpuContext::new().unwrap();
    let mut painting = session(&gpu);
    painting.brush.geometry.max_diameter_px = DRY_RUN_DIAMETER_PX;
    painting.brush.load = DRY_RUN_LOAD;
    painting.reload_brush(&gpu);
    drag(&gpu, &mut painting, &straight(ROW, |_| DRY_RUN_PRESSURE));
    write_preview(&gpu, &painting, "bristle-runs-dry.png");
    let volume = volume(&gpu, &painting);
    let rows = painted_rows(&volume, ((FROM_X + TO_X) / 2.0) as usize);
    let ends = row_ends_px(&volume, &rows);
    let first = ends.iter().cloned().fold(f64::INFINITY, f64::min);
    let last = ends.iter().cloned().fold(0.0, f64::max);
    let spread = (last - first) / (TO_X - FROM_X);
    eprintln!(
        "{} rows end between x {first} and {last}: spread {spread:.3}",
        ends.len()
    );
    assert!(last < TO_X, "the stroke never ran dry");
    assert!(
        spread >= MIN_END_SPREAD_SHARE,
        "the rows run dry together: spread {spread:.3}"
    );
}

fn paint_fields(gpu: &GpuContext, painting: &mut PaintSession) {
    for (slot, from, to) in [
        (ULTRAMARINE, FROM_X, BOUNDARY_X),
        (CADMIUM_YELLOW, BOUNDARY_X, TO_X),
    ] {
        painting.brush.recipe = vec![(slot, 1.0)];
        for row in FIELD_ROWS {
            painting.reload_brush(gpu);
            let y = FIELD_TOP_PX + FIELD_ROW_STEP_PX * f64::from(row);
            let samples: Vec<StrokeSample> = (0..=SAMPLES)
                .map(|index| sample(from + (to - from) * along(index), y, 1.0, index))
                .collect();
            drag(gpu, painting, &samples);
        }
    }
}

#[test]
fn a_blender_splays_into_a_track_wider_than_its_ferrule() {
    let gpu = GpuContext::new().unwrap();
    let mut painting = session(&gpu);
    paint_fields(&gpu, &mut painting);
    let before = volume(&gpu, &painting);
    painting.brush.tool = Tool::Blender;
    painting.clean_brush(&gpu);
    drag(&gpu, &mut painting, &straight(ROW, |_| CURVE_PRESSURE));
    frame(&gpu, &mut painting);
    write_preview(&gpu, &painting, "bristle-blender.png");
    let after = volume(&gpu, &painting);
    let changed: Vec<bool> = before
        .iter()
        .zip(&after)
        .map(|(was, is)| (is - was).abs() > BLENDED_UM)
        .collect();
    let widths: Vec<usize> = WAVE_BODY_X
        .map(|x| {
            let changed = column(&changed, x);
            let top = changed.iter().position(|&is_marked| is_marked).unwrap_or(0);
            let bottom = changed
                .iter()
                .rposition(|&is_marked| is_marked)
                .unwrap_or(0);
            bottom.saturating_sub(top)
        })
        .collect();
    let width_px = median(widths) as f32;
    let ferrule_px = painting.brush.geometry.max_diameter_px;
    eprintln!("the blender track is {width_px} px wide under a {ferrule_px} px ferrule");
    assert!(
        width_px >= MIN_SPLAY_SHARE * ferrule_px,
        "the blender track is no wider than a stamp of its ferrule: {width_px} px"
    );
}
