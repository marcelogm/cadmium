use super::*;
use crate::mechanics::{cantilever_tip_deflection, euler_buckling_load, metres_from_px};

const LENGTH_PX: f32 = 30.0;
/// EI of the test strand, N·m²: about three 60 µm hairs at 4 GPa.
const STIFFNESS_N_M2: f64 = 7.6e-9;
const BEAM_TOLERANCE: f64 = 0.05;
const FLOOR_FAR_BELOW_PX: f32 = -1.0e4;
const SEGMENTS: usize = BRISTLE_VERTICES - 1;
const ALONG_X: [f32; 3] = [1.0, 0.0, 0.0];
const STRAIGHT_DOWN: [f32; 3] = [0.0, 0.0, -1.0];
const AT_ORIGIN: [f32; 3] = [0.0, 0.0, 0.0];
/// How far under its floor a vertex may end, px: the contact tolerance of the solve.
const SINK_TOLERANCE_PX: f32 = 1e-3;
/// How far a rebuilt segment may stray from its length, px.
const LENGTH_TOLERANCE_PX: f32 = 1e-3;

/// The cantilever's target tip drop, px.
const CANTILEVER_DROP_PX: f64 = 0.5;
const CANTILEVER_STEPS: u32 = 4;

const TILT_DEGREES: f32 = 20.0;
const TILT_PRESS_PX: f32 = 0.3;

/// The rest curl of the upright strand, rad, about y: `REST_CURL_RAD` of the layout.
const UPRIGHT_CURL: [f32; 3] = [0.0, 0.02, 0.0];
const UPRIGHT_PRESSES_PX: [f32; 3] = [5.0, 10.0, 15.0];
const UPRIGHT_STEPS: u32 = 60;
/// A curl small enough that the strand is a near-perfect column.
const NEAR_STRAIGHT_CURL: [f32; 3] = [0.0, 0.001, 0.0];
/// Presses just past the onset of buckling, px: end shortening of a few percent, where the
/// elastica's load is still within 2% of Euler's.
const ONSET_PRESSES_PX: [f32; 2] = [1.0, 2.0];
const EULER_TOLERANCE: f64 = 0.05;

const DEGENERATE_PRESS_PX: f32 = 5.0;
/// Bound on the force of a strand with no lever, in Euler loads.
const DEGENERATE_EULER_LOADS: f64 = 10.0;

const STEPPED_DEGREES: f32 = 45.0;
const STEPPED_PRESS_PX: f32 = 3.0;
const FEW_STEPS: u32 = 4;
const MANY_STEPS: u32 = 8;
const SAME_SHAPE_PX: f32 = 0.05;

const DEEP_DEGREES: f32 = 45.0;
const DEEP_PRESS_PX: f32 = 15.0;
const DEEP_STEPS: u32 = 30;
const MIN_LANDED_VERTICES: usize = 2;

/// A floor plane rising 45° toward +x through the upright strand's tip.
const RAMP_SLOPE: [f32; 2] = [1.0, 0.0];
const RAMP_PRESS_PX: f32 = 5.0;
const RAMP_STEPS: u32 = 20;
/// How far down the ramp the pressed tip must slide at least, px.
const MIN_RAMP_SLIDE_PX: f32 = 1.0;

fn segment_px() -> f32 {
    LENGTH_PX / SEGMENTS as f32
}

fn length_m() -> f64 {
    metres_from_px(f64::from(LENGTH_PX))
}

/// Uniform compliances, rad per N·m, with the clamp spanning half a segment.
fn uniform_compliances() -> [f32; LAB_HINGE_SLOTS] {
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

fn span_px(from: [f32; 3], to: [f32; 3]) -> f32 {
    (0..3)
        .map(|a| (to[a] - from[a]).powi(2))
        .sum::<f32>()
        .sqrt()
}

fn assert_lengths_kept(run: &LabRun) {
    for pair in run.chain_px.windows(2) {
        let span = span_px(pair[0], pair[1]);
        assert!(
            (span - segment_px()).abs() < LENGTH_TOLERANCE_PX,
            "segment {span}"
        );
    }
}

fn assert_above_floor(run: &LabRun) {
    assert!(
        run.chain_px.iter().all(|at| at[2] >= -SINK_TOLERANCE_PX),
        "{:?}",
        run.chain_px
    );
}

#[test]
fn a_tip_load_bends_the_strand_as_a_cantilever() {
    let gpu = GpuContext::new().unwrap();
    let force_n = 3.0 * STIFFNESS_N_M2 * metres_from_px(CANTILEVER_DROP_PX) / length_m().powi(3);
    let loaded = strand(ALONG_X, AT_ORIGIN)
        .with_floor(FLOOR_FAR_BELOW_PX)
        .with_tip_force([0.0, 0.0, -(force_n as f32)])
        .with_press(0.0, CANTILEVER_STEPS);
    let run = &run(&gpu, &[loaded])[0];
    let got_px = -f64::from(run.chain_px[SEGMENTS][2]);
    let exact_px =
        cantilever_tip_deflection(force_n, length_m(), STIFFNESS_N_M2) / metres_from_px(1.0);
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
    let run = &run(&gpu, &[pressed_tilted(TILT_DEGREES, TILT_PRESS_PX, 1)])[0];
    let cosine = f64::from(TILT_DEGREES.to_radians().cos());
    // Tip load N on a beam at angle θ: the tip drops Δz = N·L³·cos²θ / (3EI).
    let exact_n = 3.0 * STIFFNESS_N_M2 * metres_from_px(f64::from(TILT_PRESS_PX))
        / (length_m().powi(3) * cosine * cosine);
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

fn upright(curl: [f32; 3], press_px: f32) -> LabStrand {
    strand(STRAIGHT_DOWN, [0.0, 0.0, LENGTH_PX])
        .with_curl(curl)
        .with_floor(0.0)
        .with_press(press_px, UPRIGHT_STEPS)
}

#[test]
fn an_upright_strand_pressed_down_buckles_wider_and_never_sinks() {
    let gpu = GpuContext::new().unwrap();
    let strands: Vec<LabStrand> = UPRIGHT_PRESSES_PX
        .iter()
        .map(|&press| upright(UPRIGHT_CURL, press))
        .collect();
    let runs = run(&gpu, &strands);
    for run in &runs {
        assert_above_floor(run);
        assert_lengths_kept(run);
    }
    let reaches: Vec<f32> = runs.iter().map(tip_reach_px).collect();
    assert!(
        reaches.windows(2).all(|pair| pair[1] > pair[0]),
        "{reaches:?}"
    );
    // Past buckling the elastica stiffens: its load rises with the press.
    let forces: Vec<f32> = runs.iter().map(|run| run.normal_force_n).collect();
    assert!(
        forces.windows(2).all(|pair| pair[1] > pair[0]),
        "{forces:?}"
    );
}

/// A near-straight column pressed just past buckling carries the Euler load. The rest
/// curl of the app is an imperfection that lowers it, and deeper presses raise it along
/// the elastica, so both stay out of this check.
#[test]
fn a_near_straight_strand_buckles_at_the_euler_load() {
    let gpu = GpuContext::new().unwrap();
    let strands: Vec<LabStrand> = ONSET_PRESSES_PX
        .iter()
        .map(|&press| upright(NEAR_STRAIGHT_CURL, press))
        .collect();
    let euler_n = euler_buckling_load(STIFFNESS_N_M2, length_m());
    for run in &run(&gpu, &strands) {
        let got_n = f64::from(run.normal_force_n);
        assert!(
            (got_n / euler_n - 1.0).abs() <= EULER_TOLERANCE,
            "{got_n} N vs {euler_n} N"
        );
    }
}

/// A straight upright strand has no lever: no force can lift it by bending, so it sinks
/// into the floor (allowed here, and only here). What it must not do is answer with an
/// unbounded force.
#[test]
fn a_straight_upright_strand_keeps_a_bounded_force() {
    let gpu = GpuContext::new().unwrap();
    let run = &run(&gpu, &[upright([0.0; 3], DEGENERATE_PRESS_PX)])[0];
    let euler_n = euler_buckling_load(STIFFNESS_N_M2, length_m());
    let got_n = f64::from(run.normal_force_n);
    assert!(
        got_n.is_finite() && got_n < DEGENERATE_EULER_LOADS * euler_n,
        "{got_n} N vs {euler_n} N"
    );
    assert!(run.chain_px.iter().flatten().all(|v| v.is_finite()));
    assert_lengths_kept(run);
}

#[test]
fn a_deep_press_lands_several_vertices_and_pushes_each_up() {
    let gpu = GpuContext::new().unwrap();
    let run = &run(
        &gpu,
        &[pressed_tilted(DEEP_DEGREES, DEEP_PRESS_PX, DEEP_STEPS)],
    )[0];
    assert_above_floor(run);
    assert_lengths_kept(run);
    assert!(
        run.vertex_forces_n.iter().all(|&force| force >= 0.0),
        "{:?}",
        run.vertex_forces_n
    );
    let landed = run.vertex_forces_n.iter().filter(|&&f| f > 0.0).count();
    assert!(
        landed >= MIN_LANDED_VERTICES,
        "{:?} {:?}",
        run.vertex_forces_n,
        run.chain_px
    );
}

#[test]
fn pressing_in_twice_the_steps_ends_in_the_same_shape() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(
        &gpu,
        &[
            pressed_tilted(STEPPED_DEGREES, STEPPED_PRESS_PX, FEW_STEPS),
            pressed_tilted(STEPPED_DEGREES, STEPPED_PRESS_PX, MANY_STEPS),
        ],
    );
    for (a, b) in runs[0].chain_px.iter().zip(&runs[1].chain_px) {
        let gap = (0..3)
            .map(|axis| (a[axis] - b[axis]).abs())
            .fold(0.0, f32::max);
        assert!(gap < SAME_SHAPE_PX, "{a:?} vs {b:?}");
    }
}

#[test]
fn a_strand_pressed_onto_a_ramp_slides_down_it_and_never_sinks_into_it() {
    let gpu = GpuContext::new().unwrap();
    let pressed = strand(STRAIGHT_DOWN, [0.0, 0.0, LENGTH_PX])
        .with_floor(0.0)
        .with_floor_slope(RAMP_SLOPE)
        .with_press(RAMP_PRESS_PX, RAMP_STEPS);
    let run = &run(&gpu, &[pressed])[0];
    assert_lengths_kept(run);
    let over_ramp = |at: &[f32; 3]| at[2] - (RAMP_SLOPE[0] * at[0] + RAMP_SLOPE[1] * at[1]);
    assert!(
        run.chain_px
            .iter()
            .all(|at| over_ramp(at) >= -SINK_TOLERANCE_PX),
        "{:?}",
        run.chain_px
    );
    assert!(run.normal_force_n > 0.0, "{}", run.normal_force_n);
    let tip = run.chain_px[SEGMENTS];
    assert!(
        tip[0] <= -MIN_RAMP_SLIDE_PX && over_ramp(&tip) <= SINK_TOLERANCE_PX,
        "the tip {tip:?} did not slide down the ramp"
    );
}
