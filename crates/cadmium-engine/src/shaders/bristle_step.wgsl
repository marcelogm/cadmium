@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var<storage, read> rests: array<BristleRest>;
@group(0) @binding(2) var<storage, read_write> states: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> handle: HandleState;
@group(0) @binding(4) var<storage, read> tip: array<f32>;
@group(0) @binding(5) var<storage, read> clusters: array<u32>;
@group(0) @binding(6) var<uniform> tuning: BristleTuning;
@group(0) @binding(7) var dry_height: texture_storage_2d<r32float, read>;
@group(0) @binding(8) var wet_vol: texture_storage_2d<r32float, read>;
@group(0) @binding(9) var<storage, read_write> hair_loads: array<f32>;

struct Travel {
    start: Pose,
    end: Pose,
    substeps: u32,
    seconds: f32,
    previous_seconds: f32,
    velocity_change: vec3<f32>,
};

struct Motion {
    start: Pose,
    end: Pose,
    seconds: f32,
    previous_seconds: f32,
    velocity_change: vec3<f32>,
};

fn current_pose(base_px: f32) -> Pose {
    return Pose(
        vec3<f32>(dab.center, dab.handle_height_px + base_px),
        vec3<f32>(dab.handle_axis_x, dab.handle_axis_y, dab.handle_axis_z),
        dab.bristle_orientation,
    );
}

fn lifted_pose(base_px: f32) -> Pose {
    var lifted = current_pose(base_px);
    lifted.ferrule.z = dab.handle_rest_height_px + base_px;
    return lifted;
}

fn starting_pose(base_px: f32) -> Pose {
    if dab.bristle_reset != 0u {
        return lifted_pose(base_px);
    }
    return Pose(handle.ferrule, handle.axis, handle.orientation);
}

fn hung_vertex(rest: BristleRest, pose: Pose, wetness: f32, vertex: u32) -> vec3<f32> {
    let lean = resting_lean(rest, gathering(wetness));
    let step = hanging_along(lean, pose) * segment_length(rest);
    return root_position(rest, pose) + step * f32(vertex);
}

fn hang_at_rest(bristle: u32, pose: Pose, wetness: f32) {
    let rest = rests[bristle];
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        let at = hung_vertex(rest, pose, wetness, vertex);
        let previous = vec4<f32>(at, 0.0);
        states[position_slot(bristle, vertex)] = vec4<f32>(at, surface_px(at.xy));
        states[previous_slot(bristle, vertex)] = previous;
        states[anchor_slot(bristle, vertex)] = vec4<f32>(0.0);
    }
    states[bend_slot(bristle)] = vec4<f32>(0.0);
}

fn resting_floor_px(bristle: u32) -> f32 {
    let band_px = rests[bristle].radius_px;
    var support_px = states[position_slot(bristle, BRISTLE_VERTICES - 1u)].w;
    var touching = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let at = states[position_slot(bristle, vertex)];
        if at.z - at.w <= band_px {
            support_px = select(at.w, max(support_px, at.w), touching);
            touching = true;
        }
    }
    return support_px;
}

fn support_floor_px(bristle: u32, wetness: f32) -> f32 {
    if dab.bristle_reset != 0u {
        let tip = hung_vertex(rests[bristle], lifted_pose(0.0), wetness, BRISTLE_VERTICES - 1u);
        return surface_px(tip.xy);
    }
    return resting_floor_px(bristle);
}

fn record_owned_loads(thread: u32, base_px: f32) {
    for (var share = 0u; share < BRISTLES_PER_THREAD; share++) {
        let bristle = owned_bristle(thread, share);
        if bristle < dab.bristle_total {
            hair_loads[bristle] = hair_load(resting_floor_px(bristle), base_px);
        }
    }
}

fn heading_of(offset: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let span = length(offset);
    let heading = offset / max(span, MIN_SEGMENT_SPAN_PX);
    return select(fallback, heading, span > MIN_SEGMENT_SPAN_PX);
}

fn keep_segment_lengths(chain: ptr<function, Chain>, segment: f32, hanging: vec3<f32>) {
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let parent = (*chain)[vertex - 1u];
        (*chain)[vertex] = parent + heading_of((*chain)[vertex] - parent, hanging) * segment;
    }
}

fn bend_share(rest: BristleRest, vertex: u32) -> f32 {
    let along = f32(vertex - 1u) / f32(BRISTLE_VERTICES - 2u);
    let taper = 1.0 - (1.0 - BRISTLE_TIP_STIFFNESS_SHARE) * along;
    return clamp(dab.bristle_stiffness * rest.stiffness * taper, 0.0, 1.0);
}

fn bend_toward_rest(chain: ptr<function, Chain>, rest: BristleRest, hanging: vec3<f32>) {
    let segment = segment_length(rest);
    var heading = hanging;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let parent = (*chain)[vertex - 1u];
        let bent = mix((*chain)[vertex], parent + heading * segment, bend_share(rest, vertex));
        (*chain)[vertex] = bent;
        heading = heading_of(bent - parent, heading);
    }
}

fn solve(
    chain: ptr<function, Chain>,
    anchors: ptr<function, Anchors>,
    rest: BristleRest,
    hanging: vec3<f32>,
) -> Floors {
    let floors = floors_under(*chain);
    for (var iteration = 0u; iteration < BRISTLE_SOLVER_ITERATIONS; iteration++) {
        keep_segment_lengths(chain, segment_length(rest), hanging);
        bend_toward_rest(chain, rest, hanging);
        touch_canvas(chain, anchors, floors);
    }
    return floors;
}

fn local_velocity(at: vec3<f32>, follow: vec3<f32>, motion: Motion) -> vec3<f32> {
    if motion.seconds <= 0.0 {
        return vec3<f32>(0.0);
    }
    var carried = vec3<f32>(0.0);
    if motion.previous_seconds > 0.0 {
        carried = (at - follow) / motion.previous_seconds;
    }
    return carried - dab.bristle_damping * motion.velocity_change;
}

fn predicted(bristle: u32, carry: vec3<f32>, motion: Motion) -> Chain {
    var chain: Chain;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        let at = states[position_slot(bristle, vertex)].xyz;
        let follow = states[previous_slot(bristle, vertex)].xyz;
        chain[vertex] = at + carry + local_velocity(at, follow, motion) * motion.seconds;
        states[previous_slot(bristle, vertex)] = vec4<f32>(at + carry, 0.0);
    }
    return chain;
}

fn held_anchors(bristle: u32) -> Anchors {
    var anchors: Anchors;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        anchors[vertex] = states[anchor_slot(bristle, vertex)];
    }
    return anchors;
}

fn store_positions(bristle: u32, chain: Chain, floors: Floors) {
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        states[position_slot(bristle, vertex)] = vec4<f32>(chain[vertex], floors[vertex]);
    }
}

fn store(bristle: u32, chain: Chain, anchors: Anchors, floors: Floors, seconds: f32) {
    store_positions(bristle, chain, floors);
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        states[anchor_slot(bristle, vertex)] = anchors[vertex];
        if seconds <= 0.0 {
            states[previous_slot(bristle, vertex)] = vec4<f32>(chain[vertex], 0.0);
        }
    }
}

fn advance(bristle: u32, motion: Motion, wetness: f32) {
    let rest = rests[bristle];
    let root = root_position(rest, motion.end);
    var chain = predicted(bristle, root - root_position(rest, motion.start), motion);
    var anchors = held_anchors(bristle);
    chain[0] = root;
    let bend = states[bend_slot(bristle)].xy;
    let lean = resting_lean(rest, gathering(wetness));
    let floors = solve(&chain, &anchors, rest, hanging_along(lean + bend, motion.end));
    let span = chain[BRISTLE_VERTICES - 1u] - root;
    let kept = remembered_bend(bend, lean, span, motion.end, motion.seconds);
    states[bend_slot(bristle)] = vec4<f32>(kept, 0.0, 0.0);
    store(bristle, chain, anchors, floors, motion.seconds);
}

fn handle_velocity(start: Pose, end: Pose) -> vec3<f32> {
    if dab.dt_seconds <= 0.0 {
        return vec3<f32>(0.0);
    }
    return (end.ferrule - start.ferrule) / dab.dt_seconds;
}

fn owned_bristle(thread: u32, share: u32) -> u32 {
    return thread + share * BRISTLE_WORKGROUP_SIZE;
}

fn remember_pose(pose: Pose, substep_seconds: f32, velocity: vec3<f32>) {
    handle = HandleState(
        pose.ferrule,
        substep_seconds,
        pose.axis,
        0.0,
        velocity,
        0.0,
        pose.orientation,
        vec2<f32>(0.0),
    );
}

fn hang_owned_at_rest(thread: u32, pose: Pose, wetness: Wetness) {
    for (var share = 0u; share < BRISTLES_PER_THREAD; share++) {
        let bristle = owned_bristle(thread, share);
        if bristle < dab.bristle_total {
            hang_at_rest(bristle, pose, wetness[share]);
        }
    }
}

fn motion_of(travel: Travel, substep: u32) -> Motion {
    let first = substep == 0u;
    let count = f32(travel.substeps);
    return Motion(
        pose_between(travel.start, travel.end, f32(substep) / count),
        pose_between(travel.start, travel.end, f32(substep + 1u) / count),
        travel.seconds,
        select(travel.seconds, travel.previous_seconds, first),
        select(vec3<f32>(0.0), travel.velocity_change, first),
    );
}

fn advance_owned(thread: u32, motion: Motion, wetness: Wetness) {
    for (var share = 0u; share < BRISTLES_PER_THREAD; share++) {
        let bristle = owned_bristle(thread, share);
        if bristle < dab.bristle_total {
            advance(bristle, motion, wetness[share]);
        }
    }
}

@compute @workgroup_size(BRISTLE_WORKGROUP_SIZE)
fn step(@builtin(local_invocation_index) thread: u32) {
    let resetting = dab.bristle_reset != 0u;
    let wetness = owned_wetness(thread);
    let base_px = resting_base_px(thread, wetness);
    let start = starting_pose(base_px);
    let end = current_pose(base_px);
    let velocity = handle_velocity(start, end);
    let substeps = max(dab.substeps, 1u);
    let travel = Travel(
        start,
        end,
        substeps,
        dab.dt_seconds / f32(substeps),
        select(handle.substep_seconds, 0.0, resetting),
        velocity - select(handle.velocity, vec3<f32>(0.0), resetting),
    );
    if resetting {
        hang_owned_at_rest(thread, start, wetness);
    }
    storageBarrier();
    for (var substep = 0u; substep < substeps; substep++) {
        advance_owned(thread, motion_of(travel, substep), wetness);
        storageBarrier();
        clump_owned(thread, wetness);
    }
    record_owned_loads(thread, base_px);
    if thread == 0u {
        remember_pose(end, travel.seconds, velocity);
    }
}
