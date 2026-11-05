use super::{BRISTLE_TUNING_BYTES, BristleTuning};
use crate::brush::HeadSpec;
use crate::params::SimParams;

const SCALAR_BYTES: u64 = 4;

fn wgsl_tuning_struct_bytes() -> u64 {
    let source = include_str!("../../../shaders/bristle_tuning.wgsl");
    let body = source
        .split("struct BristleTuning {")
        .nth(1)
        .and_then(|rest| rest.split("};").next())
        .expect("bristle_tuning.wgsl declares struct BristleTuning");
    body.lines().filter(|line| line.contains(": f32")).count() as u64 * SCALAR_BYTES
}

#[test]
fn the_shader_tuning_struct_mirrors_the_bristle_tuning() {
    assert_eq!(wgsl_tuning_struct_bytes(), BRISTLE_TUNING_BYTES);
}

#[test]
fn kinetic_friction_never_exceeds_static_friction() {
    let mut params = SimParams::default();
    params.set("bristle_static_friction", 0.4);
    params.set("bristle_kinetic_friction", 1.5);
    let tuning = BristleTuning::of(&params, &HeadSpec::ROUND, 0.0);
    assert_eq!(tuning.kinetic_friction, 0.4);
    params.set("bristle_kinetic_friction", 0.25);
    assert_eq!(
        BristleTuning::of(&params, &HeadSpec::ROUND, 0.0).kinetic_friction,
        0.25
    );
}

#[test]
fn a_round_points_on_both_axes_a_flat_on_its_thin_axis_and_a_fan_not_at_all() {
    let params = SimParams::default();
    let axes = |head: HeadSpec| BristleTuning::of(&params, &head, 0.0).point_axes;
    assert_eq!(axes(HeadSpec::ROUND), [1.0, 1.0]);
    assert_eq!(axes(HeadSpec::FLAT), [0.0, 1.0]);
    assert_eq!(axes(HeadSpec::FAN), [0.0, 0.0]);
}

#[test]
fn the_default_round_brush_keeps_todays_tuning() {
    let tuning = BristleTuning::of(&SimParams::default(), &HeadSpec::ROUND, 3.0);
    assert_eq!(
        tuning,
        BristleTuning {
            static_friction: 1.5,
            kinetic_friction: 1.2,
            drag_anisotropy: 0.3,
            plasticity: 2.0,
            clumping: 0.5,
            pointing: 0.9,
            point_release: 0.15,
            point_axes: [1.0, 1.0],
            point_film: 0.25,
            wet_support: 0.0,
            canvas_tooth_um: 3.0,
        }
    );
}
