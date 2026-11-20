use super::*;

const SLANT_DEGREES: f32 = 30.0;
/// Bends hinges 0–4 of the test strand past the 0.02 rad yield, the most bent about 0.05 rad.
const HOLD_FORCE_N: f32 = 2.85e-3;
const CREEP_PER_S: f32 = 10.0;
const STEP_S: f32 = 0.01;
const HOLD_STEPS: u32 = 5;
const LONG_HOLD_STEPS: u32 = 400;
const RELEASE_STEPS: u32 = 10;
const YIELD_RAD: f32 = 0.02;
const CAP_RAD: f32 = 0.1;
const CREEP_TOLERANCE: f32 = 0.05;
/// The closed form is checked only this far past the yield, where e/(e − yield) stays small.
const CLOSED_FORM_YIELDS: f32 = 1.5;
/// The shader's creep is exact in exact arithmetic; f32 summing over a few steps stays far
/// inside half a percent.
const CREEP_LAW_TOLERANCE: f32 = 0.005;
/// A twentieth of the yield: a released strand must bend far less than it would past the yield.
const RESTED_RAD: f32 = 1e-3;
/// The bend is planar, so the bound is nearly tight (measured 0.99).
const STAYS_BENT_SHARE: f32 = 0.9;
const STILL_STEPS: u32 = 5;

fn held(steps: u32) -> LabStrand {
    strand(tilted(SLANT_DEGREES), AT_ORIGIN)
        .with_floor(FLOOR_FAR_BELOW_PX)
        .with_tip_force([0.0, 0.0, -HOLD_FORCE_N])
        .with_press(0.0, steps)
        .with_creep(CREEP_PER_S, STEP_S)
}

fn length3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// Σ over the steps relies on a planar bend: every step's excess points the same way, so the
/// shift's length is the sum of the steps' shares.
#[test]
fn a_held_bend_creeps_by_its_excess_over_the_yield_each_step() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(&gpu, &[held(HOLD_STEPS)]);
    let share = CREEP_PER_S * STEP_S;
    for (hinge, (&summed, shift)) in runs[0]
        .crept_excess_rad
        .iter()
        .zip(&runs[0].plastic)
        .enumerate()
    {
        let expected = share * summed;
        let got = length3(*shift);
        assert!(got < CAP_RAD, "hinge {hinge} reached the cap");
        if summed == 0.0 {
            assert_eq!(got, 0.0, "hinge {hinge} crept under the yield");
        } else {
            assert!(
                (got - expected).abs() <= CREEP_LAW_TOLERANCE * expected,
                "hinge {hinge}: shift {got}, expected {expected}"
            );
        }
    }
}

#[test]
fn a_held_bend_creeps_the_rest_shape_past_the_yield() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(&gpu, &[held(HOLD_STEPS)]);
    let share = CREEP_PER_S * STEP_S;
    let mut asserted = 0;
    for (hinge, (&excess, shift)) in runs[0].excess_rad.iter().zip(&runs[0].plastic).enumerate() {
        if excess < CLOSED_FORM_YIELDS * YIELD_RAD {
            continue;
        }
        // The tip load's lever shrinks as the rest shape sets, so the excess drifts about
        // 0.5% a step while the closed form holds it constant.
        let expected = HOLD_STEPS as f32 * share * (excess - YIELD_RAD);
        let got = length3(*shift);
        asserted += 1;
        assert!(
            (got - expected).abs() <= CREEP_TOLERANCE * expected,
            "hinge {hinge}: shift {got}, expected {expected}"
        );
    }
    assert!(asserted > 0, "no hinge was bent far enough past the yield");
}

#[test]
fn a_bend_held_for_no_time_does_not_creep_further() {
    let gpu = GpuContext::new().unwrap();
    let crept = held(HOLD_STEPS);
    let then_still = held(HOLD_STEPS)
        .with_drag([0.0, 0.0], STILL_STEPS)
        .with_drag_seconds(0.0);
    let runs = run(&gpu, &[crept, then_still]);
    assert!(runs[0].plastic.iter().any(|shift| *shift != [0.0; 3]));
    // Bitwise: a zero step adds exactly 0 · excess, so no rounding can enter.
    assert_eq!(runs[0].plastic, runs[1].plastic);
}

#[test]
fn the_plastic_shift_stops_at_its_cap() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(&gpu, &[held(LONG_HOLD_STEPS)]);
    let largest = runs[0]
        .plastic
        .iter()
        .map(|s| length3(*s))
        .fold(0.0, f32::max);
    assert!(
        largest <= CAP_RAD * (1.0 + 1e-5),
        "a shift reached {largest} rad"
    );
    assert!(
        largest >= CAP_RAD * (1.0 - 1e-3),
        "the largest shift is {largest} rad"
    );
}

#[test]
fn a_released_strand_rests_on_its_shifted_shape() {
    let gpu = GpuContext::new().unwrap();
    let unbent = strand(tilted(SLANT_DEGREES), AT_ORIGIN).with_floor(FLOOR_FAR_BELOW_PX);
    let released = held(LONG_HOLD_STEPS)
        .with_drag([0.0, 0.0], RELEASE_STEPS)
        .released_for_the_drag()
        .with_drag_seconds(0.0);
    let runs = run(&gpu, &[unbent, released]);
    let rested = &runs[1];
    let largest_excess = rested.excess_rad.iter().copied().fold(0.0, f32::max);
    assert!(
        largest_excess < RESTED_RAD,
        "the released strand still bends {largest_excess} rad"
    );
    let tip = BRISTLE_VERTICES - 1;
    // Each shift turns the chain beyond its hinge, moving the tip by about shift · arm; the
    // sum bounds the tip's move, and the strand must keep at least this share of it.
    let implied_px: f32 = rested
        .plastic
        .iter()
        .enumerate()
        .map(|(hinge, shift)| {
            length3(*shift) * span_px(runs[0].chain_px[hinge], runs[0].chain_px[tip])
        })
        .sum();
    let stays_bent_px = STAYS_BENT_SHARE * implied_px;
    let bent_px = span_px(runs[0].chain_px[tip], rested.chain_px[tip]);
    assert!(
        bent_px > stays_bent_px,
        "the tip moved {bent_px} px, expected over {stays_bent_px}"
    );
}
