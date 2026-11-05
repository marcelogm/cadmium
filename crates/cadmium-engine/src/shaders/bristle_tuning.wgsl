struct BristleTuning {
    static_friction: f32,
    kinetic_friction: f32,
    drag_anisotropy: f32,
    plasticity: f32,
    clumping: f32,
    pointing: f32,
    point_release: f32,
    point_axis_across: f32,
    point_axis_along: f32,
    point_film: f32,
    wet_support: f32,
    canvas_tooth_um: f32,
};

fn load_bearing_um(dry_um: f32, wet_um: f32) -> f32 {
    return max(dry_um + tuning.wet_support * wet_um - tuning.canvas_tooth_um, 0.0);
}
