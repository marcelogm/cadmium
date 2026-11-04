struct Dip {
    fractions_low: vec4<f32>,
    fractions_high: vec4<f32>,
    tip_radius: f32,
    tip_add_um: f32,
    tip_limit_um: f32,
    body_add_um: f32,
    body_limit_um: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

const SUM_CHANNELS: u32 = TIP_CHANNELS + 1u;
const TEXEL_COUNT_SUM: u32 = TIP_CHANNELS;

@group(0) @binding(0) var<uniform> dip: Dip;
@group(0) @binding(1) var<storage, read_write> body: Body;
@group(0) @binding(2) var<storage, read_write> tip: array<f32>;
@group(0) @binding(3) var<storage, read_write> tip_sums: array<f32, SUM_CHANNELS>;

var<workgroup> partial: array<f32, TIP_SUM_INVOCATIONS>;

fn inside_tip(texel: vec2<i32>) -> bool {
    let offset = vec2<f32>(texel - vec2<i32>(TIP_HALF));
    return dot(offset, offset) <= dip.tip_radius * dip.tip_radius;
}

fn dipped_fraction(slot: u32) -> f32 {
    return slot_lane(dip.fractions_low, dip.fractions_high, slot);
}

fn room_for(added: f32, held: f32, limit: f32) -> f32 {
    return min(max(limit - held, 0.0), added);
}

@compute @workgroup_size(1)
fn fill_body() {
    let added = room_for(dip.body_add_um, body.volume, dip.body_limit_um);
    if (added <= 0.0) {
        return;
    }
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        body.pigment[slot] += added * dipped_fraction(slot);
    }
    body.volume += added;
}

@compute @workgroup_size(8, 8)
fn fill_tip(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= TIP_SIZE || id.y >= TIP_SIZE || !inside_tip(vec2<i32>(id.xy))) {
        return;
    }
    let base = tip_index(id.xy);
    let added = room_for(dip.tip_add_um, tip[base + VOLUME_CHANNEL], dip.tip_limit_um);
    if (added <= 0.0) {
        return;
    }
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        tip[base + slot] += added * dipped_fraction(slot);
    }
    tip[base + VOLUME_CHANNEL] += added;
}

@compute @workgroup_size(TIP_SUM_INVOCATIONS)
fn sum_tip(@builtin(local_invocation_index) local: u32) {
    let reach = i32(ceil(dip.tip_radius));
    let side = 2 * reach + 1;
    let origin = vec2<i32>(TIP_HALF - reach);
    var mine = array<f32, SUM_CHANNELS>();
    for (var index = i32(local); index < side * side; index += i32(TIP_SUM_INVOCATIONS)) {
        let texel = origin + vec2<i32>(index % side, index / side);
        if (any(texel < vec2<i32>(0)) || any(texel >= vec2<i32>(i32(TIP_SIZE))) || !inside_tip(texel)) {
            continue;
        }
        let base = tip_index(vec2<u32>(texel));
        for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
            mine[channel] += tip[base + channel];
        }
        mine[TEXEL_COUNT_SUM] += 1.0;
    }
    for (var channel = 0u; channel < SUM_CHANNELS; channel++) {
        partial[local] = mine[channel];
        workgroupBarrier();
        for (var stride = TIP_SUM_INVOCATIONS / 2u; stride > 0u; stride /= 2u) {
            if (local < stride) {
                partial[local] += partial[local + stride];
            }
            workgroupBarrier();
        }
        if (local == 0u) {
            tip_sums[channel] = partial[0];
        }
        workgroupBarrier();
    }
}
