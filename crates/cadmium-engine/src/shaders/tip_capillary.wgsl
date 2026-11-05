const POOL_COUNT: u32 = MAX_BRISTLES;
const POOL_VALUES: u32 = POOL_COUNT * TIP_CHANNELS;
const POOL_START: u32 = 0u;
const SHARE_START: u32 = POOL_START + POOL_VALUES;
const REMAINDER_START: u32 = SHARE_START + POOL_VALUES;
const MEMBERS_START: u32 = REMAINDER_START + POOL_VALUES;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var<storage, read_write> tip: array<f32>;
@group(0) @binding(2) var<storage, read_write> tip_delta: array<atomic<i32>>;
@group(0) @binding(5) var<storage, read_write> pools: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read> owners: array<u32>;
@group(0) @binding(7) var<storage, read> ranks: array<u32>;

fn capillary_texel(id: vec3<u32>) -> vec2<u32> {
    return vec2<u32>(dab.tip_origin + vec2<i32>(id.xy));
}

fn owner_of(texel: vec2<u32>) -> u32 {
    return owners[texel.y * TIP_SIZE + texel.x];
}

fn rank_of(texel: vec2<u32>) -> u32 {
    return ranks[texel.y * TIP_SIZE + texel.x];
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
    let bristle = owner_of(texel);
    if (bristle == OUTSIDE_TIP) {
        return;
    }
    let base = tip_index(texel);
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        let amount = max(tip[base + channel], 0.0);
        let along_bristle = given(dab.capillary_rate, amount);
        if (along_bristle == 0u) {
            continue;
        }
        tip[base + channel] -= f32(along_bristle) / dab.capillary_scale;
        atomicAdd(&pools[POOL_START + bristle * TIP_CHANNELS + channel], along_bristle);
    }
    atomicAdd(&pools[MEMBERS_START + bristle], 1u);
}

@compute @workgroup_size(64)
fn settle(@builtin(global_invocation_id) id: vec3<u32>) {
    let pool = id.x;
    if (pool >= POOL_COUNT) {
        return;
    }
    let members = atomicExchange(&pools[MEMBERS_START + pool], 0u);
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

fn receive(pool: u32, rank: u32, base: u32) {
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        let value = pool * TIP_CHANNELS + channel;
        var received = atomicLoad(&pools[SHARE_START + value]);
        if (rank < atomicLoad(&pools[REMAINDER_START + value])) {
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
    let bristle = owner_of(texel);
    if (bristle == OUTSIDE_TIP) {
        return;
    }
    let base = tip_index(texel);
    receive(bristle, rank_of(texel), base);
}

const NEIGHBOUR_STEPS: array<vec2<i32>, 4> = array<vec2<i32>, 4>(
    vec2<i32>(1, 0),
    vec2<i32>(-1, 0),
    vec2<i32>(0, 1),
    vec2<i32>(0, -1),
);

fn foreign_neighbour(texel: vec2<u32>, step: vec2<i32>, bristle: u32) -> bool {
    let reached = vec2<i32>(texel) + step;
    if (any(reached < vec2<i32>(0)) || any(reached >= vec2<i32>(i32(TIP_SIZE)))) {
        return false;
    }
    let other = owner_of(vec2<u32>(reached));
    return other != OUTSIDE_TIP && other != bristle;
}

fn foreign_neighbours(texel: vec2<u32>, bristle: u32) -> u32 {
    var count = 0u;
    for (var side = 0u; side < 4u; side++) {
        count += select(0u, 1u, foreign_neighbour(texel, NEIGHBOUR_STEPS[side], bristle));
    }
    return count;
}

@compute @workgroup_size(8, 8)
fn wick_give(@builtin(global_invocation_id) id: vec3<u32>) {
    if (!in_window(id)) {
        return;
    }
    let texel = capillary_texel(id);
    let bristle = owner_of(texel);
    if (bristle == OUTSIDE_TIP) {
        return;
    }
    let sides = foreign_neighbours(texel, bristle);
    if (sides == 0u) {
        return;
    }
    let base = tip_index(texel);
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        let amount = max(tip[base + channel], 0.0);
        let held_q = i32(floor(amount * DELTA_SCALE));
        let each_q = min(i32(floor(dab.wicking_rate * amount * DELTA_SCALE)), held_q / i32(sides));
        if (each_q == 0) {
            continue;
        }
        tip[base + channel] -= f32(each_q * i32(sides)) / DELTA_SCALE;
        atomicStore(&tip_delta[base + channel], each_q);
    }
}

@compute @workgroup_size(8, 8)
fn wick_take(@builtin(global_invocation_id) id: vec3<u32>) {
    if (!in_window(id)) {
        return;
    }
    let texel = capillary_texel(id);
    let bristle = owner_of(texel);
    if (bristle == OUTSIDE_TIP) {
        return;
    }
    let base = tip_index(texel);
    for (var side = 0u; side < 4u; side++) {
        let step = NEIGHBOUR_STEPS[side];
        if (!foreign_neighbour(texel, step, bristle)) {
            continue;
        }
        let neighbour = tip_index(vec2<u32>(vec2<i32>(texel) + step));
        for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
            let received_q = atomicLoad(&tip_delta[neighbour + channel]);
            if (received_q != 0) {
                tip[base + channel] += f32(received_q) / DELTA_SCALE;
            }
        }
    }
}

@compute @workgroup_size(8, 8)
fn wick_clear(@builtin(global_invocation_id) id: vec3<u32>) {
    if (!in_window(id)) {
        return;
    }
    let base = tip_index(capillary_texel(id));
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        atomicStore(&tip_delta[base + channel], 0);
    }
}
