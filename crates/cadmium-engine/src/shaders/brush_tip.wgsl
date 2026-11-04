struct Dab {
    center: vec2<f32>,
    radius: f32,
    pressure: f32,
    altitude: f32,
    azimuth: f32,
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
    bristle_contrast: f32,
    bristle_count: f32,
    refill_rate: f32,
    loaded_thickness_um: f32,
    advection: f32,
    squeeze: f32,
    diffusion: f32,
    blade_gap_um: f32,
    spread_origin: vec2<i32>,
    spread_size: vec2<u32>,
    rag_strength: f32,
    rest_follow: f32,
    blade_capacity_um: f32,
    blade_spill_share: f32,
};

struct Body {
    pigment: array<f32, 8>,
    volume: f32,
    granted_ratio: f32,
    granted_fractions: array<f32, 8>,
};

fn tip_index(texel: vec2<u32>) -> u32 {
    return (texel.y * TIP_SIZE + texel.x) * TIP_CHANNELS;
}
