use super::{BRISTLE_TUNING_BYTES, BristleTuning};
use crate::brush::BrushShape;
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
    let tuning = BristleTuning::of(&params, BrushShape::Round, 0.0);
    assert_eq!(tuning.kinetic_friction, 0.4);
    params.set("bristle_kinetic_friction", 0.25);
    assert_eq!(
        BristleTuning::of(&params, BrushShape::Round, 0.0).kinetic_friction,
        0.25
    );
}

#[test]
fn a_round_points_on_both_axes_a_flat_on_its_thin_axis_and_a_fan_not_at_all() {
    let params = SimParams::default();
    let axes = |shape| BristleTuning::of(&params, shape, 0.0).point_axes;
    assert_eq!(axes(BrushShape::Round), [1.0, 1.0]);
    assert_eq!(axes(BrushShape::Flat), [0.0, 1.0]);
    assert_eq!(axes(BrushShape::Fan), [0.0, 0.0]);
}
