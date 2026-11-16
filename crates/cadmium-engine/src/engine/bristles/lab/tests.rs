use super::*;
use crate::mechanics::{cantilever_tip_deflection, metres_from_px};

const LENGTH_PX: f32 = 30.0;
/// EI of the test strand, N·m²: about three 60 µm hairs at 4 GPa.
const STIFFNESS_N_M2: f64 = 7.6e-9;
const BEAM_TOLERANCE: f64 = 0.05;
const FLOOR_FAR_BELOW_PX: f32 = -1.0e4;
const SEGMENTS: usize = BRISTLE_VERTICES - 1;

fn segment_px() -> f32 {
    LENGTH_PX / SEGMENTS as f32
}

/// Uniform compliances, rad per N·m, with the clamp spanning half a segment.
fn uniform_compliances() -> [f32; 8] {
    let segment_m = metres_from_px(f64::from(segment_px()));
    std::array::from_fn(|hinge| match hinge {
        0 => (0.5 * segment_m / STIFFNESS_N_M2) as f32,
        h if h < SEGMENTS => (segment_m / STIFFNESS_N_M2) as f32,
        _ => 0.0,
    })
}

fn strand(clamp: [f32; 3], root: [f32; 3]) -> LabStrand {
    LabStrand::new(root, clamp, segment_px(), uniform_compliances())
}

fn tilted(degrees_below: f32) -> [f32; 3] {
    let angle = degrees_below.to_radians();
    [angle.cos(), 0.0, -angle.sin()]
}

#[test]
fn a_tip_load_bends_the_strand_as_a_cantilever() {
    let gpu = GpuContext::new().unwrap();
    let want_px = 0.5;
    let length_m = metres_from_px(f64::from(LENGTH_PX));
    let force_n = 3.0 * STIFFNESS_N_M2 * metres_from_px(want_px) / length_m.powi(3);
    let loaded = strand([1.0, 0.0, 0.0], [0.0, 0.0, 0.0])
        .with_floor(FLOOR_FAR_BELOW_PX)
        .with_tip_force([0.0, 0.0, -(force_n as f32)])
        .with_press(0.0, 4);
    let run = &run(&gpu, &[loaded])[0];
    let got_px = -f64::from(run.chain_px[SEGMENTS][2]);
    let exact_px =
        cantilever_tip_deflection(force_n, length_m, STIFFNESS_N_M2) / metres_from_px(1.0);
    assert!(
        (got_px / exact_px - 1.0).abs() <= BEAM_TOLERANCE,
        "{got_px} px vs {exact_px} px"
    );
}

/// The rest strand at `degrees` below horizontal with its tip on the floor at z = 0,
/// pressed `press_px` down in `steps`.
fn pressed_tilted(degrees: f32, press_px: f32, steps: u32) -> LabStrand {
    let clamp = tilted(degrees);
    let rise = -clamp[2] * LENGTH_PX;
    strand(clamp, [0.0, 0.0, rise])
        .with_floor(0.0)
        .with_press(press_px, steps)
}

#[test]
fn a_pressed_tilted_strand_pushes_back_as_a_beam() {
    let gpu = GpuContext::new().unwrap();
    let degrees = 20.0_f32;
    let press_px = 0.3;
    let run = &run(&gpu, &[pressed_tilted(degrees, press_px, 1)])[0];
    let length_m = metres_from_px(f64::from(LENGTH_PX));
    let cosine = f64::from(degrees.to_radians().cos());
    // Tip load N on a beam at angle θ: the tip drops Δz = N·L³·cos²θ / (3EI).
    let exact_n = 3.0 * STIFFNESS_N_M2 * metres_from_px(f64::from(press_px))
        / (length_m.powi(3) * cosine * cosine);
    let got_n = f64::from(run.normal_force_n);
    assert!(
        (got_n / exact_n - 1.0).abs() <= BEAM_TOLERANCE,
        "{got_n} N vs {exact_n} N"
    );
}

fn tip_reach_px(run: &LabRun) -> f32 {
    let tip = run.chain_px[SEGMENTS];
    tip[0].hypot(tip[1])
}

#[test]
fn an_upright_strand_pressed_down_buckles_wider_and_never_sinks() {
    let gpu = GpuContext::new().unwrap();
    let presses = [5.0, 10.0, 15.0];
    let strands: Vec<LabStrand> = presses
        .iter()
        .map(|&press| {
            strand([0.0, 0.0, -1.0], [0.0, 0.0, LENGTH_PX])
                .with_curl([0.0, 0.02, 0.0])
                .with_floor(0.0)
                .with_press(press, 60)
        })
        .collect();
    let runs = run(&gpu, &strands);
    for run in &runs {
        assert!(
            run.chain_px.iter().all(|at| at[2] >= -1e-3),
            "{:?}",
            run.chain_px
        );
        for pair in run.chain_px.windows(2) {
            let span = (0..3)
                .map(|a| (pair[1][a] - pair[0][a]).powi(2))
                .sum::<f32>()
                .sqrt();
            assert!((span - segment_px()).abs() < 1e-3, "segment {span}");
        }
    }
    let reaches: Vec<f32> = runs.iter().map(tip_reach_px).collect();
    assert!(
        reaches.windows(2).all(|pair| pair[1] > pair[0]),
        "{reaches:?}"
    );
}

#[test]
fn pressing_in_twice_the_steps_ends_in_the_same_shape() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(
        &gpu,
        &[pressed_tilted(45.0, 3.0, 4), pressed_tilted(45.0, 3.0, 8)],
    );
    for (a, b) in runs[0].chain_px.iter().zip(&runs[1].chain_px) {
        let gap = (0..3)
            .map(|axis| (a[axis] - b[axis]).abs())
            .fold(0.0, f32::max);
        assert!(gap < 0.05, "{a:?} vs {b:?}");
    }
}
