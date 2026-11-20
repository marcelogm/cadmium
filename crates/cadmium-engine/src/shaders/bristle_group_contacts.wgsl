// The grouped solve's contact rows: gaps, compliances and the active set, run per strand
// by lane 0 or by the lane that owns the rows' vertex (lane v, vertex v + 1).

// mirrors row_direction, once per row and iteration
fn place_directions(strand: u32, vertex: u32) {
    let first = normal_row(vertex);
    for (var row = first; row < first + ROW_KINDS; row++) {
        group_directions[strand][row] = row_direction(group_chain[strand], group_normals[strand], row);
    }
}

// mirrors gaps_after
fn place_gaps(strand: u32, vertex: u32) {
    var moved = vec3<f32>(0.0);
    for (var hinge = 0u; hinge < vertex; hinge++) {
        moved += cross(group_turns[strand][hinge], group_chain[strand][vertex] - group_chain[strand][hinge]);
    }
    let normal = group_normals[strand][vertex];
    let row = normal_row(vertex);
    group_gaps[strand][row] = (group_heights[strand][vertex] - group_chain[strand][vertex].z) * normal.z - dot(normal, moved);
    let off_anchor = group_chain[strand][vertex] + moved - group_anchors[strand][vertex].xyz;
    group_gaps[strand][row + 1u] = -dot(group_directions[strand][row + 1u], off_anchor);
    group_gaps[strand][row + 2u] = -dot(group_directions[strand][row + 2u], off_anchor);
}

// mirrors contact_problem diagonal
fn place_diagonal(strand: u32, vertex: u32) {
    let first = normal_row(vertex);
    for (var row = first; row < first + ROW_KINDS; row++) {
        let along = group_directions[strand][row];
        var own = 0.0;
        for (var hinge = 0u; hinge < vertex; hinge++) {
            let lever = cross(group_chain[strand][vertex] - group_chain[strand][hinge], along);
            own += group_compliances[strand][hinge] * dot(lever, lever);
        }
        group_diagonal[strand][row] = own * METRES_PER_PX;
    }
}

// mirrors row_compliance
fn group_compliance(strand: u32, row: u32, column: u32) -> f32 {
    let vertex = row_vertex(row);
    let pushed = row_vertex(column);
    let along = group_directions[strand][row];
    let push = group_directions[strand][column];
    var rise = 0.0;
    for (var hinge = 0u; hinge < min(vertex, pushed); hinge++) {
        let lever = group_chain[strand][vertex] - group_chain[strand][hinge];
        let arm_m = (group_chain[strand][pushed] - group_chain[strand][hinge]) * METRES_PER_PX;
        rise += group_compliances[strand][hinge] * dot(cross(arm_m, push), cross(lever, along));
    }
    return rise;
}

// Whether the rounds can engage `row`: a liftable normal, or a tangent of a contact that
// opened held. With `engaged_only`, whether it is engaged now.
fn reachable_row(strand: u32, row: u32, engaged_only: bool) -> bool {
    if engaged_only {
        return group_engaged[strand][row] != 0u;
    }
    if row % ROW_KINDS == 0u {
        return group_diagonal[strand][row] > group_ridge[strand];
    }
    return group_engaged[strand][normal_row(row_vertex(row)) + 2u] != 0u;
}

// mirrors cholesky_solved's entries: every column left of the vertex's rows the solve can read
fn place_matrix_rows(strand: u32, vertex: u32, engaged_only: bool) {
    let first = normal_row(vertex);
    for (var row = first; row < first + ROW_KINDS; row++) {
        if reachable_row(strand, row, engaged_only) {
            for (var column = 0u; column < row; column++) {
                if reachable_row(strand, column, engaged_only) {
                    group_matrix[strand][row][column] = group_compliance(strand, row, column);
                }
            }
        }
    }
}

// mirrors ridge_scale
fn group_ridge_of(strand: u32) -> f32 {
    let segment_px = distance(group_chain[strand][1], group_chain[strand][0]);
    var largest = group_compliances[strand][BRISTLE_SEGMENTS - 1u] * segment_px * segment_px * METRES_PER_PX;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        largest = max(largest, group_diagonal[strand][normal_row(vertex)]);
    }
    return CONTACT_RIDGE_SHARE * largest;
}

// mirrors any_under, contact_forces' first engagement
fn open_contacts(strand: u32, moves: bool, friction: Friction) {
    var under = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        under = under || group_gaps[strand][normal_row(vertex)] > 0.0;
        group_kinetic[strand][vertex] = vec3<f32>(0.0);
        group_sliding[strand][vertex] = 0u;
    }
    for (var row = 0u; row < MAX_CONTACT_ROWS; row++) {
        group_forces[strand][row] = 0.0;
        group_engaged[strand][row] = 0u;
    }
    under = under && moves;
    group_solving[strand] = select(0u, 1u, under);
    group_settled[strand] = 0u;
    group_dropped[strand] = 0u;
    group_slid[strand] = 0u;
    group_regap[strand] = 0u;
    if !under {
        return;
    }
    let ridge = group_ridge_of(strand);
    group_ridge[strand] = ridge;
    let frictional = friction.static_mu > 0.0;
    var held_above = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let engaged = group_gaps[strand][row] > 0.0 && group_diagonal[strand][row] > ridge;
        let held = engaged && frictional && group_anchors[strand][vertex].w != 0.0;
        group_engaged[strand][row] = select(0u, 1u, engaged);
        group_engaged[strand][row + 1u] = select(0u, 1u, held && !held_above);
        group_engaged[strand][row + 2u] = select(0u, 1u, held);
        held_above = held;
    }
}

// mirrors pivot
fn group_pivot(strand: u32, row: u32) -> f32 {
    var own = group_diagonal[strand][row] + group_ridge[strand];
    if row % ROW_KINDS != 0u {
        own += TANGENT_RIDGE_SHARE * group_diagonal[strand][normal_row(row_vertex(row))];
    }
    return own;
}

// mirrors released
fn group_released(strand: u32) -> bool {
    var dropped = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let held = group_engaged[strand][row + 1u] != 0u || group_engaged[strand][row + 2u] != 0u;
        if group_engaged[strand][row] != 0u && !held && group_forces[strand][row] < 0.0 {
            group_engaged[strand][row] = 0u;
            dropped = true;
        }
    }
    return dropped;
}

// mirrors one round of contact_forces after its solve, up to the gaps' rebuild, which every
// lane runs
fn solve_round(strand: u32, friction: Friction) {
    let dropped = group_released(strand);
    var regap = dropped && group_let_go(strand);
    var slid = false;
    if !dropped && friction.static_mu > 0.0 && group_slipped(strand, friction) {
        regap = true;
        slid = true;
    }
    group_dropped[strand] = select(0u, 1u, dropped);
    group_slid[strand] = select(0u, 1u, slid);
    group_regap[strand] = select(0u, 1u, regap);
}

// mirrors rises
fn rise_turn(strand: u32, hinge: u32) -> vec3<f32> {
    var turn = vec3<f32>(0.0);
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

// mirrors landed
fn land_vertex(strand: u32, vertex: u32) {
    var moved = vec3<f32>(0.0);
    for (var hinge = 0u; hinge < vertex; hinge++) {
        moved += cross(group_rise_turns[strand][hinge], group_chain[strand][vertex] - group_chain[strand][hinge]);
    }
    let lifted = dot(group_normals[strand][vertex], moved);
    let row = normal_row(vertex);
    let under = group_gaps[strand][row] - lifted > CONTACT_TOLERANCE_PX;
    let added = group_engaged[strand][row] == 0u && under && group_diagonal[strand][row] > group_ridge[strand];
    if added {
        group_engaged[strand][row] = 1u;
    }
    group_added[strand][vertex - 1u] = select(0u, 1u, added);
}

fn any_added(strand: u32) -> bool {
    var added = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        added = added || group_added[strand][row] != 0u;
    }
    return added;
}

// mirrors ends_under
fn strand_under(strand: u32) -> bool {
    var under = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        under = under || group_heights[strand][vertex] - group_chain[strand][vertex].z > FLOOR_SLACK_PX;
    }
    return under;
}

fn lifting_strands(group: u32) -> u32 {
    var count = 0u;
    for (var strand = 0u; strand < BRISTLE_STRANDS_PER_GROUP; strand++) {
        let live = group * BRISTLE_STRANDS_PER_GROUP + strand < dab.bristle_total;
        if live && group_lifting[strand] != 0u {
            count++;
        }
    }
    return count;
}

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

// mirrors contact_forces' rounds; `free` is the lane's hinge's free turn
fn settle_contacts(lane: Lane, free: vec3<f32>, friction: Friction) {
    let s = lane.strand;
    for (var round = 0u; round < CONTACT_ROUNDS; round++) {
        let solving = lane.live && group_solving[s] != 0u && group_settled[s] == 0u;
        share_active_forces(lane, solving);
        if solving && lane.vertex == 0u {
            solve_round(s, friction);
        }
        workgroupBarrier();
        let hinge_lane = solving && lane.vertex < BRISTLE_SEGMENTS;
        let regap = hinge_lane && group_regap[s] != 0u;
        let landing = hinge_lane && group_dropped[s] == 0u && group_slid[s] == 0u;
        if regap {
            group_turns[s][lane.vertex] = kinetic_turn(s, lane.vertex, free);
        }
        if landing {
            group_rise_turns[s][lane.vertex] = rise_turn(s, lane.vertex);
        }
        workgroupBarrier();
        if regap {
            place_gaps(s, lane.vertex + 1u);
        }
        if landing {
            land_vertex(s, lane.vertex + 1u);
        }
        workgroupBarrier();
        if solving && lane.vertex == 0u {
            let quiet = group_dropped[s] == 0u && group_slid[s] == 0u;
            group_settled[s] = select(0u, 1u, quiet && !any_added(s));
        }
        workgroupBarrier();
        if lane.strand == 0u && lane.vertex == 0u {
            group_unsettled = unsettled_strands(lane.group);
        }
        if workgroupUniformLoad(&group_unsettled) == 0u {
            break;
        }
    }
    share_active_forces(lane, lane.live && group_solving[s] != 0u && group_settled[s] == 0u);
    if leads(lane) && group_solving[s] != 0u {
        close_contacts(s, friction);
    }
    workgroupBarrier();
}
