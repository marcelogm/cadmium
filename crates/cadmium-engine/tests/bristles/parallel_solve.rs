use cadmium_engine::canvas::StateTexture;
use cadmium_engine::engine::BristleSnapshot;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::support::{drag, line, session};

const WIDTH: usize = 512;
const HEIGHT: usize = 256;
const CENTRE_Y: f64 = 128.0;
const RIDGE_PERIOD_PX: usize = 16;
const RIDGE_WIDTH_PX: usize = 6;
const RIDGE_HEIGHT_UM: f32 = 1200.0;
const TILT_DEG: f32 = 20.0;
/// The fresh stroke presses in over this many samples, so contacts land a few at a time.
const PRESS_IN_SAMPLES: usize = 8;
const PRESS_IN_PRESSURE: f32 = 0.6;
/// Three samples past the first contacts.
const FRESH_SAMPLES: usize = 7;
const WORN_SAMPLES: usize = 3;
/// They differ only where the compiler fuses a multiply-add differently.
const POSITION_TOLERANCE_PX: f32 = 1.0e-3;
const FORCE_TOLERANCE_SHARE: f32 = 1.0e-3;
const FORCE_FLOOR_N: f32 = 1.0e-9;
const SHIFT_TOLERANCE_RAD: f32 = 1.0e-5;

fn on_flat_canvas(gpu: &GpuContext, reference: bool) -> PaintSession {
    let mut s = session(gpu);
    s.engine.use_reference_bristle_solve(reference);
    s
}

fn pressing_in(samples: &mut [StrokeSample]) {
    for (index, sample) in samples.iter_mut().enumerate() {
        let share = (index + 1) as f32 / PRESS_IN_SAMPLES as f32;
        sample.pressure = PRESS_IN_PRESSURE * share.min(1.0);
    }
}

/// Stick and slip over ridges magnify a last-bit difference about 10⁴-fold within a few
/// dabs: the serial solve against itself, with the stroke nudged 1e-4 px along x, ends
/// 0.46 px apart after 5 samples and 5.2 px after 61. Over ridges the solves are compared
/// over a handful of dabs from the same state.
fn on_ridges(gpu: &GpuContext, reference: bool) -> PaintSession {
    let mut s = session(gpu);
    let heights: Vec<f32> = (0..WIDTH * HEIGHT)
        .map(|pixel| {
            if pixel % WIDTH % RIDGE_PERIOD_PX < RIDGE_WIDTH_PX {
                RIDGE_HEIGHT_UM
            } else {
                0.0
            }
        })
        .collect();
    s.canvas.write_f32(gpu, StateTexture::DryHeight, &heights);
    s.engine.use_reference_bristle_solve(reference);
    s
}

fn assert_same_vertices(bristle: usize, serial: &[[f32; 3]], parallel: &[[f32; 3]]) {
    for (vertex, (a, b)) in serial.iter().zip(parallel).enumerate() {
        for axis in 0..3 {
            assert!(
                (a[axis] - b[axis]).abs() <= POSITION_TOLERANCE_PX,
                "bristle {bristle} vertex {vertex}: reference {a:?}, grouped {b:?}"
            );
        }
    }
}

fn assert_same_anchors(bristle: usize, serial: &[[f32; 4]], parallel: &[[f32; 4]]) {
    for (vertex, (x, y)) in serial.iter().zip(parallel).enumerate() {
        assert_eq!(
            x[3].signum(),
            y[3].signum(),
            "bristle {bristle} vertex {vertex}: modes {x:?} {y:?}"
        );
        for axis in 0..3 {
            assert!(
                (x[axis] - y[axis]).abs() <= POSITION_TOLERANCE_PX,
                "bristle {bristle} vertex {vertex}: anchors {x:?} {y:?}"
            );
        }
    }
}

fn assert_same_shifts(bristle: usize, serial: &[[f32; 3]], parallel: &[[f32; 3]]) {
    for (hinge, (x, y)) in serial.iter().zip(parallel).enumerate() {
        for axis in 0..3 {
            assert!(
                (x[axis] - y[axis]).abs() <= SHIFT_TOLERANCE_RAD,
                "bristle {bristle} hinge {hinge}: shifts {x:?} {y:?}"
            );
        }
    }
}

fn assert_same_strands(reference: &BristleSnapshot, grouped: &BristleSnapshot) {
    assert_eq!(reference.chains_px.len(), grouped.chains_px.len());
    for (bristle, (serial, parallel)) in reference
        .chains_px
        .iter()
        .zip(&grouped.chains_px)
        .enumerate()
    {
        assert_same_vertices(bristle, serial, parallel);
    }
    for (bristle, (a, b)) in reference.anchors.iter().zip(&grouped.anchors).enumerate() {
        assert_same_anchors(bristle, a, b);
    }
    for (bristle, (a, b)) in reference.plastic.iter().zip(&grouped.plastic).enumerate() {
        assert_same_shifts(bristle, a, b);
    }
    for (bristle, (a, b)) in reference
        .normal_forces_n
        .iter()
        .zip(&grouped.normal_forces_n)
        .enumerate()
    {
        let scale = a.abs().max(b.abs()).max(FORCE_FLOOR_N);
        assert!(
            (a - b).abs() <= FORCE_TOLERANCE_SHARE * scale,
            "bristle {bristle}: reference {a} N, grouped {b} N"
        );
    }
}

fn snapshots(
    gpu: &GpuContext,
    reference: &PaintSession,
    grouped: &PaintSession,
) -> (BristleSnapshot, BristleSnapshot) {
    (
        reference.engine.bristle_snapshot(gpu).unwrap(),
        grouped.engine.bristle_snapshot(gpu).unwrap(),
    )
}

#[test]
fn the_grouped_solve_matches_the_serial_one_over_ridges() {
    let gpu = GpuContext::new().unwrap();
    let mut reference = on_ridges(&gpu, true);
    let mut grouped = on_ridges(&gpu, false);
    let mut samples = line(CENTRE_Y, 60.0, 140.0, 0.9, TILT_DEG);
    pressing_in(&mut samples);
    drag(&gpu, &mut reference, &samples[..FRESH_SAMPLES]);
    drag(&gpu, &mut grouped, &samples[..FRESH_SAMPLES]);
    let (serial, parallel) = snapshots(&gpu, &reference, &grouped);
    assert!(
        serial.normal_forces_n.iter().any(|&force| force > 0.0),
        "the stroke pressed no strand onto the ridges"
    );
    let modes: Vec<f32> = serial.anchors.iter().flatten().map(|a| a[3]).collect();
    assert!(modes.iter().any(|&w| w > 0.0), "no contact stuck");
    assert!(modes.iter().any(|&w| w < 0.0), "no contact slid");
    assert!(
        serial.plastic.iter().flatten().any(|p| *p != [0.0; 3]),
        "nothing set"
    );
    assert_same_strands(&serial, &parallel);
}

#[test]
fn the_grouped_solve_matches_the_serial_one_from_a_worn_brush() {
    let gpu = GpuContext::new().unwrap();
    let mut reference = on_ridges(&gpu, true);
    let mut grouped = on_ridges(&gpu, true);
    // Both wear the brush with the serial solve, so they start the next stroke alike.
    let wearing = line(CENTRE_Y, 60.0, 300.0, 0.9, TILT_DEG);
    drag(&gpu, &mut reference, &wearing);
    drag(&gpu, &mut grouped, &wearing);
    grouped.engine.use_reference_bristle_solve(false);
    let samples = line(CENTRE_Y, 300.0, 360.0, 0.6, TILT_DEG);
    drag(&gpu, &mut reference, &samples[..WORN_SAMPLES]);
    drag(&gpu, &mut grouped, &samples[..WORN_SAMPLES]);
    let (serial, parallel) = snapshots(&gpu, &reference, &grouped);
    assert!(serial.normal_forces_n.iter().any(|&force| force > 0.0));
    assert!(
        serial.plastic.iter().flatten().any(|p| *p != [0.0; 3]),
        "nothing set"
    );
    assert_same_strands(&serial, &parallel);
}

#[test]
fn the_grouped_solve_matches_the_serial_one_along_a_long_stroke_on_flat_canvas() {
    let gpu = GpuContext::new().unwrap();
    let mut reference = on_flat_canvas(&gpu, true);
    let mut grouped = on_flat_canvas(&gpu, false);
    for samples in [
        line(CENTRE_Y, 60.0, 300.0, 0.9, TILT_DEG),
        line(CENTRE_Y, 300.0, 360.0, 0.6, TILT_DEG),
    ] {
        drag(&gpu, &mut reference, &samples);
        drag(&gpu, &mut grouped, &samples);
    }
    let (serial, parallel) = snapshots(&gpu, &reference, &grouped);
    let modes: Vec<f32> = serial.anchors.iter().flatten().map(|a| a[3]).collect();
    assert!(modes.iter().any(|&w| w < 0.0), "no contact slid");
    assert_same_strands(&serial, &parallel);
}
