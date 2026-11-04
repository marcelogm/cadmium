struct Motion {
    velocity: vec2<f32>,
    diffusion: f32,
};

const AMOUNT_CHANNELS: u32 = 9u;
const NEIGHBOUR_SHARE: f32 = 0.25;
const EMPTY_VOLUME_UM: f32 = 1e-4;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var scratch_pig0: texture_storage_2d<rgba16float, read>;
@group(0) @binding(2) var scratch_pig1: texture_storage_2d<rgba16float, read>;
@group(0) @binding(3) var scratch_vol: texture_storage_2d<r32float, read>;
@group(0) @binding(4) var wet_pig0: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var wet_pig1: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var wet_vol: texture_storage_2d<r32float, write>;
@group(0) @binding(7) var scratch_motion: texture_storage_2d<rgba32float, read>;

fn inside_copied_region(pixel: vec2<i32>) -> bool {
    let offset = pixel - dab.canvas_origin;
    return all(offset >= vec2<i32>(0)) && all(offset < vec2<i32>(dab.canvas_size));
}

fn amounts_at(pixel: vec2<i32>) -> array<f32, 9> {
    let volume = textureLoad(scratch_vol, pixel).x;
    let low = textureLoad(scratch_pig0, pixel) * volume;
    let high = textureLoad(scratch_pig1, pixel) * volume;
    return array<f32, 9>(low.x, low.y, low.z, low.w, high.x, high.y, high.z, high.w, volume);
}

fn motion(pixel: vec2<i32>) -> Motion {
    if (!inside_copied_region(pixel)) {
        return Motion(vec2<f32>(0.0), 0.0);
    }
    let stored = textureLoad(scratch_motion, pixel);
    return Motion(stored.xy, stored.z);
}

fn outflow(velocity: vec2<f32>) -> f32 {
    return abs(velocity.x) + abs(velocity.y);
}

fn store_amounts(pixel: vec2<i32>, amounts: array<f32, 9>) {
    let volume = max(amounts[VOLUME_CHANNEL], 0.0);
    var fractions = array<f32, 8>();
    if (volume >= EMPTY_VOLUME_UM) {
        for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
            fractions[slot] = nearest_half(max(amounts[slot], 0.0) / volume);
        }
    }
    textureStore(wet_vol, pixel, vec4<f32>(volume, 0.0, 0.0, 0.0));
    textureStore(wet_pig0, pixel, vec4<f32>(fractions[0], fractions[1], fractions[2], fractions[3]));
    textureStore(wet_pig1, pixel, vec4<f32>(fractions[4], fractions[5], fractions[6], fractions[7]));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.spread_size.x || id.y >= dab.spread_size.y) {
        return;
    }
    let pixel = dab.spread_origin + vec2<i32>(id.xy);
    let own = motion(pixel);
    let own_amounts = amounts_at(pixel);
    var amounts = own_amounts;
    var changed = false;
    let kept = 1.0 - outflow(own.velocity);
    if (kept < 1.0) {
        changed = true;
        for (var channel = 0u; channel < AMOUNT_CHANNELS; channel++) {
            amounts[channel] = own_amounts[channel] * kept;
        }
    }
    let offsets = array<vec2<i32>, 4>(
        vec2<i32>(1, 0),
        vec2<i32>(-1, 0),
        vec2<i32>(0, 1),
        vec2<i32>(0, -1),
    );
    for (var side = 0u; side < 4u; side++) {
        let neighbour = pixel + offsets[side];
        let theirs = motion(neighbour);
        let inflow = max(dot(theirs.velocity, -vec2<f32>(offsets[side])), 0.0);
        let exchange = NEIGHBOUR_SHARE * min(own.diffusion, theirs.diffusion);
        if (inflow <= 0.0 && exchange <= 0.0) {
            continue;
        }
        changed = true;
        let their_amounts = amounts_at(neighbour);
        for (var channel = 0u; channel < AMOUNT_CHANNELS; channel++) {
            amounts[channel] += their_amounts[channel] * inflow
                + exchange * (their_amounts[channel] - own_amounts[channel]);
        }
    }
    if (changed) {
        store_amounts(pixel, amounts);
    }
}
