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
/// the drag.
const HELD_PX: f32 = 0.05 * SMALL_DRAG_PX;
const COEFFICIENT_TOLERANCE: f32 = 0.02;
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
    assert!(xy_span(held.chain_px[TIP], dragged.chain_px[TIP]) > HELD_PX);
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
        highest <= STATIC_MU * (1.0 + 1e-3),
        "a stuck tip held {highest} of its load"
    );
    assert!(
        highest >= STATIC_MU * (1.0 - 10.0 * COEFFICIENT_TOLERANCE),
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
            ratio <= STATIC_MU * (1.0 + 1e-3),
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
