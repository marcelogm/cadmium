use super::*;

const STATIC_MU: f32 = 0.5;
const KINETIC_MU: f32 = 0.3;
const SLANT_DEGREES: f32 = 45.0;
const PRESS_PX: f32 = 1.0;
const PRESS_STEPS: u32 = 10;
const SMALL_DRAG_PX: f32 = 0.1;
const LONG_DRAG_PX: f32 = 3.0;
const DRAG_STEPS: u32 = 30;
/// A held vertex creeps by the contact's shear compliance, about `TANGENT_RIDGE_SHARE` of
/// the drive; the creep piles up on one anchor over a drag and was measured under 2e-3 px.
const HELD_PX: f32 = 0.05 * SMALL_DRAG_PX;
const COEFFICIENT_TOLERANCE: f32 = 0.02;
/// The cone test and the friction it bounds are the same solve's, so a held friction can
/// pass μ·fₙ only by rounding.
const CONE_SLACK: f32 = 1e-3;
/// Below the cone, the highest stuck run of the sweep may sit one drag step short of the
/// let-go point: a step raises the held friction by up to this share of μₛ.
const LET_GO_RESOLUTION: f32 = 10.0 * COEFFICIENT_TOLERANCE;
/// Raised this far after the press, the tip leaves the canvas.
const LIFT_PX: f32 = 2.0 * PRESS_PX;
const SETTLE_STEPS: u32 = 5;
/// Axial drags of a deep press: the upper of two held vertices slides, the lower holds.
const AXIAL_DRAGS_PX: [f32; 3] = [0.02, 0.05, -0.02];
const THRESHOLD_RUNS: usize = 60;
const THRESHOLD_STEP_PX: f32 = 0.005;
const ANISOTROPY: f32 = 0.5;
const DEEP_PRESS_PX: f32 = 15.0;
const DEEP_PRESS_STEPS: u32 = 30;
const SAME_PATH_PX: f32 = 0.05;
const TIP: usize = BRISTLE_VERTICES - 1;
/// Hold tests drag sideways, where bending holds the tip; along the strand (x) it is
/// inextensible, so even a small axial drag crosses the cone and slides it at once.
const SIDEWAYS: [f32; 2] = [0.0, 1.0];
/// App strands carry a rest curl of about 0.02 rad per hinge; a straight strand is the
/// degenerate column, which has no lever along its axis.
const REST_CURL: [f32; 3] = [0.0, -0.02, 0.0];

/// The rest tip's height of a strand from `clamp` curled by `REST_CURL` about y, px.
fn rest_tip_z(clamp: [f32; 3]) -> f32 {
    let angle = REST_CURL[1];
    let (mut x, mut z) = (clamp[0], clamp[2]);
    let mut tip_z = 0.0;
    for edge in 0..SEGMENTS {
        if edge > 0 {
            (x, z) = (
                x * angle.cos() + z * angle.sin(),
                z * angle.cos() - x * angle.sin(),
            );
        }
        tip_z += z * segment_px();
    }
    tip_z
}

/// A curled strand slanted toward +x, its tip resting on the floor, then pressed.
fn slanted(press_px: f32, steps: u32) -> LabStrand {
    let clamp = tilted(SLANT_DEGREES);
    strand(clamp, AT_ORIGIN)
        .with_curl(REST_CURL)
        .with_floor(rest_tip_z(clamp))
        .with_press(press_px, steps)
        .with_friction(STATIC_MU, KINETIC_MU, 0.0)
}

fn pressed() -> LabStrand {
    slanted(PRESS_PX, PRESS_STEPS)
}

fn length3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn tip_ratio(run: &LabRun) -> f32 {
    length3(run.friction_n[TIP]) / run.vertex_forces_n[TIP]
}

fn sideways(px: f32) -> [f32; 2] {
    [SIDEWAYS[0] * px, SIDEWAYS[1] * px]
}

fn xy_span(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

#[test]
fn a_tip_dragged_less_than_its_cone_allows_stays_on_its_anchor() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(
        &gpu,
        &[
            pressed(),
            pressed().with_drag(sideways(SMALL_DRAG_PX), DRAG_STEPS),
        ],
    );
    let (held, dragged) = (&runs[0], &runs[1]);
    assert!(
        held.vertex_forces_n[TIP] > 0.0,
        "the press put no load on the tip"
    );
    let moved = xy_span(held.chain_px[TIP], dragged.chain_px[TIP]);
    assert!(moved < HELD_PX, "the stuck tip moved {moved} px");
    assert!(dragged.anchor_modes[TIP] > 0.0, "the tip is not stuck");
    assert!(
        length3(dragged.friction_n[TIP]) > 0.0,
        "the stuck tip holds no friction"
    );
    assert!(
        tip_ratio(dragged) <= STATIC_MU,
        "ratio {}",
        tip_ratio(dragged)
    );
}

#[test]
fn a_tip_dragged_far_slides_at_the_kinetic_coefficient() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(
        &gpu,
        &[
            pressed(),
            pressed().with_drag([LONG_DRAG_PX, 0.0], DRAG_STEPS),
        ],
    );
    let (held, dragged) = (&runs[0], &runs[1]);
    assert!(dragged.anchor_modes[TIP] < 0.0, "the tip is not sliding");
    assert!(xy_span(held.chain_px[TIP], dragged.chain_px[TIP]) > 0.5 * LONG_DRAG_PX);
    let ratio = tip_ratio(dragged);
    assert!(
        (ratio - KINETIC_MU).abs() <= COEFFICIENT_TOLERANCE * KINETIC_MU,
        "the sliding tip's friction is {ratio} of its load"
    );
}

#[test]
fn a_tip_lets_go_at_the_static_coefficient() {
    let gpu = GpuContext::new().unwrap();
    let strands: Vec<LabStrand> = (1..=THRESHOLD_RUNS)
        .map(|run| pressed().with_drag(sideways(run as f32 * THRESHOLD_STEP_PX), 1))
        .collect();
    let runs = run(&gpu, &strands);
    let stuck: Vec<f32> = runs
        .iter()
        .filter(|r| r.anchor_modes[TIP] > 0.0)
        .map(tip_ratio)
        .collect();
    assert!(
        !stuck.is_empty() && stuck.len() < runs.len(),
        "no run crossed the cone"
    );
    let highest = stuck.iter().copied().fold(0.0, f32::max);
    assert!(
        highest <= STATIC_MU * (1.0 + CONE_SLACK),
        "a stuck tip held {highest} of its load"
    );
    assert!(
        highest >= STATIC_MU * (1.0 - LET_GO_RESOLUTION),
        "the tip let go at {highest} of its load"
    );
}

/// The tip's pull direction points to the root, along −x; dragging toward −x pulls it.
#[test]
fn a_pulled_tip_slides_more_easily_than_a_pushed_one() {
    let gpu = GpuContext::new().unwrap();
    let drag = |sign: f32| {
        pressed()
            .with_friction(STATIC_MU, KINETIC_MU, ANISOTROPY)
            .with_drag([sign * LONG_DRAG_PX, 0.0], DRAG_STEPS)
    };
    let runs = run(&gpu, &[drag(-1.0), drag(1.0)]);
    let (pulled, pushed) = (tip_ratio(&runs[0]), tip_ratio(&runs[1]));
    let expected_pulled = KINETIC_MU * (1.0 - ANISOTROPY);
    assert!(
        (pulled - expected_pulled).abs() <= COEFFICIENT_TOLERANCE * expected_pulled,
        "pulled: {pulled} of the load, expected {expected_pulled}"
    );
    assert!(
        (pushed - KINETIC_MU).abs() <= COEFFICIENT_TOLERANCE * KINETIC_MU,
        "pushed: {pushed} of the load"
    );
}

#[test]
fn two_stuck_contacts_both_hold_their_anchors() {
    let gpu = GpuContext::new().unwrap();
    let deep = || slanted(DEEP_PRESS_PX, DEEP_PRESS_STEPS);
    let runs = run(
        &gpu,
        &[
            deep(),
            deep().with_drag(sideways(SMALL_DRAG_PX), DRAG_STEPS),
        ],
    );
    let (held, dragged) = (&runs[0], &runs[1]);
    let stuck: Vec<usize> = (1..BRISTLE_VERTICES)
        .filter(|&v| dragged.anchor_modes[v] > 0.0)
        .collect();
    assert!(stuck.len() >= 2, "only {stuck:?} stuck");
    for v in stuck {
        let moved = xy_span(held.chain_px[v], dragged.chain_px[v]);
        assert!(moved < HELD_PX, "vertex {v} moved {moved} px");
        let ratio = length3(dragged.friction_n[v]) / dragged.vertex_forces_n[v];
        assert!(
            ratio <= STATIC_MU * (1.0 + CONE_SLACK),
            "vertex {v} holds {ratio}"
        );
    }
}

#[test]
fn without_friction_a_dragged_tip_slides_along() {
    let gpu = GpuContext::new().unwrap();
    let frictionless = || pressed().with_friction(0.0, 0.0, 0.0);
    let runs = run(
        &gpu,
        &[
            frictionless(),
            frictionless().with_drag([SMALL_DRAG_PX, 0.0], DRAG_STEPS),
        ],
    );
    assert!(xy_span(runs[0].chain_px[TIP], runs[1].chain_px[TIP]) > 0.5 * SMALL_DRAG_PX);
    assert_eq!(runs[1].anchor_modes[TIP], 0.0);
    assert_eq!(runs[1].friction_n[TIP], [0.0; 3]);
}

#[test]
fn a_drag_in_steps_half_as_long_ends_in_the_same_place() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(
        &gpu,
        &[
            pressed().with_drag([LONG_DRAG_PX, 0.0], DRAG_STEPS),
            pressed().with_drag([LONG_DRAG_PX, 0.0], 2 * DRAG_STEPS),
        ],
    );
    for (a, b) in runs[0].chain_px.iter().zip(&runs[1].chain_px) {
        assert!(span_px(*a, *b) < SAME_PATH_PX, "{a:?} against {b:?}");
    }
}

#[test]
fn a_sliding_tip_sticks_again_once_the_drag_stops() {
    let gpu = GpuContext::new().unwrap();
    let stopped = pressed()
        .with_drag([LONG_DRAG_PX, 0.0], DRAG_STEPS)
        .with_settle(0.0, SETTLE_STEPS);
    let run = &run(&gpu, &[stopped])[0];
    assert!(run.anchor_modes[TIP] > 0.0, "the tip is still sliding");
    let ratio = tip_ratio(run);
    assert!(
        ratio <= KINETIC_MU * (1.0 + CONE_SLACK),
        "the tip sticks again holding {ratio} of its load"
    );
}

#[test]
fn a_held_tip_lifted_off_keeps_no_friction_and_no_anchor() {
    let gpu = GpuContext::new().unwrap();
    let run = &run(&gpu, &[pressed().with_settle(LIFT_PX, SETTLE_STEPS)])[0];
    assert_eq!(run.vertex_forces_n[TIP], 0.0);
    assert_eq!(run.friction_n[TIP], [0.0; 3]);
    assert_eq!(run.anchor_modes[TIP], 0.0);
}

/// The lower of two held neighbours has no toward-root row; once the upper one slides it
/// must take it back, or it drifts along the strand while it reports stuck.
#[test]
fn a_held_vertex_stays_on_its_anchor_when_the_one_above_slides() {
    let gpu = GpuContext::new().unwrap();
    let strands: Vec<LabStrand> = AXIAL_DRAGS_PX
        .iter()
        .map(|&px| slanted(DEEP_PRESS_PX, DEEP_PRESS_STEPS).with_drag([px, 0.0], DRAG_STEPS))
        .collect();
    let mut upper_slid = false;
    for run in run(&gpu, &strands) {
        for v in 1..BRISTLE_VERTICES - 1 {
            upper_slid |= run.anchor_modes[v] < 0.0 && run.anchor_modes[v + 1] > 0.0;
        }
        for v in (1..BRISTLE_VERTICES).filter(|&v| run.anchor_modes[v] > 0.0) {
            let off = xy_span(run.chain_px[v], run.anchors_px[v]);
            assert!(
                off <= HELD_PX,
                "stuck vertex {v} is {off} px off its anchor"
            );
        }
    }
    assert!(upper_slid, "no upper neighbour slid over a held one");
}

/// A rest curl about x tilts the strand out of the plane it is pressed in: the solve then
/// turns hinges about their own edges, which the next step's bend no longer sees, so the
/// step stays as large however long the pose is held.
const OUT_OF_PLANE_CURLS: [[f32; 3]; 2] = [[0.08, 0.0, 0.0], [0.04, 0.02, 0.0]];
const OUT_OF_PLANE_DEGREES: f32 = 60.0;
const OUT_OF_PLANE_PRESS_PX: f32 = 8.0;
const HELD_STILL_STEPS: u32 = 10;
/// The floor sits this share of the strand's length along its clamp below the root.
const FLOOR_REACH_SHARE: f32 = 26.0 / 30.0;
/// The contact tolerance of the solve, px.
const ON_FLOOR_PX: f32 = 1e-3;

#[test]
fn a_held_tip_curled_out_of_its_plane_rests_on_its_floor_and_its_anchor() {
    let gpu = GpuContext::new().unwrap();
    let clamp = tilted(OUT_OF_PLANE_DEGREES);
    let floor_px = FLOOR_REACH_SHARE * LENGTH_PX * clamp[2];
    let strands: Vec<LabStrand> = OUT_OF_PLANE_CURLS
        .iter()
        .map(|&curl| {
            strand(clamp, AT_ORIGIN)
                .with_curl(curl)
                .with_floor(floor_px)
                .with_press(OUT_OF_PLANE_PRESS_PX, PRESS_STEPS)
                .with_friction(STATIC_MU, KINETIC_MU, 0.0)
                .with_settle(0.0, HELD_STILL_STEPS)
        })
        .collect();
    for (curl, run) in OUT_OF_PLANE_CURLS.iter().zip(run(&gpu, &strands)) {
        let engaged: Vec<usize> = (1..BRISTLE_VERTICES)
            .filter(|&v| run.vertex_forces_n[v] > 0.0)
            .collect();
        assert!(!engaged.is_empty(), "curl {curl:?}: nothing touches");
        for v in engaged {
            let clearance = run.chain_px[v][2] - floor_px;
            assert!(
                clearance.abs() <= ON_FLOOR_PX,
                "curl {curl:?}: pressed vertex {v} stands {clearance} px off its floor"
            );
        }
        for v in (1..BRISTLE_VERTICES).filter(|&v| run.anchor_modes[v] > 0.0) {
            let off = xy_span(run.chain_px[v], run.anchors_px[v]);
            assert!(
                off <= ON_FLOOR_PX,
                "curl {curl:?}: stuck vertex {v} is {off} px off its anchor"
            );
        }
    }
}
