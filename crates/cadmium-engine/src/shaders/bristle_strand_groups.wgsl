// The strand solve of `solve_reference`, spread over the lanes of a workgroup. Each strand
// gets BRISTLE_VERTICES lanes, and lane v owns vertex v, hinge v and contact row v (vertex
// v + 1). Every value is built from the same terms in the same order as the serial solve,
// so the two agree but for the compiler's freedom to fuse multiply-adds. Lane 0 of a strand
// keeps the strand's serial work: the Cholesky, the active-set decisions, the cap and the
// force sum. Phases are separated by workgroup barriers, so every loop that holds one runs
// a workgroup-uniform count.

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

struct Lane {
    group: u32,
    strand: u32,
    vertex: u32,
    bristle: u32,
    live: bool,
};

/// What one iteration leaves the lane: its vertex, and on lane 0 the canvas's force, N.
struct Stepped {
    at: vec3<f32>,
    force: f32,
};

fn lane_of(group: u32, local: u32) -> Lane {
    let strand = local / BRISTLE_VERTICES;
    let bristle = group * BRISTLE_STRANDS_PER_GROUP + strand;
    return Lane(group, strand, local % BRISTLE_VERTICES, bristle, bristle < dab.bristle_total);
}

/// Lane 0 of a live strand, where the strand's serial work runs.
fn leads(lane: Lane) -> bool {
    return lane.live && lane.vertex == 0u;
}

/// A live lane with a hinge (and a contact row) of its own: every lane but the tip's.
fn owns_hinge(lane: Lane) -> bool {
    return lane.live && lane.vertex < BRISTLE_SEGMENTS;
}

/// The lane's vertex as the dab finds it: hung at rest after a reset, else where the last
/// dab left it (`place_at_rest`, `held_chain`).
fn held_position(lane: Lane, start: Pose) -> vec3<f32> {
    if dab.bristle_reset != 0u {
        var local = rest_chain_local(rests[lane.bristle]);
        return from_ferrule(local[lane.vertex], start);
    }
    return states[position_slot(lane.bristle, lane.vertex)].xyz;
}

/// Publishes the lane's vertex and the floor under it (`floors_under`, `floor_normals`).
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

/// Edge `lane.vertex` (`edges_of`), with a short edge marked for `mend_short_edges`.
fn place_edge(lane: Lane) {
    let s = lane.strand;
    let edge = lane.vertex;
    let span = group_chain[s][edge + 1u] - group_chain[s][edge];
    let size = length(span);
    group_edges[s][edge] = span / max(size, MIN_SEGMENT_SPAN_PX);
    group_short[s][edge] = select(1u, 0u, size > MIN_SEGMENT_SPAN_PX);
}

/// Hinge `lane.vertex`'s bending compliance, rad per N·m, as `strand_shape` computes it.
/// It depends on the rest strand alone, so it is placed once per dab.
fn place_compliance(lane: Lane, rest: BristleRest) {
    let segment_m = segment_length(rest) * METRES_PER_PX;
    group_compliances[lane.strand][lane.vertex] = hinge_compliance_per_m(lane.vertex) * segment_m / rest.stiffness;
}

/// A short edge takes the heading of the edge above it, as `edges_of` does.
fn mend_short_edges(strand: u32, clamp: vec3<f32>) {
    var previous = clamp;
    for (var edge = 0u; edge < BRISTLE_SEGMENTS; edge++) {
        if group_short[strand][edge] != 0u {
            group_edges[strand][edge] = previous;
        }
        previous = group_edges[strand][edge];
    }
}

/// Hinge `lane.vertex`'s step of the bending energy alone (`free_turns`).
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

/// How far inside the relief `vertex` would end after the free turns (`gaps_after`).
fn gap_of(strand: u32, vertex: u32) -> f32 {
    var moved = vec3<f32>(0.0);
    for (var hinge = 0u; hinge < vertex; hinge++) {
        moved += cross(group_turns[strand][hinge], group_chain[strand][vertex] - group_chain[strand][hinge]);
    }
    let normal = group_normals[strand][vertex];
    let under = (group_heights[strand][vertex] - group_chain[strand][vertex].z) * normal.z;
    return under - dot(normal, moved);
}

/// `vertex`'s own contact compliance (`contact_problem`'s diagonal).
fn diagonal_of(strand: u32, vertex: u32) -> f32 {
    var own = 0.0;
    for (var hinge = 0u; hinge < vertex; hinge++) {
        let lever = cross(group_chain[strand][vertex] - group_chain[strand][hinge], group_normals[strand][vertex]);
        own += group_compliances[strand][hinge] * dot(lever, lever);
    }
    return own * METRES_PER_PX;
}

/// `contact_compliance` over the strand's workgroup copy.
fn group_compliance(strand: u32, vertex: u32, pushed: u32) -> f32 {
    var rise = 0.0;
    for (var hinge = 0u; hinge < min(vertex, pushed); hinge++) {
        let lever = group_chain[strand][vertex] - group_chain[strand][hinge];
        let arm_m = (group_chain[strand][pushed] - group_chain[strand][hinge]) * METRES_PER_PX;
        rise += group_compliances[strand][hinge] * dot(cross(arm_m, group_normals[strand][pushed]), cross(lever, group_normals[strand][vertex]));
    }
    return rise;
}

/// `CONTACT_RIDGE_SHARE` of `ridge_scale`.
fn group_ridge_of(strand: u32) -> f32 {
    let segment_px = distance(group_chain[strand][1], group_chain[strand][0]);
    var largest = group_compliances[strand][BRISTLE_SEGMENTS - 1u] * segment_px * segment_px * METRES_PER_PX;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        largest = max(largest, group_diagonal[strand][row]);
    }
    return CONTACT_RIDGE_SHARE * largest;
}

/// Whether the free step leaves a vertex under its floor (`any_under`), and if so the ridge
/// and the first active set of `contact_forces`.
fn open_contacts(strand: u32) {
    var under = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        under = under || group_gaps[strand][row] > 0.0;
        group_forces[strand][row] = 0.0;
    }
    group_solving[strand] = select(0u, 1u, under);
    group_settled[strand] = 0u;
    group_dropped[strand] = 0u;
    if !under {
        return;
    }
    let ridge = group_ridge_of(strand);
    group_ridge[strand] = ridge;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        let engaged = group_gaps[strand][row] > 0.0 && group_diagonal[strand][row] > ridge;
        group_engaged[strand][row] = select(0u, 1u, engaged);
    }
}

/// `active_forces` over the strand's workgroup copy.
fn group_active_forces(strand: u32) -> PerContact {
    var engaged: Engaged;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        engaged[row] = group_engaged[strand][row] != 0u;
    }
    var rows: ActiveRows;
    let count = active_rows(engaged, &rows);
    let ridge = group_ridge[strand];
    var forces: PerContact;
    if count == 1u {
        let row = rows[0];
        forces[row] = group_gaps[strand][row] / (group_diagonal[strand][row] + ridge);
        return forces;
    }
    var lower: ContactMatrix;
    var rhs: PerContact;
    for (var i = 0u; i < count; i++) {
        rhs[i] = group_gaps[strand][rows[i]];
        for (var j = 0u; j < i; j++) {
            lower[i][j] = group_matrix[strand][rows[i]][rows[j]];
        }
        lower[i][i] = group_diagonal[strand][rows[i]] + ridge;
    }
    let solved = solved_by_cholesky(lower, rhs, count);
    for (var i = 0u; i < count; i++) {
        forces[rows[i]] = solved[i];
    }
    return forces;
}

/// One round's forces, then the contacts that would pull the strand down dropped
/// (`released`).
fn solve_round(strand: u32) {
    let forces = group_active_forces(strand);
    var dropped = 0u;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        group_forces[strand][row] = forces[row];
        if group_engaged[strand][row] != 0u && forces[row] < 0.0 {
            group_engaged[strand][row] = 0u;
            dropped = 1u;
        }
    }
    group_dropped[strand] = dropped;
}

/// The turn the round's forces give `hinge` (`rises`).
fn rise_turn(strand: u32, hinge: u32) -> vec3<f32> {
    var turn = vec3<f32>(0.0);
    for (var pushed = hinge + 1u; pushed < BRISTLE_VERTICES; pushed++) {
        let force = group_forces[strand][pushed - 1u];
        if force != 0.0 {
            let arm_m = (group_chain[strand][pushed] - group_chain[strand][hinge]) * METRES_PER_PX;
            turn += group_compliances[strand][hinge] * force * cross(arm_m, group_normals[strand][pushed]);
        }
    }
    return turn;
}

/// Engages `row` if the round's forces leave its vertex under its floor (`landed`).
fn land_row(strand: u32, row: u32) {
    let vertex = row + 1u;
    var moved = vec3<f32>(0.0);
    for (var hinge = 0u; hinge < vertex; hinge++) {
        moved += cross(group_rise_turns[strand][hinge], group_chain[strand][vertex] - group_chain[strand][hinge]);
    }
    let lifted = dot(group_normals[strand][vertex], moved);
    let under = group_gaps[strand][row] - lifted > CONTACT_TOLERANCE_PX;
    let added = group_engaged[strand][row] == 0u && under && group_diagonal[strand][row] > group_ridge[strand];
    if added {
        group_engaged[strand][row] = 1u;
    }
    group_added[strand][row] = select(0u, 1u, added);
}

fn any_added(strand: u32) -> bool {
    var added = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        added = added || group_added[strand][row] != 0u;
    }
    return added;
}

/// The live strands of the workgroup still solving their contacts.
fn unsettled_strands(group: u32) -> u32 {
    var count = 0u;
    for (var strand = 0u; strand < BRISTLE_STRANDS_PER_GROUP; strand++) {
        let live = group * BRISTLE_STRANDS_PER_GROUP + strand < dab.bristle_total;
        if live && group_solving[strand] != 0u && group_settled[strand] == 0u {
            count++;
        }
    }
    return count;
}

/// The last forces of `contact_forces`, kept only where engaged and never pulling.
fn close_contacts(strand: u32) {
    if group_settled[strand] == 0u {
        let forces = group_active_forces(strand);
        for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
            group_forces[strand][row] = forces[row];
        }
    }
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        let engaged = group_engaged[strand][row] != 0u;
        group_forces[strand][row] = select(0.0, max(group_forces[strand][row], 0.0), engaged);
    }
}

/// The active-set loop of `contact_forces`, round by round across the workgroup, until
/// every strand has settled or `CONTACT_ROUNDS` ran.
fn settle_contacts(lane: Lane) {
    let s = lane.strand;
    for (var round = 0u; round < CONTACT_ROUNDS; round++) {
        let solving = lane.live && group_solving[s] != 0u && group_settled[s] == 0u;
        if solving && lane.vertex == 0u {
            solve_round(s);
        }
        workgroupBarrier();
        let landing = solving && group_dropped[s] == 0u && lane.vertex < BRISTLE_SEGMENTS;
        if landing {
            group_rise_turns[s][lane.vertex] = rise_turn(s, lane.vertex);
        }
        workgroupBarrier();
        if landing {
            land_row(s, lane.vertex);
        }
        workgroupBarrier();
        if solving && lane.vertex == 0u {
            group_settled[s] = select(0u, 1u, group_dropped[s] == 0u && !any_added(s));
        }
        workgroupBarrier();
        if lane.strand == 0u && lane.vertex == 0u {
            group_unsettled = unsettled_strands(lane.group);
        }
        if workgroupUniformLoad(&group_unsettled) == 0u {
            break;
        }
    }
    if leads(lane) && group_solving[s] != 0u {
        close_contacts(s);
    }
    workgroupBarrier();
}

/// Hinge `hinge`'s free turn plus what the canvas's forces turn it (`solve_iteration`).
fn pushed_turn(strand: u32, hinge: u32, free: vec3<f32>) -> vec3<f32> {
    var turn = free;
    for (var pushed = hinge + 1u; pushed < BRISTLE_VERTICES; pushed++) {
        let force = group_forces[strand][pushed - 1u];
        let arm_m = (group_chain[strand][pushed] - group_chain[strand][hinge]) * METRES_PER_PX;
        turn += group_compliances[strand][hinge] * force * cross(arm_m, group_normals[strand][pushed]);
    }
    return turn;
}

/// The uniform scale of `capped`.
fn cap_scale(strand: u32) -> f32 {
    var largest = 0.0;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        largest = max(largest, length(group_turns[strand][hinge]));
    }
    return MAX_HINGE_TURN_RAD / max(largest, MAX_HINGE_TURN_RAD);
}

/// Where `turn_chain` puts `vertex`: the quaternion prefix down to it, rebuilt by this
/// lane alone in the serial order.
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

/// `summed`.
fn summed_group_forces(strand: u32) -> f32 {
    var total = 0.0;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        total += group_forces[strand][row];
    }
    return total;
}

/// One `solve_iteration` of every strand in the workgroup, once `place_vertex` has
/// published the carried chain and its floors.
fn solve_step(lane: Lane, shape: StrandShape) -> Stepped {
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
    if owns_hinge(lane) {
        free = free_turn(lane, shape);
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
        open_contacts(s);
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

/// Solves BRISTLE_STRANDS_PER_GROUP strands along the path `touch` recorded, one lane per
/// vertex: the same strands as `solve_reference`.
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
    for (var iteration = 0u; iteration < iterations; iteration++) {
        let pose = pose_between(start, end, min(f32(iteration + 1u) / f32(substeps), 1.0));
        var shape: StrandShape;
        if lane.live {
            shape = strand_shape(rests[lane.bristle], pose);
            at = carried(at, previous, pose);
            place_vertex(lane, at, sampled, radius_px);
            sampled = at;
        }
        previous = pose;
        workgroupBarrier();
        let stepped = solve_step(lane, shape);
        at = stepped.at;
        force = stepped.force;
    }
    if lane.live {
        states[position_slot(lane.bristle, lane.vertex)] = vec4<f32>(at, disc_floor_px(at.xy, radius_px));
        if lane.vertex == 0u {
            normal_forces[lane.bristle] = force;
            hair_loads[lane.bristle] = FLAT_HAIR_LOAD;
        }
    }
}
