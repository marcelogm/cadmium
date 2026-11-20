struct BristleTuning {
    wet_support: f32,
    canvas_tooth_um: f32,
    spare_0: f32,
    spare_1: f32,
    hinge_compliance_per_m: array<vec4<f32>, 2>,
    /// μₛ, μₖ, anisotropy C_η, creep per second.
    friction: vec4<f32>,
};

fn load_bearing_um(dry_um: f32, wet_um: f32) -> f32 {
    return max(dry_um + tuning.wet_support * wet_um - tuning.canvas_tooth_um, 0.0);
}

/// Bending compliance per metre of segment of hinge `hinge`, 1/(N·m²).
fn hinge_compliance_per_m(hinge: u32) -> f32 {
    return tuning.hinge_compliance_per_m[hinge / 4u][hinge % 4u];
}
