use super::{BRISTLE_TUNING_BYTES, BristleTuning};
use crate::brush::HairSpec;
use crate::engine::bristles::material::{HairMaterial, hinge_compliances_per_m};
use crate::params::SimParams;

const SCALAR_BYTES: u64 = 4;
const VEC4_BYTES: u64 = 16;

fn wgsl_tuning_struct_bytes() -> u64 {
    let source = include_str!("../../../shaders/bristle_tuning.wgsl");
    let body = source
        .split("struct BristleTuning {")
        .nth(1)
        .and_then(|rest| rest.split("};").next())
        .expect("bristle_tuning.wgsl declares struct BristleTuning");
    let scalars = body.lines().filter(|line| line.contains(": f32")).count() as u64;
    let vectors = body
        .lines()
        .filter(|line| line.contains("array<vec4<f32>, 2>"))
        .count() as u64;
    scalars * SCALAR_BYTES + vectors * 2 * VEC4_BYTES
}

#[test]
fn the_shader_tuning_struct_mirrors_the_bristle_tuning() {
    assert_eq!(wgsl_tuning_struct_bytes(), BRISTLE_TUNING_BYTES);
}

#[test]
fn the_tuning_carries_the_hairs_compliances() {
    let tuning = BristleTuning::of(&SimParams::default(), &HairSpec::SABLE, 3.0, 20.0, 64);
    let want = hinge_compliances_per_m(&HairMaterial::of(&HairSpec::SABLE), 20.0, 64);
    assert_eq!(&tuning.hinge_compliance_per_m[..want.len()], &want);
    assert_eq!(tuning.hinge_compliance_per_m[want.len()..], [0.0]);
    assert_eq!(tuning.canvas_tooth_um, 3.0);
    assert_eq!(tuning.wet_support, 0.0);
}
