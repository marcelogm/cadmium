struct ContactReference {
    height: f32,
};

const MAX_DEPOSIT_SHARE: f32 = 0.45;
const SLOW_TRANSFER_SHARE: f32 = 0.3;
const MIN_VELOCITY_CUTOFF: f32 = 1e-6;
const MIN_CONTACT_DEPTH_UM: f32 = 1.0;
const EMPTY_VOLUME_UM: f32 = 1e-4;
const QUANTIZE_LIMIT: f32 = 2.0e9;
const MIN_KEPT_SHARE: f32 = 1e-6;
const REFERENCE_THREADS: u32 = 256u;

var<workgroup> weighted_heights: array<f32, REFERENCE_THREADS>;
var<workgroup> weights: array<f32, REFERENCE_THREADS>;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var wet_pig0: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(2) var wet_pig1: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, read_write>;
@group(0) @binding(4) var dry_height: texture_storage_2d<r32float, read>;
@group(0) @binding(5) var<storage, read> tip: array<f32>;
@group(0) @binding(6) var<storage, read_write> tip_delta: array<atomic<i32>>;
@group(0) @binding(7) var<storage, read_write> contact_reference: ContactReference;

fn quantize(amount: f32) -> i32 {
    return i32(round(clamp(amount * DELTA_SCALE, -QUANTIZE_LIMIT, QUANTIZE_LIMIT)));
}

fn quantize_down(amount: f32) -> i32 {
    return i32(floor(clamp(amount * DELTA_SCALE, -QUANTIZE_LIMIT, QUANTIZE_LIMIT)));
}

fn dequantize(amount: i32) -> f32 {
    return f32(amount) / DELTA_SCALE;
}

fn surface_height(pixel: vec2<i32>) -> f32 {
    return textureLoad(dry_height, pixel).x + textureLoad(wet_vol, pixel).x;
}

fn contact(height: f32) -> f32 {
    let depth = dab.penetration_um * dab.pressure;
    let plane = contact_reference.height - depth;
    return clamp((height - plane) / max(depth, MIN_CONTACT_DEPTH_UM), 0.0, 1.0);
}

fn velocity_share() -> f32 {
    let ramp = smoothstep(0.0, max(dab.velocity_cutoff, MIN_VELOCITY_CUTOFF), dab.speed);
    return mix(SLOW_TRANSFER_SHARE, 1.0, ramp);
}

struct Transfer {
    deposit: f32,
    pickup: f32,
};

fn bristle_transfer(mask: f32, volume: f32, height: f32, tip_volume: f32) -> Transfer {
    let reference_rate = dab.transfer_fraction * mask * contact(height) * velocity_share();
    let rate = 1.0 - pow(max(1.0 - reference_rate, MIN_KEPT_SHARE), dab.step_ratio);
    let deposit = min(rate * tip_volume, MAX_DEPOSIT_SHARE * tip_volume);
    let pickup = rate * max(volume - dab.residual_um, 0.0);
    return Transfer(deposit, pickup);
}

fn read_fractions(pixel: vec2<i32>) -> array<f32, 8> {
    let low = textureLoad(wet_pig0, pixel);
    let high = textureLoad(wet_pig1, pixel);
    return array<f32, 8>(low.x, low.y, low.z, low.w, high.x, high.y, high.z, high.w);
}

fn centre_height() -> f32 {
    let size = vec2<i32>(textureDimensions(wet_vol));
    let pixel = clamp(vec2<i32>(floor(dab.center)), vec2<i32>(0), size - vec2<i32>(1));
    return surface_height(pixel);
}

@compute @workgroup_size(REFERENCE_THREADS)
fn reference(@builtin(local_invocation_index) thread: u32) {
    var weighted = 0.0;
    var weight = 0.0;
    let width = dab.canvas_size.x;
    let count = width * dab.canvas_size.y;
    for (var index = thread; index < count; index += REFERENCE_THREADS) {
        let pixel = dab.canvas_origin + vec2<i32>(vec2<u32>(index % width, index / width));
        let cover = outline_footprint(brush_space(pixel));
        weighted += cover * surface_height(pixel);
        weight += cover;
    }
    weighted_heights[thread] = weighted;
    weights[thread] = weight;
    workgroupBarrier();
    for (var stride = REFERENCE_THREADS / 2u; stride > 0u; stride /= 2u) {
        if (thread < stride) {
            weighted_heights[thread] += weighted_heights[thread + stride];
            weights[thread] += weights[thread + stride];
        }
        workgroupBarrier();
    }
    if (thread == 0u) {
        var found = centre_height();
        if (weights[0] > 0.0) {
            found = weighted_heights[0] / weights[0];
        }
        contact_reference.height = mix(contact_reference.height, found, dab.rest_follow);
    }
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let pixel = dab.canvas_origin + vec2<i32>(id.xy);
    let position = brush_space(pixel);
    let mask = footprint(position);
    if (mask <= 0.0) {
        return;
    }
    let texel = vec2<i32>(round(position)) + vec2<i32>(TIP_HALF);
    if (any(texel < vec2<i32>(0)) || any(texel >= vec2<i32>(i32(TIP_SIZE)))) {
        return;
    }
    let base = tip_index(vec2<u32>(texel));

    let volume = textureLoad(wet_vol, pixel).x;
    let fractions = read_fractions(pixel);
    let dry = textureLoad(dry_height, pixel).x;
    let tip_volume = tip[base + VOLUME_CHANNEL];
    let transfer = bristle_transfer(mask, volume, dry + volume, tip_volume);
    let deposit = transfer.deposit;
    let pickup = transfer.pickup;
    let deposit_q = max(quantize(deposit), 0);
    let pickup_q = clamp(quantize(pickup), 0, max(quantize_down(volume), 0));
    if (deposit_q == 0 && pickup_q == 0) {
        return;
    }
    let deposit_amount = dequantize(deposit_q);
    let pickup_amount = dequantize(pickup_q);
    let new_volume = volume - pickup_amount + deposit_amount;

    var new_fractions = array<f32, 8>();
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        var tip_share = 0.0;
        if (tip_volume > EMPTY_VOLUME_UM) {
            tip_share = max(tip[base + slot], 0.0) / tip_volume;
        }
        let deposit_slot_q = max(quantize(deposit * tip_share), 0);
        let pickup_slot_q = max(quantize(pickup_amount * fractions[slot]), 0);
        let amount = fractions[slot] * volume
            + dequantize(deposit_slot_q)
            - dequantize(pickup_slot_q);
        if (new_volume >= EMPTY_VOLUME_UM) {
            new_fractions[slot] = nearest_half(max(amount, 0.0) / new_volume);
        }
        let tip_change = pickup_slot_q - deposit_slot_q;
        if (tip_change != 0) {
            atomicAdd(&tip_delta[base + slot], tip_change);
        }
    }
    if (new_volume >= EMPTY_VOLUME_UM) {
        new_fractions = within_whole_volume(new_fractions);
    }
    atomicAdd(&tip_delta[base + VOLUME_CHANNEL], pickup_q - deposit_q);

    textureStore(wet_vol, pixel, vec4<f32>(new_volume, 0.0, 0.0, 0.0));
    textureStore(wet_pig0, pixel, vec4<f32>(new_fractions[0], new_fractions[1], new_fractions[2], new_fractions[3]));
    textureStore(wet_pig1, pixel, vec4<f32>(new_fractions[4], new_fractions[5], new_fractions[6], new_fractions[7]));
}
