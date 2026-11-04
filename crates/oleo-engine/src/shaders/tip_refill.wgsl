struct Body {
    pigment: array<f32, 8>,
    volume: f32,
    granted_ratio: f32,
    granted_fractions: array<f32, 8>,
};

struct Wanted {
    low: atomic<u32>,
    high: atomic<u32>,
};

const LOW_WORD_MAX: u32 = 0xffffffffu;
const HIGH_WORD_WEIGHT: f32 = 4294967296.0;
const QUANTIZE_LIMIT: f32 = 2.0e9;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var<storage, read_write> tip: array<f32>;
@group(0) @binding(2) var<storage, read_write> tip_delta: array<atomic<i32>>;
@group(0) @binding(3) var<storage, read_write> body: Body;
@group(0) @binding(4) var<storage, read_write> wanted: Wanted;

fn refill_texel(id: vec3<u32>) -> vec2<u32> {
    return vec2<u32>(dab.tip_origin + vec2<i32>(id.xy));
}

fn inside_footprint(texel: vec2<u32>) -> bool {
    return inside_outline(vec2<f32>(vec2<i32>(texel) - vec2<i32>(TIP_HALF)));
}

@compute @workgroup_size(8, 8)
fn gather(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.tip_size.x || id.y >= dab.tip_size.y) {
        return;
    }
    let texel = refill_texel(id);
    if (!inside_footprint(texel)) {
        return;
    }
    let base = tip_index(texel);
    let shortfall = max(dab.loaded_thickness_um - tip[base + VOLUME_CHANNEL], 0.0);
    let want = u32(round(clamp(dab.refill_rate * shortfall * DELTA_SCALE, 0.0, QUANTIZE_LIMIT)));
    if (want == 0u) {
        return;
    }
    atomicStore(&tip_delta[base + VOLUME_CHANNEL], i32(want));
    let previous = atomicAdd(&wanted.low, want);
    if (previous > LOW_WORD_MAX - want) {
        atomicAdd(&wanted.high, 1u);
    }
}

@compute @workgroup_size(1)
fn settle() {
    let wanted_volume = (f32(atomicLoad(&wanted.high)) * HIGH_WORD_WEIGHT + f32(atomicLoad(&wanted.low))) / DELTA_SCALE;
    atomicStore(&wanted.low, 0u);
    atomicStore(&wanted.high, 0u);
    let available = body.volume;
    var ratio = 0.0;
    if (wanted_volume > 0.0 && available > 0.0) {
        ratio = min(1.0, available / wanted_volume);
    }
    let granted = wanted_volume * ratio;
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        var share = 0.0;
        if (available > 0.0) {
            share = body.pigment[slot] / available;
        }
        body.granted_fractions[slot] = share;
        body.pigment[slot] = max(body.pigment[slot] - granted * share, 0.0);
    }
    body.volume = max(available - granted, 0.0);
    body.granted_ratio = ratio;
}

@compute @workgroup_size(8, 8)
fn distribute(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.tip_size.x || id.y >= dab.tip_size.y) {
        return;
    }
    let base = tip_index(refill_texel(id));
    let want = atomicExchange(&tip_delta[base + VOLUME_CHANNEL], 0);
    if (want == 0) {
        return;
    }
    let granted = f32(want) / DELTA_SCALE * body.granted_ratio;
    tip[base + VOLUME_CHANNEL] += granted;
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        tip[base + slot] += granted * body.granted_fractions[slot];
    }
}
