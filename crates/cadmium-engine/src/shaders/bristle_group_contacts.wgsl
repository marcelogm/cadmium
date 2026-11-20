// The grouped solve's contact rows: gaps, compliances and the active set, run per strand
// by lane 0 or per row by the lane that owns it.

// mirrors gaps_after
fn gap_of(strand: u32, vertex: u32) -> f32 {
    var moved = vec3<f32>(0.0);
    for (var hinge = 0u; hinge < vertex; hinge++) {
        moved += cross(group_turns[strand][hinge], group_chain[strand][vertex] - group_chain[strand][hinge]);
    }
    let normal = group_normals[strand][vertex];
    let under = (group_heights[strand][vertex] - group_chain[strand][vertex].z) * normal.z;
    return under - dot(normal, moved);
}

// mirrors contact_problem diagonal
fn diagonal_of(strand: u32, vertex: u32) -> f32 {
    var own = 0.0;
    for (var hinge = 0u; hinge < vertex; hinge++) {
        let lever = cross(group_chain[strand][vertex] - group_chain[strand][hinge], group_normals[strand][vertex]);
        own += group_compliances[strand][hinge] * dot(lever, lever);
    }
    return own * METRES_PER_PX;
}

// mirrors contact_compliance
fn group_compliance(strand: u32, vertex: u32, pushed: u32) -> f32 {
    var rise = 0.0;
    for (var hinge = 0u; hinge < min(vertex, pushed); hinge++) {
        let lever = group_chain[strand][vertex] - group_chain[strand][hinge];
        let arm_m = (group_chain[strand][pushed] - group_chain[strand][hinge]) * METRES_PER_PX;
        rise += group_compliances[strand][hinge] * dot(cross(arm_m, group_normals[strand][pushed]), cross(lever, group_normals[strand][vertex]));
    }
    return rise;
}

fn group_ridge_of(strand: u32) -> f32 {
    let segment_px = distance(group_chain[strand][1], group_chain[strand][0]);
    var largest = group_compliances[strand][BRISTLE_SEGMENTS - 1u] * segment_px * segment_px * METRES_PER_PX;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        largest = max(largest, group_diagonal[strand][row]);
    }
    return CONTACT_RIDGE_SHARE * largest;
}

// mirrors any_under, contact_forces
fn open_contacts(strand: u32, moves: bool) {
    var under = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        under = under || group_gaps[strand][row] > 0.0;
        group_forces[strand][row] = 0.0;
    }
    under = under && moves;
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

// mirrors active_forces
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

// mirrors released
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

// mirrors rises
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

// mirrors landed
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

// mirrors contact_forces
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

// mirrors contact_forces
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
