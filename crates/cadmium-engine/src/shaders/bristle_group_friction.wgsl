// The grouped solve's friction: kinetic forces, the cones and the anchors, run per strand
// by lane 0 or by the lane that owns the hinge or vertex.

var<workgroup> group_anchors: array<Anchors, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_kinetic: GroupVectors;
var<workgroup> group_sliding: array<array<u32, BRISTLE_VERTICES>, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_slid: array<u32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_regap: array<u32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_holding: array<u32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_step_forces: GroupRowValues;

// mirrors kinetic_turns
fn kinetic_turn(strand: u32, hinge: u32, free: vec3<f32>) -> vec3<f32> {
    var turn = free;
    for (var vertex = hinge + 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let force = group_kinetic[strand][vertex];
        if any(force != vec3<f32>(0.0)) {
            let arm_m = (group_chain[strand][vertex] - group_chain[strand][hinge]) * METRES_PER_PX;
            turn += group_compliances[strand][hinge] * cross(arm_m, force);
        }
    }
    return turn;
}

// the serial solve inlines this sum in slipped and clamp_to_cones
fn held_friction(strand: u32, row: u32) -> vec3<f32> {
    return group_forces[strand][row + 1u] * group_directions[strand][row + 1u]
        + group_forces[strand][row + 2u] * group_directions[strand][row + 2u];
}

// mirrors slipped
fn group_slipped(strand: u32, friction: Friction) -> bool {
    var any_slipped = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        if group_engaged[strand][row + 1u] != 0u || group_engaged[strand][row + 2u] != 0u {
            let held = held_friction(strand, row);
            let size = length(held);
            let anchor = group_anchors[strand][vertex];
            let load = max(group_forces[strand][row], 0.0);
            let lagged = abs(anchor.w);
            let cone = cone_mu(friction, anchor);
            let pull = group_directions[strand][row + 1u];
            let along = select(vec3<f32>(0.0), held / size, size > 0.0);
            let pulling = group_forces[strand][row] < 0.0;
            if pulling || (size > 0.0 && size > anisotropic_mu(cone, friction, pull, -along) * load) {
                group_engaged[strand][row + 1u] = 0u;
                group_engaged[strand][row + 2u] = 0u;
                group_kinetic[strand][vertex] = anisotropic_mu(friction.kinetic_mu, friction, pull, -along) * lagged * along;
                group_sliding[strand][vertex] = 1u;
                any_slipped = true;
                let below = normal_row(vertex + 1u);
                if vertex + 1u < BRISTLE_VERTICES && group_engaged[strand][below + 2u] != 0u {
                    group_engaged[strand][below + 1u] = 1u;
                }
            }
        }
    }
    return any_slipped;
}

// mirrors let_go
fn group_let_go(strand: u32) -> bool {
    var any_cleared = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        if group_sliding[strand][vertex] != 0u && group_engaged[strand][normal_row(vertex)] == 0u {
            group_kinetic[strand][vertex] = vec3<f32>(0.0);
            group_sliding[strand][vertex] = 0u;
            any_cleared = true;
        }
    }
    return any_cleared;
}

// mirrors clamp_to_cones
fn clamp_group_to_cones(strand: u32, friction: Friction) {
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let held = held_friction(strand, row);
        let size = length(held);
        if size > 0.0 {
            let pull = group_directions[strand][row + 1u];
            let cone = anisotropic_mu(cone_mu(friction, group_anchors[strand][vertex]), friction, pull, -held / size);
            let bound = cone * max(group_forces[strand][row], 0.0);
            let scale = min(bound / size, 1.0);
            group_forces[strand][row + 1u] *= scale;
            group_forces[strand][row + 2u] *= scale;
        }
    }
}

// mirrors contact_forces' end
fn close_contacts(strand: u32, friction: Friction) {
    if group_settled[strand] == 0u {
        group_active_forces(strand);
        clamp_group_to_cones(strand, friction);
    }
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let engaged = group_engaged[strand][row] != 0u;
        group_forces[strand][row] = select(0.0, max(group_forces[strand][row], 0.0), engaged);
        for (var tangent = 1u; tangent < ROW_KINDS; tangent++) {
            let kept = engaged && group_engaged[strand][row + tangent] != 0u;
            group_engaged[strand][row + tangent] = select(0u, 1u, kept);
            group_forces[strand][row + tangent] = select(0.0, group_forces[strand][row + tangent], kept);
        }
        if !engaged {
            group_kinetic[strand][vertex] = vec3<f32>(0.0);
            group_sliding[strand][vertex] = 0u;
        }
    }
}

// mirrors pushed_step's turns: the kinetic turn, then every row in order
fn pushed_turn(strand: u32, hinge: u32, free: vec3<f32>) -> vec3<f32> {
    var turn = kinetic_turn(strand, hinge, free);
    for (var row = normal_row(hinge + 1u); row < MAX_CONTACT_ROWS; row++) {
        let force = group_forces[strand][row];
        if force != 0.0 {
            let pushed = row_vertex(row);
            let arm_m = (group_chain[strand][pushed] - group_chain[strand][hinge]) * METRES_PER_PX;
            turn += group_compliances[strand][hinge] * force * cross(arm_m, group_directions[strand][row]);
        }
    }
    return turn;
}

// mirrors next_anchors, for the vertex at `at` after the step
fn next_anchor(strand: u32, vertex: u32, at: vec3<f32>) -> vec4<f32> {
    if vertex == 0u {
        return vec4<f32>(0.0);
    }
    let row = normal_row(vertex);
    let load = group_forces[strand][row];
    if !(group_engaged[strand][row] != 0u && load > 0.0) {
        return vec4<f32>(0.0);
    }
    if group_sliding[strand][vertex] != 0u {
        return vec4<f32>(at, -load);
    }
    if group_engaged[strand][row + 1u] != 0u || group_engaged[strand][row + 2u] != 0u {
        return vec4<f32>(group_anchors[strand][vertex].xyz, load);
    }
    return vec4<f32>(at, load);
}

// mirrors any_engaged
fn group_any_engaged(strand: u32) -> bool {
    var any_row = false;
    for (var row = 0u; row < MAX_CONTACT_ROWS; row++) {
        any_row = any_row || group_engaged[strand][row] != 0u;
    }
    return any_row;
}

// mirrors held_back, from the lane's vertex at `at` after the step; the step's forces are
// kept for the anchors and the reported load
fn held_back_position(lane: Lane, at: vec3<f32>, shape: StrandShape) -> vec3<f32> {
    let s = lane.strand;
    if lane.live {
        group_chain[s][lane.vertex] = at;
    }
    workgroupBarrier();
    if owns_hinge(lane) {
        place_edge(lane);
        place_directions(s, lane.vertex + 1u);
        group_turns[s][lane.vertex] = vec3<f32>(0.0);
    }
    workgroupBarrier();
    if leads(lane) {
        mend_short_edges(s, shape.clamp);
        group_holding[s] = select(0u, 1u, group_any_engaged(s));
        group_step_forces[s] = group_forces[s];
    }
    workgroupBarrier();
    let holding = lane.live && group_holding[s] != 0u;
    if holding && lane.vertex < BRISTLE_SEGMENTS {
        place_gaps(s, lane.vertex + 1u);
        place_diagonal(s, lane.vertex + 1u);
    }
    workgroupBarrier();
    if holding && lane.vertex == 0u {
        group_ridge[s] = group_ridge_of(s);
    }
    if holding && lane.vertex < BRISTLE_SEGMENTS {
        place_matrix_rows(s, lane.vertex + 1u);
    }
    workgroupBarrier();
    if holding && lane.vertex == 0u {
        group_active_forces(s);
    }
    workgroupBarrier();
    if holding && lane.vertex < BRISTLE_SEGMENTS {
        group_turns[s][lane.vertex] = rise_turn(s, lane.vertex);
    }
    workgroupBarrier();
    if holding && lane.vertex == 0u {
        group_scale[s] = cap_scale(s);
    }
    workgroupBarrier();
    var held = at;
    if holding {
        held = turned_position(s, lane.vertex, group_scale[s], shape.segment_px);
    }
    workgroupBarrier();
    if leads(lane) {
        group_forces[s] = group_step_forces[s];
    }
    workgroupBarrier();
    return held;
}
