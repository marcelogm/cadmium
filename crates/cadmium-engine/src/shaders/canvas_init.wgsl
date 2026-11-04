struct InitParams {
    primer_bands_low: vec4<f32>,
    primer_bands_high: vec4<f32>,
    weave_amplitude_um: f32,
    weave_period_px: f32,
    _pad: vec2<f32>,
};

const TAU: f32 = 6.283185307;
const WEAVE_BASE_SHARE: f32 = 0.85;
const WEAVE_JITTER_SHARE: f32 = 0.15;
const HASH_SCALE: f32 = 1.0 / 4294967295.0;

@group(0) @binding(0) var<uniform> params: InitParams;
@group(0) @binding(1) var wet_pig0: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var wet_pig1: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, write>;
@group(0) @binding(4) var dry_refl0: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var dry_refl1: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var dry_height: texture_storage_2d<r32float, write>;
@group(0) @binding(7) var dry_top_pig0: texture_storage_2d<rgba16float, write>;
@group(0) @binding(8) var dry_top_pig1: texture_storage_2d<rgba16float, write>;
@group(0) @binding(9) var display: texture_storage_2d<rgba16float, write>;

fn pcg(seed: u32) -> u32 {
    let state = seed * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn cell_hash(cell: vec2<i32>) -> f32 {
    let mixed = pcg(bitcast<u32>(cell.x) ^ pcg(bitcast<u32>(cell.y)));
    return f32(mixed) * HASH_SCALE;
}

fn weave_height(pixel: vec2<u32>) -> f32 {
    let position = vec2<f32>(pixel) + vec2<f32>(0.5);
    let period = params.weave_period_px;
    let threads = sin(TAU * position.x / period) + sin(TAU * position.y / period);
    let cell = vec2<i32>(floor(position / period));
    let jitter = WEAVE_BASE_SHARE + WEAVE_JITTER_SHARE * cell_hash(cell);
    let height = params.weave_amplitude_um * 0.5 * (1.0 + 0.5 * threads * jitter);
    return clamp(height, 0.0, params.weave_amplitude_um);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(dry_height);
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let pixel = vec2<i32>(id.xy);
    let empty = vec4<f32>(0.0);
    textureStore(wet_pig0, pixel, empty);
    textureStore(wet_pig1, pixel, empty);
    textureStore(wet_vol, pixel, empty);
    textureStore(dry_refl0, pixel, params.primer_bands_low);
    textureStore(dry_refl1, pixel, params.primer_bands_high);
    textureStore(dry_height, pixel, vec4<f32>(weave_height(id.xy), 0.0, 0.0, 0.0));
    textureStore(dry_top_pig0, pixel, empty);
    textureStore(dry_top_pig1, pixel, empty);
    textureStore(display, pixel, empty);
}
