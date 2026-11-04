const ROW_COUNT: u32 = TIP_SIZE;
const WHOLE_TIP: u32 = ROW_COUNT;
const POOL_COUNT: u32 = ROW_COUNT + 1u;
const POOL_VALUES: u32 = POOL_COUNT * TIP_CHANNELS;
const POOL_START: u32 = 0u;
const SHARE_START: u32 = POOL_START + POOL_VALUES;
const REMAINDER_START: u32 = SHARE_START + POOL_VALUES;
const MEMBERS_START: u32 = REMAINDER_START + POOL_VALUES;
const TICKET_START: u32 = MEMBERS_START + POOL_COUNT;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var<storage, read_write> tip: array<f32>;
@group(0) @binding(5) var<storage, read_write> pools: array<atomic<u32>>;

fn capillary_texel(id: vec3<u32>) -> vec2<u32> {
    return vec2<u32>(dab.tip_origin + vec2<i32>(id.xy));
}

fn texel_offset(texel: vec2<u32>) -> vec2<f32> {
    return vec2<f32>(vec2<i32>(texel) - vec2<i32>(TIP_HALF));
}

fn row_of(offset: vec2<f32>) -> u32 {
    let row = i32(round(across_heading(offset))) + i32(TIP_HALF);
    return u32(clamp(row, 0, i32(ROW_COUNT) - 1));
}

fn in_window(id: vec3<u32>) -> bool {
    return id.x < dab.tip_size.x && id.y < dab.tip_size.y;
}

fn given(rate: f32, amount: f32) -> u32 {
    return u32(floor(rate * amount * dab.capillary_scale));
}

@compute @workgroup_size(8, 8)
fn gather(@builtin(global_invocation_id) id: vec3<u32>) {
    if (!in_window(id)) {
        return;
    }
    let texel = capillary_texel(id);
    let offset = texel_offset(texel);
    if (!inside_outline(offset)) {
        return;
    }
    let base = tip_index(texel);
    let row = row_of(offset);
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        let amount = max(tip[base + channel], 0.0);
        let along_row = given(dab.capillary_rate, amount);
        let across_rows = given(dab.cross_capillary_rate, amount);
        if (along_row + across_rows == 0u) {
            continue;
        }
        tip[base + channel] -= f32(along_row + across_rows) / dab.capillary_scale;
        atomicAdd(&pools[POOL_START + row * TIP_CHANNELS + channel], along_row);
        atomicAdd(&pools[POOL_START + WHOLE_TIP * TIP_CHANNELS + channel], across_rows);
    }
    atomicAdd(&pools[MEMBERS_START + row], 1u);
    atomicAdd(&pools[MEMBERS_START + WHOLE_TIP], 1u);
}

@compute @workgroup_size(64)
fn settle(@builtin(global_invocation_id) id: vec3<u32>) {
    let pool = id.x;
    if (pool >= POOL_COUNT) {
        return;
    }
    let members = atomicExchange(&pools[MEMBERS_START + pool], 0u);
    atomicStore(&pools[TICKET_START + pool], 0u);
    if (members == 0u) {
        return;
    }
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        let value = pool * TIP_CHANNELS + channel;
        let pooled = atomicExchange(&pools[POOL_START + value], 0u);
        let share = pooled / members;
        atomicStore(&pools[SHARE_START + value], share);
        atomicStore(&pools[REMAINDER_START + value], pooled - share * members);
    }
}

fn receive(pool: u32, base: u32) {
    let ticket = atomicAdd(&pools[TICKET_START + pool], 1u);
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        let value = pool * TIP_CHANNELS + channel;
        var received = atomicLoad(&pools[SHARE_START + value]);
        if (ticket < atomicLoad(&pools[REMAINDER_START + value])) {
            received += 1u;
        }
        if (received > 0u) {
            tip[base + channel] += f32(received) / dab.capillary_scale;
        }
    }
}

@compute @workgroup_size(8, 8)
fn distribute(@builtin(global_invocation_id) id: vec3<u32>) {
    if (!in_window(id)) {
        return;
    }
    let texel = capillary_texel(id);
    let offset = texel_offset(texel);
    if (!inside_outline(offset)) {
        return;
    }
    let base = tip_index(texel);
    receive(row_of(offset), base);
    receive(WHOLE_TIP, base);
}
