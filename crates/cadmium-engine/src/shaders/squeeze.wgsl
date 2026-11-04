struct Squeeze {
    segment_start: vec2<f32>,
    segment_end: vec2<f32>,
    radius: f32,
    thickness_um: f32,
    max_um: f32,
    slot: u32,
    origin: vec2<i32>,
    size: vec2<u32>,
};

const EMPTY_VOLUME_UM: f32 = 1e-4;

@group(0) @binding(0) var<uniform> squeeze: Squeeze;
@group(0) @binding(1) var wet_pig0: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(2) var wet_pig1: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, read_write>;

fn distance_to_segment(point: vec2<f32>) -> f32 {
    let along = squeeze.segment_end - squeeze.segment_start;
    let length_squared = max(dot(along, along), 1e-6);
    let t = clamp(dot(point - squeeze.segment_start, along) / length_squared, 0.0, 1.0);
    return distance(point, squeeze.segment_start + t * along);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= squeeze.size.x || id.y >= squeeze.size.y) {
        return;
    }
    let pixel = squeeze.origin + vec2<i32>(id.xy);
    if (distance_to_segment(vec2<f32>(pixel) + vec2<f32>(0.5)) > squeeze.radius) {
        return;
    }
    let volume = max(textureLoad(wet_vol, pixel).x, 0.0);
    let added = min(squeeze.thickness_um, max(squeeze.max_um - volume, 0.0));
    if (added <= 0.0) {
        return;
    }
    let total = volume + added;
    let low = textureLoad(wet_pig0, pixel);
    let high = textureLoad(wet_pig1, pixel);
    var fractions = array<f32, 8>();
    if (total >= EMPTY_VOLUME_UM) {
        for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
            let squeezed = select(0.0, added, slot == squeeze.slot);
            let amount = max(slot_lane(low, high, slot), 0.0) * volume + squeezed;
            fractions[slot] = nearest_half(amount / total);
        }
        fractions = within_whole_volume(fractions);
    }
    textureStore(wet_vol, pixel, vec4<f32>(total, 0.0, 0.0, 0.0));
    textureStore(wet_pig0, pixel, vec4<f32>(fractions[0], fractions[1], fractions[2], fractions[3]));
    textureStore(wet_pig1, pixel, vec4<f32>(fractions[4], fractions[5], fractions[6], fractions[7]));
}
