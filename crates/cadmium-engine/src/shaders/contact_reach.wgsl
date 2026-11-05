struct ContactReference {
    height: f32,
};

const MIN_CONTACT_DEPTH_UM: f32 = 1.0;

fn contact_reach(height: f32, hanging_film: f32) -> f32 {
    let depth = dab.penetration_um * dab.pressure;
    let plane = contact_reference.height - depth - hanging_film;
    return clamp((height - plane) / max(depth, MIN_CONTACT_DEPTH_UM), 0.0, 1.0);
}
