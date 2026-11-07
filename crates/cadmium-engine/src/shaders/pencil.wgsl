// Graphite is scraped off the lead by the tooth: every texel has a fixed grain (the canvas
// relief plus a fine speckle), and pressure decides which grains are high enough to catch it.
// A texel only darkens towards the coverage its grain allows, so going over a line builds
// it up gently instead of filling the grain in solid. A soft B lead catches on lower grains
// and lays more down; a hard H lead only marks the peaks and never gets very dark.
const TOOTH_REACH_PX: i32 = 3;
const NEIGHBOUR_SHARE: f32 = 0.25;
const RELIEF_SPAN_UM: f32 = 12.0;
const RELIEF_SHARE: f32 = 0.4;
const SPECKLE_SHARE: f32 = 0.6;
const SOFT_THRESHOLD_LIGHT: f32 = 0.52;
const SOFT_THRESHOLD_FIRM: f32 = 0.1;
const SOFT_COVERAGE_LIGHT: f32 = 0.7;
const SOFT_COVERAGE_FIRM: f32 = 1.25;
const HARD_THRESHOLD_LIGHT: f32 = 0.9;
const HARD_THRESHOLD_FIRM: f32 = 0.45;
const HARD_COVERAGE_LIGHT: f32 = 0.22;
const HARD_COVERAGE_FIRM: f32 = 0.65;
const THRESHOLD_SOFTNESS: f32 = 0.12;
const FULL_DENSITY: f32 = 1.0;
const HASH_MULTIPLIER: u32 = 747796405u;
const HASH_INCREMENT: u32 = 2891336453u;
const HASH_MIX: u32 = 277803737u;
const HASH_SHIFT_SELECT: u32 = 28u;
const HASH_SHIFT_BASE: u32 = 4u;
const HASH_OUTPUT_SHIFT: u32 = 22u;
const HASH_SALT: u32 = 0x9e3779b9u;
const NOISE_DISCARDED_BITS: u32 = 8u;
const NOISE_RANGE: f32 = 16777216.0;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var graphite: texture_storage_2d<r32float, read_write>;
@group(0) @binding(2) var dry_height: texture_storage_2d<r32float, read>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, read>;
@group(0) @binding(4) var<uniform> lead: Lead;

struct Lead {
    hardness: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

fn hash(value: u32) -> u32 {
    let state = value * HASH_MULTIPLIER + HASH_INCREMENT;
    let word = ((state >> ((state >> HASH_SHIFT_SELECT) + HASH_SHIFT_BASE)) ^ state) * HASH_MIX;
    return (word >> HASH_OUTPUT_SHIFT) ^ word;
}

fn speckle(pixel: vec2<i32>) -> f32 {
    let texel = vec2<u32>(pixel);
    let word = hash(texel.x ^ hash(texel.y ^ HASH_SALT));
    return f32(word >> NOISE_DISCARDED_BITS) / NOISE_RANGE;
}

fn surface_um(pixel: vec2<i32>) -> f32 {
    let size = vec2<i32>(textureDimensions(graphite));
    let p = clamp(pixel, vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(dry_height, p).x + textureLoad(wet_vol, p).x;
}

fn relief_um(pixel: vec2<i32>) -> f32 {
    let across = vec2<i32>(TOOTH_REACH_PX, 0);
    let down = vec2<i32>(0, TOOTH_REACH_PX);
    let around = surface_um(pixel + across) + surface_um(pixel - across)
        + surface_um(pixel + down) + surface_um(pixel - down);
    return surface_um(pixel) - NEIGHBOUR_SHARE * around;
}

/// How proud this texel stands, 0 in the deepest valley to 1 on the highest peak.
fn grain(pixel: vec2<i32>) -> f32 {
    let relief = clamp(relief_um(pixel) / RELIEF_SPAN_UM + 0.5, 0.0, 1.0);
    return RELIEF_SHARE * relief + SPECKLE_SHARE * speckle(pixel);
}

fn coverage(pixel: vec2<i32>) -> f32 {
    let hardness = clamp(lead.hardness, 0.0, 1.0);
    let threshold_light = mix(SOFT_THRESHOLD_LIGHT, HARD_THRESHOLD_LIGHT, hardness);
    let threshold_firm = mix(SOFT_THRESHOLD_FIRM, HARD_THRESHOLD_FIRM, hardness);
    let coverage_light = mix(SOFT_COVERAGE_LIGHT, HARD_COVERAGE_LIGHT, hardness);
    let coverage_firm = mix(SOFT_COVERAGE_FIRM, HARD_COVERAGE_FIRM, hardness);
    let threshold = mix(threshold_light, threshold_firm, dab.pressure);
    let caught = smoothstep(
        threshold - THRESHOLD_SOFTNESS,
        threshold + THRESHOLD_SOFTNESS,
        grain(pixel)
    );
    return min(caught * mix(coverage_light, coverage_firm, dab.pressure), FULL_DENSITY);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let pixel = dab.canvas_origin + vec2<i32>(id.xy);
    let mask = footprint(brush_space(pixel));
    if (mask <= 0.0) {
        return;
    }
    let target_density = coverage(pixel) * mask;
    let density = textureLoad(graphite, pixel).x;
    if (target_density <= density) {
        return;
    }
    let rate = clamp(dab.tool_strength, 0.0, 1.0);
    textureStore(graphite, pixel, vec4<f32>(mix(density, target_density, rate), 0.0, 0.0, 0.0));
}
