use std::ops::Range;

use cadmium_engine::{
    canvas::StateTexture, engine::PaintEngine, gpu::GpuContext, session::PaintSession,
    surface::Surface, tube::Nozzle,
};

use super::scene::{
    LAID_TILT_DEG, MIXING_PRESSURE, Pass, TrayPaint, knife_pass, mixing_session, queue_pass,
    tray_and_tools, weighted_variance, write_tray_preview,
};
use crate::FRAME_SECONDS;

const BOUNDARY_X: f64 = 350.0;
const BLOB_TOP_Y: f32 = 230.0;
const BLOB_BOTTOM_Y: f32 = 390.0;
const BLOB_CORD_RADIUS: f32 = 13.0;
const BLOB_CORD_FRAMES: usize = 60;
const A_CORDS_X: [f32; 2] = [313.0, 337.0];
const B_CORDS_X: [f32; 2] = [363.0, 387.0];
const PASS_CENTRE_Y: f64 = 310.0;
const PASS_HALF_LENGTH_PX: f64 = 95.0;
/// Each pass leans this far off the boundary, alternately either way, as a hand
/// working paint back and forth never runs twice along the same line.
const PASS_LEAN_DEG: f64 = 25.0;
/// The blade's rows measured for streaks: those well inside its length.
const SWEPT_HALF_LENGTH_PX: f64 = 36.0;
const SWEPT_X: Range<usize> = 270..431;
const SWEPT_Y: Range<usize> = 210..411;
const FIRST_PASS_KEEPS: f64 = 0.6;
/// How much more streaky than the heaps' own boundary the first pass must leave
/// the paint, and an uneven edge than an even one: both measured in one run, so
/// the scene's own score (about 0.82, the boundary leaning off the pass) cancels.
const FIRST_PASS_GAIN: f64 = 0.015;
const EDGE_GAIN: f64 = 0.01;
const TEN_PASSES_LEAVE: f64 = 0.25;
const MONOTONE_SLACK: f64 = 1.05;
const VOLUME_TOLERANCE: f64 = 1e-6;
const PIGMENT_TOLERANCE: f64 = 1e-3;
const PREVIEWED_PASSES: [usize; 5] = [0, 1, 2, 4, 8];

fn squeeze_cord(gpu: &GpuContext, s: &mut PaintSession, slot: usize, x: f32) {
    let at = |frame: usize| {
        let t = frame as f32 / BLOB_CORD_FRAMES as f32;
        [x, BLOB_TOP_Y + t * (BLOB_BOTTOM_Y - BLOB_TOP_Y)]
    };
    s.begin_hold(gpu, Surface::Tray);
    for frame in 0..BLOB_CORD_FRAMES {
        s.squeeze(
            gpu,
            Nozzle {
                from: at(frame),
                to: at(frame + 1),
                radius: BLOB_CORD_RADIUS,
                slot,
                pressure: 1.0,
                seconds: FRAME_SECONDS,
                still_reach_px: 0.0,
            },
        );
    }
    s.end_hold();
}

/// Two broad heaps side by side, A left of the boundary and B right of it.
fn two_blobs(gpu: &GpuContext, s: &mut PaintSession) {
    for x in A_CORDS_X {
        squeeze_cord(gpu, s, 0, x);
    }
    for x in B_CORDS_X {
        squeeze_cord(gpu, s, 1, x);
    }
}

/// The direction of the `index`th pass: down and back up across the heaps, the
/// blade's length across the boundary, leaning alternately either way.
fn heading(index: usize) -> [f64; 2] {
    let lean = PASS_LEAN_DEG.to_radians() * if index % 4 < 2 { 1.0 } else { -1.0 };
    let down = if index.is_multiple_of(2) { 1.0 } else { -1.0 };
    [lean.sin() * down, lean.cos() * down]
}

fn crossing_pass(index: usize) -> Pass {
    let [dx, dy] = heading(index);
    let reach = PASS_HALF_LENGTH_PX;
    Pass {
        from: [BOUNDARY_X - dx * reach, PASS_CENTRE_Y - dy * reach],
        to: [BOUNDARY_X + dx * reach, PASS_CENTRE_Y + dy * reach],
        tilt_x_deg: LAID_TILT_DEG,
        pressure: MIXING_PRESSURE,
    }
}

/// How much the colour in the band the `index`th pass swept changes across that
/// pass rather than along it: the share of the squared change in A's share from
/// pixel to pixel that runs along the blade. Streaks left by the pass change only
/// across it (near one); blotches change every way alike (one half).
fn streakiness(paint: &TrayPaint, index: usize) -> f64 {
    let [dx, dy] = heading(index);
    let (mut across, mut along) = (0.0, 0.0);
    let pixels = SWEPT_Y.flat_map(|y| SWEPT_X.map(move |x| (x, y)));
    for (x, y) in pixels.filter(|&(x, y)| in_swept_band(x, y, [dx, dy])) {
        let Some([gx, gy]) = share_gradient(paint, x, y) else {
            continue;
        };
        let w = f64::from(paint.volume_at(x, y));
        across += w * (-gx * dy + gy * dx).powi(2);
        along += w * (gx * dx + gy * dy).powi(2);
    }
    across / (across + along)
}

/// Whether the pixel lies well inside the band a pass heading `[dx, dy]` swept.
fn in_swept_band(x: usize, y: usize, [dx, dy]: [f64; 2]) -> bool {
    let (rx, ry) = (x as f64 + 0.5 - BOUNDARY_X, y as f64 + 0.5 - PASS_CENTRE_Y);
    (-rx * dy + ry * dx).abs() <= SWEPT_HALF_LENGTH_PX
        && (rx * dx + ry * dy).abs() <= PASS_HALF_LENGTH_PX
}

/// The change in A's share per pixel across `(x, y)`, where it and its four
/// neighbours are all painted.
fn share_gradient(paint: &TrayPaint, x: usize, y: usize) -> Option<[f64; 2]> {
    let left = paint.share_of_a(x - 1, y)?;
    let right = paint.share_of_a(x + 1, y)?;
    let up = paint.share_of_a(x, y - 1)?;
    let down = paint.share_of_a(x, y + 1)?;
    Some([(right - left) / 2.0, (down - up) / 2.0])
}

struct PassMeasure {
    variance: f64,
    streakiness: f64,
}

fn mix_across_the_boundary(
    gpu: &GpuContext,
    s: &mut PaintSession,
    passes: usize,
    name: &str,
) -> Vec<PassMeasure> {
    two_blobs(gpu, s);
    crate::frame(gpu, s);
    let measure = |s: &PaintSession, index: usize| {
        let paint = TrayPaint::read(gpu, s);
        PassMeasure {
            variance: weighted_variance(&paint, SWEPT_X, SWEPT_Y),
            streakiness: streakiness(&paint, index),
        }
    };
    let mut measures = vec![measure(s, 0)];
    write_tray_preview(gpu, s, &format!("{name}-0.png"));
    for index in 0..passes {
        knife_pass(gpu, s, crossing_pass(index));
        measures.push(measure(s, index));
        if PREVIEWED_PASSES.contains(&(index + 1)) {
            write_tray_preview(gpu, s, &format!("{name}-{}.png", index + 1));
        }
    }
    measures
}

#[test]
fn the_first_pass_leaves_streaks_and_every_pass_mixes_more() {
    let gpu = GpuContext::new().unwrap();
    let mut s = mixing_session(&gpu);
    let measures = mix_across_the_boundary(&gpu, &mut s, 10, "streaks");
    let initial = measures[0].variance;
    let relative: Vec<f64> = measures.iter().map(|m| m.variance / initial).collect();
    let streaky: Vec<f64> = measures.iter().map(|m| m.streakiness).collect();
    eprintln!("relative variance {relative:?}");
    eprintln!("streakiness {streaky:?}");
    assert!(relative[1] >= FIRST_PASS_KEEPS);
    assert!(streaky[1] - streaky[0] >= FIRST_PASS_GAIN);
    for pair in relative.windows(2) {
        assert!(pair[1] <= pair[0] * MONOTONE_SLACK, "{relative:?}");
    }
    assert!(relative[10] <= TEN_PASSES_LEAVE);
}

#[test]
fn an_even_edge_leaves_fewer_streaks() {
    let gpu = GpuContext::new().unwrap();
    let mut uneven = mixing_session(&gpu);
    let streaky = mix_across_the_boundary(&gpu, &mut uneven, 1, "uneven")[1].streakiness;
    let mut even = mixing_session(&gpu);
    even.engine.params_mut().set("face_edge_unevenness", 0.0);
    let smooth = mix_across_the_boundary(&gpu, &mut even, 1, "even")[1].streakiness;
    eprintln!("streakiness uneven {streaky}, even {smooth}");
    assert!(streaky - smooth >= EDGE_GAIN);
}

#[test]
fn mixing_across_the_boundary_conserves_paint() {
    let gpu = GpuContext::new().unwrap();
    let mut s = mixing_session(&gpu);
    two_blobs(&gpu, &mut s);
    let before = tray_and_tools(&gpu, &s);
    for index in 0..4 {
        knife_pass(&gpu, &mut s, crossing_pass(index));
    }
    let after = tray_and_tools(&gpu, &s);
    assert!(((after.volume - before.volume) / before.volume).abs() <= VOLUME_TOLERANCE);
    for slot in 0..2 {
        let drift = (after.pigment[slot] - before.pigment[slot]) / before.volume;
        assert!(drift.abs() < PIGMENT_TOLERANCE, "{slot}: {drift:e}");
    }
}

/// How two queued passes get painted.
#[derive(Clone, Copy)]
enum Painting {
    FlushedTogether,
    FlushedApart,
    AFewDabsAFrame,
}

const FEW_DABS_A_FRAME: usize = 7;
const MAX_FRAMES: usize = 1000;

fn paint_a_few_dabs_a_frame(gpu: &GpuContext, s: &mut PaintSession) {
    s.limit_dabs_per_frame(Some(FEW_DABS_A_FRAME));
    for _ in 0..MAX_FRAMES {
        if !s.tray_has_pending_work() {
            return;
        }
        crate::frame(gpu, s);
    }
}

fn tray_after_two_queued_passes(gpu: &GpuContext, painting: Painting) -> Vec<u8> {
    let mut s = mixing_session(gpu);
    two_blobs(gpu, &mut s);
    queue_pass(&mut s, &crossing_pass(0));
    if matches!(painting, Painting::FlushedApart) {
        s.flush(gpu);
    }
    queue_pass(&mut s, &crossing_pass(2));
    if matches!(painting, Painting::AFewDabsAFrame) {
        paint_a_few_dabs_a_frame(gpu, &mut s);
    }
    s.flush(gpu);
    let mut bytes = s.tray.read_bytes(gpu, StateTexture::WetVol).unwrap();
    bytes.extend(s.tray.read_bytes(gpu, StateTexture::WetPig0).unwrap());
    bytes
}

#[test]
fn strokes_paint_the_same_however_their_dabs_are_split() {
    let gpu = GpuContext::new().unwrap();
    let together = tray_after_two_queued_passes(&gpu, Painting::FlushedTogether);
    for painting in [Painting::FlushedApart, Painting::AFewDabsAFrame] {
        let other = tray_after_two_queued_passes(&gpu, painting);
        assert!(
            other == together,
            "the tray depends on how its dabs were split"
        );
    }
}
