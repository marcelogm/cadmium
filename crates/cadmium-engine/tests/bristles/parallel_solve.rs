use cadmium_engine::canvas::StateTexture;
use cadmium_engine::engine::BristleSnapshot;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;

use crate::support::{drag, line, session};

const WIDTH: usize = 512;
const HEIGHT: usize = 256;
const CENTRE_Y: f64 = 128.0;
const RIDGE_PERIOD_PX: usize = 16;
const RIDGE_WIDTH_PX: usize = 6;
const RIDGE_HEIGHT_UM: f32 = 1200.0;
const TILT_DEG: f32 = 20.0;
/// They differ only where the compiler fuses a multiply-add differently.
const POSITION_TOLERANCE_PX: f32 = 1.0e-3;
const FORCE_TOLERANCE_SHARE: f32 = 1.0e-3;
const FORCE_FLOOR_N: f32 = 1.0e-9;

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
    // Short, so a last-bit difference has few dabs to grow over.
    let samples = line(CENTRE_Y, 60.0, 140.0, 0.9, TILT_DEG);
    drag(&gpu, &mut reference, &samples);
    drag(&gpu, &mut grouped, &samples);
    let (serial, parallel) = snapshots(&gpu, &reference, &grouped);
    assert!(
        serial.normal_forces_n.iter().any(|&force| force > 0.0),
        "the stroke pressed no strand onto the ridges"
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
    drag(&gpu, &mut reference, &samples);
    drag(&gpu, &mut grouped, &samples);
    let (serial, parallel) = snapshots(&gpu, &reference, &grouped);
    assert!(serial.normal_forces_n.iter().any(|&force| force > 0.0));
    assert_same_strands(&serial, &parallel);
}
