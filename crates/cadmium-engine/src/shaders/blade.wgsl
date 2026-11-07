struct BladeState {
    rest: f32,
    plane: f32,
    band_weight: array<f32, 2>,
    spill: array<f32, BLADE_SPILL_VALUES>,
};

const LOWER_END: u32 = 0u;
const UPPER_END: u32 = 1u;
const WANT_CHANNEL: u32 = TIP_CHANNELS;
const GRANT_RATIO_CHANNEL: u32 = VOLUME_CHANNEL;
const BLADE_THREADS: u32 = 256u;

var<workgroup> reduced: array<f32, BLADE_THREADS>;
var<workgroup> lower_weights: array<f32, BLADE_THREADS>;
var<workgroup> upper_weights: array<f32, BLADE_THREADS>;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var wet_pig0: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(2) var wet_pig1: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, read_write>;
@group(0) @binding(4) var dry_height: texture_storage_2d<r32float, read>;
@group(0) @binding(5) var<storage, read_write> blade_load: array<f32>;
@group(0) @binding(6) var<storage, read_write> blade_delta: array<atomic<i32>>;
@group(0) @binding(7) var<storage, read_write> blade_grant: array<f32>;
@group(0) @binding(8) var<storage, read_write> blade: BladeState;

fn band_cover(position: vec2<f32>, end: u32) -> f32 {
    let semi = semi_axes();
    let along = clamp(semi.x - abs(position.x) + BLADE_COVERAGE_HALF_PX, 0.0, 1.0);
    var outward = position.y;
    if (end == LOWER_END) {
        outward = -position.y;
    }
    let beyond = outward - semi.y + BLADE_COVERAGE_HALF_PX;
    let across = clamp(beyond, 0.0, 1.0) - clamp(beyond - BLADE_SPILL_BAND_PX, 0.0, 1.0);
    return along * across;
}

fn lay_wanted(pixel: vec2<i32>, cover: f32) -> i32 {
    let dry = textureLoad(dry_height, pixel).x;
    let surface = dry + textureLoad(wet_vol, pixel).x;
    let laid_top = mix(dry, blade.plane, cover);
    return max(quantize_down(laid_top - surface), 0);
}

fn reduce_sum(thread: u32) {
    for (var stride = BLADE_THREADS / 2u; stride > 0u; stride /= 2u) {
        if (thread < stride) {
            reduced[thread] += reduced[thread + stride];
        }
        workgroupBarrier();
    }
}

@compute @workgroup_size(BLADE_THREADS)
fn rest(@builtin(local_invocation_index) thread: u32) {
    var peak = NO_PEAK;
    var lower = 0.0;
    var upper = 0.0;
    let count = dab.canvas_size.x * dab.canvas_size.y;
    for (var index = thread; index < count; index += BLADE_THREADS) {
        let pixel = region_pixel(index);
        let position = brush_space(pixel);
        if (all(abs(position) <= semi_axes())) {
            peak = max(peak, textureLoad(dry_height, pixel).x);
        }
        lower += band_cover(position, LOWER_END);
        upper += band_cover(position, UPPER_END);
    }
    reduced[thread] = peak;
    lower_weights[thread] = lower;
    upper_weights[thread] = upper;
    workgroupBarrier();
    for (var stride = BLADE_THREADS / 2u; stride > 0u; stride /= 2u) {
        if (thread < stride) {
            reduced[thread] = max(reduced[thread], reduced[thread + stride]);
            lower_weights[thread] += lower_weights[thread + stride];
            upper_weights[thread] += upper_weights[thread + stride];
        }
        workgroupBarrier();
    }
    if (thread == 0u) {
        var found = reduced[0];
        if (found <= NO_PEAK) {
            found = centre_dry_height();
        }
        let settled = followed_rest(blade.rest, found, dab.rest_follow, dab.first_rest);
        blade.rest = settled;
        blade.plane = settled + dab.blade_gap_um;
        blade.band_weight[LOWER_END] = lower_weights[0];
        blade.band_weight[UPPER_END] = upper_weights[0];
    }
}

@compute @workgroup_size(8, 8)
fn want(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let pixel = dab.canvas_origin + vec2<i32>(id.xy);
    let position = brush_space(pixel);
    let cover = lateral_cover(position);
    if (cover <= 0.0) {
        return;
    }
    let wanted = lay_wanted(pixel, cover);
    if (wanted > 0) {
        atomicAdd(&blade_delta[cell_of(position) * BLADE_DELTA_CHANNELS + WANT_CHANNEL], wanted);
    }
}

@compute @workgroup_size(BLADE_CELL_GROUP_SIZE)
fn grant(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= TIP_SIZE) {
        return;
    }
    let wanted = dequantize(atomicExchange(&blade_delta[cell * BLADE_DELTA_CHANNELS + WANT_CHANNEL], 0));
    let base = cell * TIP_CHANNELS;
    let held = blade_load[base + VOLUME_CHANNEL];
    var ratio = 0.0;
    if (wanted > 0.0 && held > EMPTY_VOLUME_UM) {
        ratio = min(1.0, held / wanted) * GRANT_SAFETY;
    }
    blade_grant[base + GRANT_RATIO_CHANNEL] = ratio;
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        blade_grant[base + slot] = held_share(blade_load[base + slot], held);
    }
}

@compute @workgroup_size(8, 8)
fn lay(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let pixel = dab.canvas_origin + vec2<i32>(id.xy);
    let position = brush_space(pixel);
    let cover = lateral_cover(position);
    if (cover <= 0.0) {
        return;
    }
    let cell = cell_of(position);
    let grant_base = cell * TIP_CHANNELS;
    let delta_base = cell * BLADE_DELTA_CHANNELS;
    let volume = textureLoad(wet_vol, pixel).x;
    let surface = textureLoad(dry_height, pixel).x + volume;
    let deposit_q = i32(floor(f32(lay_wanted(pixel, cover)) * blade_grant[grant_base + GRANT_RATIO_CHANNEL]));
    let scraped = cover * min(max(surface - blade.plane, 0.0), max(volume - dab.residual_um, 0.0));
    let pickup_q = clamp(quantize(scraped), 0, max(quantize_down(volume), 0));
    if (deposit_q == 0 && pickup_q == 0) {
        return;
    }
    var shares = array<f32, 8>();
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        shares[slot] = blade_grant[grant_base + slot];
    }
    let gained = trade(pixel, volume, deposit_q, pickup_q, shares);
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        if (gained[channel] != 0) {
            atomicAdd(&blade_delta[delta_base + channel], gained[channel]);
        }
    }
}

@compute @workgroup_size(BLADE_THREADS)
fn settle(@builtin(local_invocation_index) thread: u32) {
    let reach = i32(ceil(dab.radius)) + 1;
    let first = max(TIP_HALF - reach, 0);
    let last = min(TIP_HALF + reach, i32(TIP_SIZE) - 1);
    var spilled = array<f32, BLADE_SPILL_VALUES>();
    for (var cell = first + i32(thread); cell <= last; cell += i32(BLADE_THREADS)) {
        let base = u32(cell) * TIP_CHANNELS;
        let delta_base = u32(cell) * BLADE_DELTA_CHANNELS;
        for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
            let change = atomicExchange(&blade_delta[delta_base + channel], 0);
            if (change != 0) {
                blade_load[base + channel] += dequantize(change);
            }
        }
        let held = blade_load[base + VOLUME_CHANNEL];
        let excess = max(held - dab.blade_capacity_um, 0.0) * dab.blade_spill_share;
        var end = UPPER_END;
        if (cell < TIP_HALF) {
            end = LOWER_END;
        }
        if (excess <= 0.0 || blade.band_weight[end] <= 0.0) {
            continue;
        }
        let share = excess / held;
        for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
            let moved = blade_load[base + channel] * share;
            blade_load[base + channel] -= moved;
            spilled[end * TIP_CHANNELS + channel] += moved;
        }
    }
    for (var value = 0u; value < BLADE_SPILL_VALUES; value++) {
        reduced[thread] = spilled[value];
        workgroupBarrier();
        reduce_sum(thread);
        if (thread == 0u) {
            blade.spill[value] = reduced[0];
        }
        workgroupBarrier();
    }
}

@compute @workgroup_size(8, 8)
fn spill(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let pixel = dab.canvas_origin + vec2<i32>(id.xy);
    let position = brush_space(pixel);
    var added = array<f32, 9>();
    var touched = false;
    for (var end = LOWER_END; end <= UPPER_END; end++) {
        let weight = band_cover(position, end);
        let total = blade.band_weight[end];
        if (weight <= 0.0 || total <= 0.0 || blade.spill[end * TIP_CHANNELS + VOLUME_CHANNEL] <= 0.0) {
            continue;
        }
        touched = true;
        let share = weight / total;
        for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
            added[channel] += blade.spill[end * TIP_CHANNELS + channel] * share;
        }
    }
    if (!touched) {
        return;
    }
    let volume = textureLoad(wet_vol, pixel).x;
    let fractions = read_fractions(pixel);
    var amounts = array<f32, 8>();
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        amounts[slot] = fractions[slot] * volume + added[slot];
    }
    store_paint(pixel, volume + added[VOLUME_CHANNEL], amounts);
}
