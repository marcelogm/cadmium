struct Dab {
    center: vec2<f32>,
    radius: f32,
    pressure: f32,
    altitude: f32,
    first_rest: u32,
    direction: f32,
    speed: f32,
    orientation: vec2<f32>,
    elongation: f32,
    tool_mode: u32,
    canvas_origin: vec2<i32>,
    canvas_size: vec2<u32>,
    tip_origin: vec2<i32>,
    tip_size: vec2<u32>,
    transfer_fraction: f32,
    velocity_cutoff: f32,
    residual_um: f32,
    penetration_um: f32,
    bristle_contact: u32,
    full_reserve_volume: f32,
    refill_rate: f32,
    loaded_thickness_um: f32,
    advection: f32,
    squeeze: f32,
    diffusion: f32,
    blade_gap_um: f32,
    spread_origin: vec2<i32>,
    spread_size: vec2<u32>,
    tool_strength: f32,
    rest_follow: f32,
    blade_capacity_um: f32,
    blade_spill_share: f32,
    step_ratio: f32,
    sweep_x: f32,
    sweep_y: f32,
    capillary_rate: f32,
    wicking_rate: f32,
    capillary_scale: f32,
    handle_height_px: f32,
    handle_axis_x: f32,
    handle_axis_y: f32,
    handle_axis_z: f32,
    handle_rest_height_px: f32,
    ferrule_radius_px: f32,
    dt_seconds: f32,
    substeps: u32,
    bristle_total: u32,
    bristle_stiffness: f32,
    bristle_damping: f32,
    bristle_reset: u32,
    bristle_orientation: vec2<f32>,
    ferrule_press_share: f32,
    slump_rate: f32,
    slump_yield_um2_per_px: f32,
    hair_load_limit: f32,
};

struct Body {
    pigment: array<f32, 8>,
    volume: f32,
    granted_ratio: f32,
    granted_fractions: array<f32, 8>,
};

// Where a dab leaves the height it rests on. A stroke's first rest takes the
// found height outright: mix() at a full follow is not enough on drivers that
// lower it as a + t * (b - a) (radv, for one), whose rounding would carry the
// previous stroke's rest into this one.
fn followed_rest(previous: f32, found: f32, follow: f32, first_rest: u32) -> f32 {
    return select(mix(previous, found, follow), found, first_rest != 0u);
}

fn tip_index(texel: vec2<u32>) -> u32 {
    return (texel.y * TIP_SIZE + texel.x) * TIP_CHANNELS;
}
