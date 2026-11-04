struct DrawUp {
    center: vec2<f32>,
    radius: f32,
    share: f32,
    room: f32,
    _pad: f32,
    origin: vec2<i32>,
    size: vec2<u32>,
};

struct Drawn {
    granted_share: f32,
    volume: f32,
    pigment: array<f32, 8>,
};

const SUM_CHANNELS: u32 = PIGMENT_CHANNELS + 1u;
const VOLUME_SUM: u32 = PIGMENT_CHANNELS;

@group(0) @binding(0) var<uniform> request: DrawUp;
@group(0) @binding(1) var wet_pig0: texture_storage_2d<rgba16float, read>;
@group(0) @binding(2) var wet_pig1: texture_storage_2d<rgba16float, read>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, read_write>;
@group(0) @binding(4) var<storage, read_write> drawn: Drawn;

var<workgroup> partial: array<f32, DRAW_UP_INVOCATIONS>;
var<workgroup> sums: array<f32, SUM_CHANNELS>;

fn under_brush(pixel: vec2<i32>) -> bool {
    return distance(vec2<f32>(pixel) + vec2<f32>(0.5), request.center) <= request.radius;
}

@compute @workgroup_size(DRAW_UP_INVOCATIONS)
fn measure(@builtin(local_invocation_index) local: u32) {
    let side = vec2<i32>(request.size);
    var mine = array<f32, SUM_CHANNELS>();
    for (var index = i32(local); index < side.x * side.y; index += i32(DRAW_UP_INVOCATIONS)) {
        let pixel = request.origin + vec2<i32>(index % side.x, index / side.x);
        if (!under_brush(pixel)) {
            continue;
        }
        let volume = max(textureLoad(wet_vol, pixel).x, 0.0);
        let low = textureLoad(wet_pig0, pixel);
        let high = textureLoad(wet_pig1, pixel);
        for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
            mine[slot] += slot_lane(low, high, slot) * volume;
        }
        mine[VOLUME_SUM] += volume;
    }
    for (var channel = 0u; channel < SUM_CHANNELS; channel++) {
        partial[local] = mine[channel];
        workgroupBarrier();
        for (var stride = DRAW_UP_INVOCATIONS / 2u; stride > 0u; stride /= 2u) {
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
    let wanted = sums[VOLUME_SUM] * request.share;
    var granted = 0.0;
    if (wanted > 0.0 && request.room > 0.0) {
        granted = request.share * min(1.0, request.room / wanted);
    }
    drawn.granted_share = granted;
    drawn.volume = sums[VOLUME_SUM] * granted;
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        drawn.pigment[slot] = sums[slot] * granted;
    }
}

@compute @workgroup_size(8, 8)
fn take(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= request.size.x || id.y >= request.size.y) {
        return;
    }
    let pixel = request.origin + vec2<i32>(id.xy);
    if (!under_brush(pixel)) {
        return;
    }
    let volume = textureLoad(wet_vol, pixel).x;
    if (volume <= 0.0) {
        return;
    }
    textureStore(wet_vol, pixel, vec4<f32>(volume * (1.0 - drawn.granted_share), 0.0, 0.0, 0.0));
}
