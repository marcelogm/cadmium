const CLOTH_BASE: f32 = 0.6;
const CLOTH_WEAVE: f32 = 0.4;
const HASH_MULTIPLIER: u32 = 747796405u;
const HASH_INCREMENT: u32 = 2891336453u;
const HASH_MIX: u32 = 277803737u;
const HASH_SHIFT_SELECT: u32 = 28u;
const HASH_SHIFT_BASE: u32 = 4u;
const HASH_OUTPUT_SHIFT: u32 = 22u;
const NOISE_DISCARDED_BITS: u32 = 8u;
const NOISE_RANGE: f32 = 16777216.0;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var wet_vol: texture_storage_2d<r32float, read_write>;

fn hash(value: u32) -> u32 {
    let state = value * HASH_MULTIPLIER + HASH_INCREMENT;
    let word = ((state >> ((state >> HASH_SHIFT_SELECT) + HASH_SHIFT_BASE)) ^ state) * HASH_MIX;
    return (word >> HASH_OUTPUT_SHIFT) ^ word;
}

fn weave_noise(pixel: vec2<i32>) -> f32 {
    let texel = vec2<u32>(pixel);
    return f32(hash(texel.x ^ hash(texel.y)) >> NOISE_DISCARDED_BITS) / NOISE_RANGE;
}

fn cloth(pixel: vec2<i32>) -> f32 {
    return CLOTH_BASE + CLOTH_WEAVE * weave_noise(pixel);
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
    let volume = textureLoad(wet_vol, pixel).x;
    if (volume <= 0.0) {
        return;
    }
    let kept = clamp(1.0 - dab.tool_strength * dab.pressure * mask * cloth(pixel), 0.0, 1.0);
    textureStore(wet_vol, pixel, vec4<f32>(volume * kept, 0.0, 0.0, 0.0));
}
