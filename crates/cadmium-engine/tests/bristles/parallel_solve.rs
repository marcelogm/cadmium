use cadmium_engine::canvas::StateTexture;
use cadmium_engine::engine::BristleSnapshot;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::support::{drag, frame, line, session};

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
/// The serial solve presses the fresh stroke in up to its first contacts, then the grouped
/// one takes over for two samples: the middle of the 1 to 3 that stay within tolerance.
const FIRST_CONTACT_SAMPLES: usize = 3;
const GROUPED_SAMPLES: usize = 2;
const WORN_SAMPLES: usize = 3;
/// They differ only where the compiler fuses a multiply-add differently.
const POSITION_TOLERANCE_PX: f32 = 1.0e-3;
const FORCE_TOLERANCE_SHARE: f32 = 1.0e-3;
const FORCE_FLOOR_N: f32 = 1.0e-9;
/// The anchors' loads are the force of an earlier iteration; measured worst 1.14e-3.
const ANCHOR_LOAD_TOLERANCE_SHARE: f32 = 2.0e-3;
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

/// Stick and slip over ridges magnify a last-bit difference within a few dabs: the serial
/// solve against itself, with the stroke nudged 1e-4 px along x, ends 2.6e-3 px apart after
/// 7 samples. One grouped dab from the serial state already differs by up to 4.2e-3 px
/// there, against 6.1e-4 before stuck contacts were held back onto their floor and anchor.
/// Over ridges the solves are compared over a handful of dabs from the same state.
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

#[derive(Debug, PartialEq)]
enum Mode {
    Free,
    Stuck,
    Sliding,
}

fn mode(w: f32) -> Mode {
    if w > 0.0 {
        Mode::Stuck
    } else if w < 0.0 {
        Mode::Sliding
    } else {
        Mode::Free
    }
}

fn assert_same_anchors(bristle: usize, serial: &[[f32; 4]], parallel: &[[f32; 4]]) {
    for (vertex, (x, y)) in serial.iter().zip(parallel).enumerate() {
        assert_eq!(
            mode(x[3]),
            mode(y[3]),
            "bristle {bristle} vertex {vertex}: modes {x:?} {y:?}"
        );
        let scale = x[3].abs().max(y[3].abs()).max(FORCE_FLOOR_N);
        assert!(
            (x[3].abs() - y[3].abs()).abs() <= ANCHOR_LOAD_TOLERANCE_SHARE * scale,
            "bristle {bristle} vertex {vertex}: loads {x:?} {y:?}"
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
    let mut grouped = on_ridges(&gpu, true);
    let mut samples = line(CENTRE_Y, 60.0, 140.0, 0.9, TILT_DEG);
    pressing_in(&mut samples);
    for s in [&mut reference, &mut grouped] {
        s.begin_stroke(samples[0]);
        s.add_samples(&samples[1..FIRST_CONTACT_SAMPLES]);
        frame(&gpu, s);
    }
    grouped.engine.use_reference_bristle_solve(false);
    for s in [&mut reference, &mut grouped] {
        s.add_samples(&samples[FIRST_CONTACT_SAMPLES..][..GROUPED_SAMPLES]);
        frame(&gpu, s);
    }
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
    // Without chaos no stuck contact is reachable on flat canvas.
    assert!(modes.iter().any(|&w| w < 0.0), "no contact slid");
    assert_same_strands(&serial, &parallel);
}

#[test]
fn the_grouped_solve_matches_the_serial_one_without_friction_over_ridges() {
    let gpu = GpuContext::new().unwrap();
    let mut reference = on_ridges(&gpu, true);
    let mut grouped = on_ridges(&gpu, false);
    // Plasticity off too: with it, creep still makes the strokes diverge by 1e-2 px.
    for s in [&mut reference, &mut grouped] {
        s.engine.use_frictionless_bristles(true);
        s.brush.hair.bend_memory = 0.0;
    }
    let samples = line(CENTRE_Y, 60.0, 300.0, 0.9, TILT_DEG);
    drag(&gpu, &mut reference, &samples);
    drag(&gpu, &mut grouped, &samples);
    let (serial, parallel) = snapshots(&gpu, &reference, &grouped);
    assert!(serial.normal_forces_n.iter().any(|&force| force > 0.0));
    for anchors in [&serial.anchors, &parallel.anchors] {
        assert!(
            anchors.iter().flatten().all(|a| *a == [0.0; 4]),
            "friction anchored a vertex"
        );
    }
    assert_same_strands(&serial, &parallel);
}
