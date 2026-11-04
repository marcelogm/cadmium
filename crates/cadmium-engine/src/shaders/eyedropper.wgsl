struct Pick {
    center: vec2<f32>,
    radius: f32,
    tip_radius: f32,
    thickness_um: f32,
    empty_body_volume_um: f32,
};

const SUM_CHANNELS: u32 = 2u * PIGMENT_CHANNELS + 2u;
const WET_VOLUME_SUM: u32 = PIGMENT_CHANNELS;
const DRY_FRACTION_SUMS: u32 = PIGMENT_CHANNELS + 1u;
const DRY_PIXEL_COUNT: u32 = 2u * PIGMENT_CHANNELS + 1u;
const MIN_WET_VOLUME_UM: f32 = 1e-3;

@group(0) @binding(0) var<uniform> pick: Pick;
@group(0) @binding(1) var wet_pig0: texture_storage_2d<rgba16float, read>;
@group(0) @binding(2) var wet_pig1: texture_storage_2d<rgba16float, read>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, read>;
@group(0) @binding(4) var dry_top_pig0: texture_storage_2d<rgba16float, read>;
@group(0) @binding(5) var dry_top_pig1: texture_storage_2d<rgba16float, read>;
@group(0) @binding(6) var<storage, read_write> body: Body;
@group(0) @binding(7) var<storage, read_write> tip: array<f32>;
@group(0) @binding(8) var<storage, read_write> paint_found: u32;

var<workgroup> partial: array<f32, PICK_INVOCATIONS>;
var<workgroup> sums: array<f32, SUM_CHANNELS>;

fn lane_total(low: vec4<f32>, high: vec4<f32>) -> f32 {
    return dot(low, vec4<f32>(1.0)) + dot(high, vec4<f32>(1.0));
}

fn found_paint() -> bool {
    return sums[WET_VOLUME_SUM] >= MIN_WET_VOLUME_UM || sums[DRY_PIXEL_COUNT] > 0.0;
}

fn picked_fractions() -> array<f32, 8> {
    var fractions = array<f32, 8>();
    let wet_volume = sums[WET_VOLUME_SUM];
    let dry_pixels = sums[DRY_PIXEL_COUNT];
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        if (wet_volume >= MIN_WET_VOLUME_UM) {
            fractions[slot] = sums[slot] / wet_volume;
        } else if (dry_pixels > 0.0) {
            fractions[slot] = sums[DRY_FRACTION_SUMS + slot] / dry_pixels;
        }
    }
    return fractions;
}

@compute @workgroup_size(PICK_INVOCATIONS)
fn sample(@builtin(local_invocation_index) local: u32) {
    let size = vec2<i32>(textureDimensions(wet_vol));
    let reach = i32(ceil(pick.radius));
    let side = 2 * reach + 1;
    let origin = vec2<i32>(floor(pick.center)) - vec2<i32>(reach);
    var mine = array<f32, SUM_CHANNELS>();
    for (var index = i32(local); index < side * side; index += i32(PICK_INVOCATIONS)) {
        let pixel = origin + vec2<i32>(index % side, index / side);
        if (any(pixel < vec2<i32>(0)) || any(pixel >= size)) {
            continue;
        }
        if (distance(vec2<f32>(pixel) + vec2<f32>(0.5), pick.center) > pick.radius) {
            continue;
        }
        let volume = max(textureLoad(wet_vol, pixel).x, 0.0);
        let wet_low = textureLoad(wet_pig0, pixel);
        let wet_high = textureLoad(wet_pig1, pixel);
        let dry_low = textureLoad(dry_top_pig0, pixel);
        let dry_high = textureLoad(dry_top_pig1, pixel);
        let dry_painted = lane_total(dry_low, dry_high) > 0.0;
        for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
            mine[slot] += slot_lane(wet_low, wet_high, slot) * volume;
            if (dry_painted) {
                mine[DRY_FRACTION_SUMS + slot] += slot_lane(dry_low, dry_high, slot);
            }
        }
        mine[WET_VOLUME_SUM] += volume;
        if (dry_painted) {
            mine[DRY_PIXEL_COUNT] += 1.0;
        }
    }
    for (var channel = 0u; channel < SUM_CHANNELS; channel++) {
        partial[local] = mine[channel];
        workgroupBarrier();
        for (var stride = PICK_INVOCATIONS / 2u; stride > 0u; stride /= 2u) {
            if (local < stride) {
                partial[local] += partial[local + stride];
            }
            workgroupBarrier();
        }
        if (local == 0u) {
            sums[channel] = partial[0];
        }
        workgroupBarrier();
    }
    if (local != 0u) {
        return;
    }
    let found = found_paint();
    paint_found = select(0u, 1u, found);
    if (!found) {
        return;
    }
    let fractions = picked_fractions();
    var volume = body.volume;
    if (volume <= 0.0) {
        volume = pick.empty_body_volume_um;
    }
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        body.pigment[slot] = volume * fractions[slot];
    }
    body.volume = volume;
}

@compute @workgroup_size(8, 8)
fn reload(@builtin(global_invocation_id) id: vec3<u32>) {
    if (paint_found == 0u || id.x >= TIP_SIZE || id.y >= TIP_SIZE) {
        return;
    }
    let offset = vec2<f32>(vec2<i32>(id.xy) - vec2<i32>(TIP_HALF));
    var thickness = 0.0;
    if (dot(offset, offset) <= pick.tip_radius * pick.tip_radius) {
        thickness = pick.thickness_um;
    }
    let base = tip_index(id.xy);
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        var share = 0.0;
        if (body.volume > 0.0) {
            share = body.pigment[slot] / body.volume;
        }
        tip[base + slot] = thickness * share;
    }
    tip[base + VOLUME_CHANNEL] = thickness;
}
