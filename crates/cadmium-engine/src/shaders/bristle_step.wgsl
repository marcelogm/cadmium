@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var<storage, read> rests: array<BristleRest>;
@group(0) @binding(2) var<storage, read_write> states: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> handle: HandleState;
@group(0) @binding(4) var<uniform> tuning: BristleTuning;
@group(0) @binding(5) var dry_height: texture_storage_2d<r32float, read>;
@group(0) @binding(6) var wet_vol: texture_storage_2d<r32float, read>;
@group(0) @binding(7) var<storage, read_write> hair_loads: array<f32>;
@group(0) @binding(8) var<storage, read_write> normal_forces: array<f32>;

// Mirrors MIN_AXIS_RISE in engine/bristles/handle.rs.
const MIN_AXIS_RISE: f32 = 0.0175;
/// Fewest solve iterations a dab runs, so a dab with no travel still settles its strands.
const MIN_DAB_ITERATIONS: u32 = 2u;
/// Every hair carries the typical load until phase 4 of #62 hands Exchange the normal force.
const FLAT_HAIR_LOAD: f32 = 1.0;

fn handle_axis() -> vec3<f32> {
    return vec3<f32>(dab.handle_axis_x, dab.handle_axis_y, dab.handle_axis_z);
}

fn pose_at_height(height_px: f32) -> Pose {
    let axis = handle_axis();
    let over_nib = dab.center + axis.xy * (height_px / max(axis.z, MIN_AXIS_RISE));
    return Pose(vec3<f32>(over_nib, height_px), axis, dab.bristle_orientation);
}

fn vertex_touch_px(index: u32, placed: Pose) -> f32 {
    let bristle = index / BRISTLE_SEGMENTS;
    let vertex = index % BRISTLE_SEGMENTS + 1u;
    let rest = rests[bristle];
    var local = rest_chain_local(rest);
    let at = from_ferrule(local[vertex], placed);
    return disc_floor_px(at.xy, rest.radius_px) - (at.z - placed.ferrule.z);
}

/// The height at which the brush's rest shape, in the dab's pose, first touches
/// (spec §3): p = 0 lands only the lowest tip, whatever the hairs are doing now. The max is
/// exact, so splitting it per vertex does not change the result.
fn touch_height_px(thread: u32) -> f32 {
    let placed = pose_at_height(dab.handle_rest_height_px);
    var touch = -NO_FLOOR_PX;
    let vertices = dab.bristle_total * BRISTLE_SEGMENTS;
    for (var index = thread; index < vertices; index += BRISTLE_WORKGROUP_SIZE) {
        touch = max(touch, vertex_touch_px(index, placed));
    }
    let bounds = workgroup_range(thread, vec2<f32>(NO_FLOOR_PX, touch));
    return select(bounds.y, 0.0, bounds.y <= -NO_FLOOR_PX);
}

fn ferrule_clearance_px() -> f32 {
    return dab.ferrule_radius_px * length(handle_axis().xy);
}

fn held_pose(offset_px: f32, touch_px: f32) -> Pose {
    return pose_at_height(max(touch_px + offset_px, ferrule_clearance_px()));
}

/// Where the dab starts: the last dab's pose, or, after a reset, just touching.
fn start_pose(touch_px: f32) -> Pose {
    if dab.bristle_reset != 0u {
        return held_pose(max(dab.handle_offset_px, 0.0), touch_px);
    }
    return pose_of(handle.end);
}

fn place_at_rest(bristle: u32, pose: Pose) {
    let rest = rests[bristle];
    let local = rest_chain_local(rest);
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        let at = from_ferrule(local[vertex], pose);
        states[position_slot(bristle, vertex)] = vec4<f32>(at, disc_floor_px(at.xy, rest.radius_px));
    }
}

fn held_chain(bristle: u32) -> Chain {
    var chain: Chain;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        chain[vertex] = states[position_slot(bristle, vertex)].xyz;
    }
    return chain;
}

/// Stores the chain with the floor the solve rests each vertex on, so Contact's clearance
/// matches what the solve enforced.
fn store_chain(bristle: u32, chain: Chain) {
    let radius_px = rests[bristle].radius_px;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        let at = chain[vertex];
        states[position_slot(bristle, vertex)] = vec4<f32>(at, disc_floor_px(at.xy, radius_px));
    }
}

/// Moves `chain` rigidly with the ferrule from pose `start` to pose `end`.
fn carry(chain: ptr<function, Chain>, start: Pose, end: Pose) {
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        (*chain)[vertex] = carried((*chain)[vertex], start, end);
    }
}

/// Moves the strand along the dab's ferrule path, one solve iteration per substep, and
/// returns the canvas's normal force at the end, N.
fn solved_dab(bristle: u32, start: Pose, end: Pose) -> f32 {
    var chain = held_chain(bristle);
    var sampled = chain;
    let radius_px = rests[bristle].radius_px;
    let substeps = max(dab.substeps, 1u);
    let iterations = max(substeps, MIN_DAB_ITERATIONS);
    var previous = start;
    var force = 0.0;
    for (var iteration = 0u; iteration < iterations; iteration++) {
        let pose = pose_between(start, end, min(f32(iteration + 1u) / f32(substeps), 1.0));
        carry(&chain, previous, pose);
        previous = pose;
        let shape = strand_shape(rests[bristle], pose);
        let floors = floors_under(sampled, chain, radius_px);
        sampled = chain;
        force = summed(solve_iteration(&chain, shape, floors));
    }
    store_chain(bristle, chain);
    return force;
}

/// Measures the dab's touch height across all strands and records its ferrule path: the
/// start is read from the last dab's end before the end is overwritten.
@compute @workgroup_size(BRISTLE_WORKGROUP_SIZE)
fn touch(@builtin(local_invocation_index) thread: u32) {
    let touch_px = touch_height_px(thread);
    if thread == 0u {
        let start = start_pose(touch_px);
        let end = held_pose(dab.handle_offset_px, touch_px);
        handle = HandleState(stored_pose(start), stored_pose(end));
    }
}

/// Solves one strand along the path `touch` recorded. No barriers, so the workgroup can be
/// small and each thread gets the registers the solve needs.
@compute @workgroup_size(BRISTLE_SOLVE_WORKGROUP_SIZE)
fn solve_reference(@builtin(global_invocation_id) id: vec3<u32>) {
    solve_serially(id.x);
}

fn solve_serially(bristle: u32) {
    if bristle >= dab.bristle_total {
        return;
    }
    let start = pose_of(handle.start);
    let end = pose_of(handle.end);
    if dab.bristle_reset != 0u {
        place_at_rest(bristle, start);
    }
    normal_forces[bristle] = solved_dab(bristle, start, end);
    hair_loads[bristle] = FLAT_HAIR_LOAD;
}
