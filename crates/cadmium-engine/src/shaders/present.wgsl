struct PresentParams {
    origin: vec2<f32>,
    zoom: f32,
    rotation: f32,
    plate: u32,
};

const NEAREST_ZOOM: f32 = 2.0;
const BACKGROUND_SRGB8: vec3<f32> = vec3<f32>(192.0, 191.0, 188.0);
const SRGB8_MAX: f32 = 255.0;
const LUMINANCE_FROM_LINEAR_SRGB: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);
const SRGB_LINEAR_SEGMENT_END: f32 = 0.0031308;
const SRGB_LINEAR_SLOPE: f32 = 12.92;
const SRGB_GAMMA: f32 = 2.4;
const SRGB_SCALE: f32 = 1.055;
const SRGB_OFFSET: f32 = 0.055;
const HASH_SCALE: f32 = 1.0 / 4294967295.0;
// The tray's plate: a darker ground around it, a soft shadow cast on that ground and a thin
// lit bevel on its edge, all in screen px.
const PLATE_GROUND_SRGB8: vec3<f32> = vec3<f32>(84.0, 84.0, 86.0);
const PLATE_SHADOW: f32 = 0.45;
const PLATE_SHADOW_PX: f32 = 6.0;
const PLATE_BEVEL_LIFT: f32 = 0.1;
const PLATE_BEVEL_PX: f32 = 1.5;

@group(0) @binding(0) var<uniform> params: PresentParams;
@group(0) @binding(1) var display: texture_2d<f32>;
@group(0) @binding(2) var display_sampler: sampler;
@group(0) @binding(3) var target_image: texture_storage_2d<rgba8unorm, write>;

fn pcg(seed: u32) -> u32 {
    let state = seed * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn dither_hash(pixel: vec2<u32>) -> f32 {
    return f32(pcg(pixel.x ^ pcg(pixel.y))) * HASH_SCALE;
}

fn compress_gamut(rgb: vec3<f32>) -> vec3<f32> {
    var out = rgb;
    let lowest = min(min(out.x, out.y), out.z);
    if (lowest < 0.0) {
        let gray = dot(out, LUMINANCE_FROM_LINEAR_SRGB);
        if (gray <= 0.0) {
            return vec3<f32>(0.0);
        }
        let toward_gray = lowest / (lowest - gray);
        out = max(out + (vec3<f32>(gray) - out) * toward_gray, vec3<f32>(0.0));
    }
    let highest = max(max(max(out.x, out.y), out.z), 0.0);
    if (highest > 1.0) {
        out = out / highest;
    }
    return out;
}

fn srgb_encode(linear: vec3<f32>) -> vec3<f32> {
    let v = clamp(linear, vec3<f32>(0.0), vec3<f32>(1.0));
    let curve = SRGB_SCALE * pow(v, vec3<f32>(1.0 / SRGB_GAMMA)) - vec3<f32>(SRGB_OFFSET);
    return select(curve, SRGB_LINEAR_SLOPE * v, v <= vec3<f32>(SRGB_LINEAR_SEGMENT_END));
}

// The canvas offset a screen offset `p` shows when the picture is turned clockwise by `angle`.
fn unturn(p: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(c * p.x + s * p.y, -s * p.x + c * p.y);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let target_size = textureDimensions(target_image);
    if (id.x >= target_size.x || id.y >= target_size.y) {
        return;
    }
    let canvas_size = vec2<f32>(textureDimensions(display));
    let position = params.origin + unturn((vec2<f32>(id.xy) + vec2<f32>(0.5)) / params.zoom, params.rotation);
    if (any(position < vec2<f32>(0.0)) || any(position >= canvas_size)) {
        var surround = BACKGROUND_SRGB8 / SRGB8_MAX;
        if (params.plate != 0u) {
            let outside = length(max(max(-position, position - canvas_size), vec2<f32>(0.0))) * params.zoom;
            let shadow = 1.0 - PLATE_SHADOW * exp(-outside / PLATE_SHADOW_PX);
            surround = PLATE_GROUND_SRGB8 / SRGB8_MAX * shadow;
        }
        textureStore(target_image, vec2<i32>(id.xy), vec4<f32>(surround, 1.0));
        return;
    }
    var linear: vec3<f32>;
    if (params.zoom >= NEAREST_ZOOM) {
        linear = textureLoad(display, vec2<i32>(floor(position)), 0).rgb;
    } else {
        linear = textureSampleLevel(display, display_sampler, position / canvas_size, 0.0).rgb;
    }
    if (params.plate != 0u) {
        let to_edge = min(min(position.x, position.y), min(canvas_size.x - position.x, canvas_size.y - position.y)) * params.zoom;
        linear += vec3<f32>(PLATE_BEVEL_LIFT * (1.0 - smoothstep(0.0, PLATE_BEVEL_PX, to_edge)));
    }
    let encoded = srgb_encode(compress_gamut(linear));
    let dithered = encoded + vec3<f32>((dither_hash(id.xy) - 0.5) / SRGB8_MAX);
    textureStore(target_image, vec2<i32>(id.xy), vec4<f32>(clamp(dithered, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
}
