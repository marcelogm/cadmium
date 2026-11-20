// Every helper mirrors the serial function of the same role, term for term, so the two
// solves stay equivalent. Lane v owns vertex v, hinge v and contact row v (vertex v + 1).
// Every loop holding a workgroup barrier runs a workgroup-uniform count.

alias GroupVectors = array<array<vec3<f32>, BRISTLE_VERTICES>, BRISTLE_STRANDS_PER_GROUP>;
alias GroupHinges = array<array<vec3<f32>, BRISTLE_SEGMENTS>, BRISTLE_STRANDS_PER_GROUP>;
alias GroupRows = array<array<f32, BRISTLE_SEGMENTS>, BRISTLE_STRANDS_PER_GROUP>;
alias GroupFlags = array<array<u32, BRISTLE_SEGMENTS>, BRISTLE_STRANDS_PER_GROUP>;

var<workgroup> group_chain: GroupVectors;
var<workgroup> group_normals: GroupVectors;
var<workgroup> group_heights: array<array<f32, BRISTLE_VERTICES>, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_edges: GroupHinges;
var<workgroup> group_short: GroupFlags;
var<workgroup> group_turns: GroupHinges;
var<workgroup> group_compliances: GroupRows;
var<workgroup> group_rise_turns: GroupHinges;
var<workgroup> group_gaps: GroupRows;
var<workgroup> group_diagonal: GroupRows;
var<workgroup> group_forces: GroupRows;
var<workgroup> group_engaged: GroupFlags;
var<workgroup> group_added: GroupFlags;
var<workgroup> group_matrix: array<ContactMatrix, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_ridge: array<f32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_scale: array<f32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_solving: array<u32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_dropped: array<u32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_settled: array<u32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_unsettled: u32;
var<workgroup> group_lifting: array<u32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_lifted: u32;

struct Lane {
    group: u32,
    strand: u32,
    vertex: u32,
    bristle: u32,
    live: bool,
};

struct Stepped {
    at: vec3<f32>,
    force: f32,
};

fn lane_of(group: u32, local: u32) -> Lane {
    let strand = local / BRISTLE_VERTICES;
    let bristle = group * BRISTLE_STRANDS_PER_GROUP + strand;
    return Lane(group, strand, local % BRISTLE_VERTICES, bristle, bristle < dab.bristle_total);
}

fn leads(lane: Lane) -> bool {
    return lane.live && lane.vertex == 0u;
}

fn owns_hinge(lane: Lane) -> bool {
    return lane.live && lane.vertex < BRISTLE_SEGMENTS;
}

// mirrors held_chain, place_at_rest
fn held_position(lane: Lane, start: Pose) -> vec3<f32> {
    if dab.bristle_reset != 0u {
        var local = rest_chain_local(rests[lane.bristle]);
        return from_ferrule(local[lane.vertex], start);
    }
    return states[position_slot(lane.bristle, lane.vertex)].xyz;
}

// mirrors floors_under, floor_normals
fn place_vertex(lane: Lane, at: vec3<f32>, sampled: vec3<f32>, radius_px: f32) {
    var height = 0.0;
    var slope = vec2<f32>(0.0);
    if lane.vertex > 0u {
        height = swept_floor_px(sampled.xy, at.xy, radius_px);
        slope = floor_slope(at.xy, radius_px);
    }
    group_chain[lane.strand][lane.vertex] = at;
    group_heights[lane.strand][lane.vertex] = height;
    group_normals[lane.strand][lane.vertex] = normalize(vec3<f32>(-slope, 1.0));
}

// mirrors rested
fn place_height(lane: Lane, at: vec3<f32>, sampled: vec3<f32>, radius_px: f32, end_floor_px: f32) {
    var height = 0.0;
    if lane.vertex > 0u {
        height = swept_from_px(sampled.xy, at.xy, radius_px, end_floor_px);
    }
    group_chain[lane.strand][lane.vertex] = at;
    group_heights[lane.strand][lane.vertex] = height;
}

// mirrors rested
fn place_lifted_normal(lane: Lane) {
    if lane.vertex > 0u {
        group_heights[lane.strand][lane.vertex] -= FLOOR_SLACK_PX;
    }
    group_normals[lane.strand][lane.vertex] = vec3<f32>(0.0, 0.0, 1.0);
}

// mirrors edges_of
fn place_edge(lane: Lane) {
    let s = lane.strand;
    let edge = lane.vertex;
    let span = group_chain[s][edge + 1u] - group_chain[s][edge];
    let size = length(span);
    group_edges[s][edge] = span / max(size, MIN_SEGMENT_SPAN_PX);
    group_short[s][edge] = select(1u, 0u, size > MIN_SEGMENT_SPAN_PX);
}

// mirrors strand_shape; depends on the rest strand alone, so placed once per dab
fn place_compliance(lane: Lane, rest: BristleRest) {
    let segment_m = segment_length(rest) * METRES_PER_PX;
    group_compliances[lane.strand][lane.vertex] = hinge_compliance_per_m(lane.vertex) * segment_m / rest.stiffness;
}

// mirrors edges_of
fn mend_short_edges(strand: u32, clamp: vec3<f32>) {
    var previous = clamp;
    for (var edge = 0u; edge < BRISTLE_SEGMENTS; edge++) {
        if group_short[strand][edge] != 0u {
            group_edges[strand][edge] = previous;
        }
        previous = group_edges[strand][edge];
    }
}

// mirrors free_turns
fn free_turn(lane: Lane, shape: StrandShape) -> vec3<f32> {
    let s = lane.strand;
    let hinge = lane.vertex;
    var before = shape.clamp;
    if hinge > 0u {
        before = group_edges[s][hinge - 1u];
    }
    let curl = transported(shape.curl, shape.clamp, before);
    let rest = select(vec3<f32>(0.0), curl, hinge > 0u);
    let tip = group_chain[s][BRISTLE_VERTICES - 1u];
    let lever_m = (tip - group_chain[s][hinge]) * METRES_PER_PX;
    let loaded = group_compliances[s][hinge] * cross(lever_m, shape.tip_force_n);
    return rest - rotation_between(before, group_edges[s][hinge]) + loaded;
}

// mirrors solve_iteration
fn pushed_turn(strand: u32, hinge: u32, free: vec3<f32>) -> vec3<f32> {
    var turn = free;
    for (var pushed = hinge + 1u; pushed < BRISTLE_VERTICES; pushed++) {
        let force = group_forces[strand][pushed - 1u];
        let arm_m = (group_chain[strand][pushed] - group_chain[strand][hinge]) * METRES_PER_PX;
        turn += group_compliances[strand][hinge] * force * cross(arm_m, group_normals[strand][pushed]);
    }
    return turn;
}

// mirrors capped
fn cap_scale(strand: u32) -> f32 {
    var largest = 0.0;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        largest = max(largest, length(group_turns[strand][hinge]));
    }
    return MAX_HINGE_TURN_RAD / max(largest, MAX_HINGE_TURN_RAD);
}

// mirrors turn_chain; rebuilt by this lane alone, in the serial order
fn turned_position(strand: u32, vertex: u32, scale: f32, segment_px: f32) -> vec3<f32> {
    var at = group_chain[strand][0];
    var total = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    for (var edge = 0u; edge < vertex; edge++) {
        total = normalize(composed(total, quaternion_of(group_turns[strand][edge] * scale)));
        let direction = normalize(rotated(total, group_edges[strand][edge]));
        at = at + direction * segment_px;
    }
    return at;
}

// mirrors summed
fn summed_group_forces(strand: u32) -> f32 {
    var total = 0.0;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        total += group_forces[strand][row];
    }
    return total;
}

// mirrors solve_iteration, or lifted_onto when `lifting`
fn solve_step(lane: Lane, shape: StrandShape, lifting: bool) -> Stepped {
    let s = lane.strand;
    if owns_hinge(lane) {
        place_edge(lane);
    }
    workgroupBarrier();
    if leads(lane) {
        mend_short_edges(s, shape.clamp);
    }
    workgroupBarrier();
    var free = vec3<f32>(0.0);
    if owns_hinge(lane) && !lifting {
        free = free_turn(lane, shape);
    }
    if owns_hinge(lane) {
        group_turns[s][lane.vertex] = free;
    }
    workgroupBarrier();
    if owns_hinge(lane) {
        let vertex = lane.vertex + 1u;
        group_gaps[s][lane.vertex] = gap_of(s, vertex);
        group_diagonal[s][lane.vertex] = diagonal_of(s, vertex);
    }
    workgroupBarrier();
    if leads(lane) {
        open_contacts(s, !lifting || group_lifting[s] != 0u);
    }
    workgroupBarrier();
    if owns_hinge(lane) && group_solving[s] != 0u {
        for (var column = 0u; column < lane.vertex; column++) {
            group_matrix[s][lane.vertex][column] =
                group_compliance(s, lane.vertex + 1u, column + 1u);
        }
    }
    workgroupBarrier();
    settle_contacts(lane);
    if owns_hinge(lane) {
        group_turns[s][lane.vertex] = pushed_turn(s, lane.vertex, free);
    }
    workgroupBarrier();
    if leads(lane) {
        group_scale[s] = cap_scale(s);
    }
    workgroupBarrier();
    var stepped = Stepped(vec3<f32>(0.0), 0.0);
    if lane.live {
        stepped.at = turned_position(s, lane.vertex, group_scale[s], shape.segment_px);
    }
    if leads(lane) {
        stepped.force = summed_group_forces(s);
    }
    workgroupBarrier();
    return stepped;
}

@compute @workgroup_size(BRISTLE_SOLVE_WORKGROUP_SIZE)
fn solve(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    if group.x * BRISTLE_STRANDS_PER_GROUP >= dab.bristle_total {
        return;
    }
    let lane = lane_of(group.x, local);
    let start = pose_of(handle.start);
    let end = pose_of(handle.end);
    var at = vec3<f32>(0.0);
    var radius_px = 0.0;
    if lane.live {
        at = held_position(lane, start);
        radius_px = rests[lane.bristle].radius_px;
    }
    if owns_hinge(lane) {
        place_compliance(lane, rests[lane.bristle]);
    }
    var sampled = at;
    let substeps = max(dab.substeps, 1u);
    let iterations = max(substeps, MIN_DAB_ITERATIONS);
    var previous = start;
    var force = 0.0;
    var shape: StrandShape;
    for (var iteration = 0u; iteration < iterations; iteration++) {
        let pose = pose_between(start, end, min(f32(iteration + 1u) / f32(substeps), 1.0));
        if lane.live {
            shape = strand_shape(rests[lane.bristle], pose);
            at = carried(at, previous, pose);
            place_vertex(lane, at, sampled, radius_px);
            sampled = at;
        }
        previous = pose;
        workgroupBarrier();
        let stepped = solve_step(lane, shape, false);
        at = stepped.at;
        force = stepped.force;
    }
    // The disc floor each check reads at `at` is the one stored, while `at` stays put.
    var end_floor = 0.0;
    var floored = false;
    for (var correction = 0u; correction < FLOOR_CORRECTIONS; correction++) {
        if lane.live {
            end_floor = disc_floor_px(at.xy, radius_px);
            floored = true;
            place_height(lane, at, sampled, radius_px, end_floor);
            sampled = at;
        }
        workgroupBarrier();
        if leads(lane) {
            group_lifting[lane.strand] = select(0u, 1u, strand_under(lane.strand));
        }
        workgroupBarrier();
        if lane.strand == 0u && lane.vertex == 0u {
            group_lifted = lifting_strands(lane.group);
        }
        let lifting = lane.live && group_lifting[lane.strand] != 0u;
        if lifting {
            place_lifted_normal(lane);
        }
        if workgroupUniformLoad(&group_lifted) == 0u {
            break;
        }
        let stepped = solve_step(lane, shape, true);
        if lifting {
            at = stepped.at;
            force += stepped.force;
            floored = false;
        }
    }
    if lane.live {
        if !floored {
            end_floor = disc_floor_px(at.xy, radius_px);
        }
        states[position_slot(lane.bristle, lane.vertex)] = vec4<f32>(at, end_floor);
        if lane.vertex == 0u {
            normal_forces[lane.bristle] = force;
            hair_loads[lane.bristle] = FLAT_HAIR_LOAD;
        }
    }
}
