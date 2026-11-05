@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var<storage, read_write> tip: array<f32>;
@group(0) @binding(2) var<storage, read_write> tip_delta: array<atomic<i32>>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.tip_size.x || id.y >= dab.tip_size.y) {
        return;
    }
    let base = tip_index(vec2<u32>(dab.tip_origin) + id.xy);
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        let change = atomicExchange(&tip_delta[base + channel], 0);
        if (change == 0) {
            continue;
        }
        tip[base + channel] += f32(change) / DELTA_SCALE;
    }
}
