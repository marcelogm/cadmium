// The grouped solve's active-set Cholesky, shared by the strand's lanes: lane v owns the
// active rows v, v + 8, … Each entry takes the serial arithmetic in the serial order. Every
// entry is computed at an index the compiler sees vary by lane, so it stays on the vector
// unit, as in the serial solve: on the scalar unit it would not fuse the multiply-adds.

const ROWS_PER_LANE: u32 = (MAX_CONTACT_ROWS + BRISTLE_VERTICES - 1u) / BRISTLE_VERTICES;

var<workgroup> group_rows: array<ActiveRows, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_vertex_rows: GroupFlags;
var<workgroup> group_count: array<u32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_most_rows: u32;
var<workgroup> group_lower: array<ContactMatrix, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_inverse_roots: GroupRowValues;
var<workgroup> group_solved: GroupRowValues;

// mirrors active_rows, a vertex's rows per lane: each lane marks its engaged rows, then
// lists them after the ones above
fn mark_active_rows(lane: Lane) {
    let s = lane.strand;
    let first = normal_row(lane.vertex + 1u);
    var bits = 0u;
    for (var kind = 0u; kind < ROW_KINDS; kind++) {
        group_forces[s][first + kind] = 0.0;
        if group_engaged[s][first + kind] != 0u {
            bits |= 1u << kind;
        }
    }
    group_vertex_rows[s][lane.vertex] = bits;
}

fn list_active_rows(lane: Lane) {
    let s = lane.strand;
    let first = normal_row(lane.vertex + 1u);
    let bits = group_vertex_rows[s][lane.vertex];
    var index = 0u;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        if hinge < lane.vertex {
            index += countOneBits(group_vertex_rows[s][hinge]);
        }
    }
    for (var kind = 0u; kind < ROW_KINDS; kind++) {
        if (bits & (1u << kind)) != 0u {
            group_rows[s][index] = first + kind;
            index++;
        }
    }
}

// mirrors active_forces' single row; the system's diagonal and right side
fn place_active_system(strand: u32) -> u32 {
    var count = 0u;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        count += countOneBits(group_vertex_rows[strand][hinge]);
    }
    if count == 1u {
        let row = group_rows[strand][0];
        group_forces[strand][row] = group_gaps[strand][row] / group_pivot(strand, row);
        return 0u;
    }
    for (var i = 0u; i < count; i++) {
        let row = group_rows[strand][i];
        group_solved[strand][i] = group_gaps[strand][row];
        group_lower[strand][i][i] = group_pivot(strand, row);
    }
    return count;
}

fn most_active_rows(group: u32) -> u32 {
    var most = 0u;
    for (var strand = 0u; strand < BRISTLE_STRANDS_PER_GROUP; strand++) {
        let live = group * BRISTLE_STRANDS_PER_GROUP + strand < dab.bristle_total;
        if live {
            most = max(most, group_count[strand]);
        }
    }
    return most;
}

fn owned_row(lane: Lane, slot: u32) -> u32 {
    return lane.vertex + slot * BRISTLE_VERTICES;
}

// mirrors solved_by_cholesky's factor and forward pass. Column `column`'s pivot and its
// rows' sums run together, and so does the forward pass's term of the column before; the
// serial `value / root` compiles to `value * inverseSqrt`.
fn factor_column(lane: Lane, column: u32, count: u32) {
    let s = lane.strand;
    var values: array<f32, ROWS_PER_LANE>;
    for (var slot = 0u; slot < ROWS_PER_LANE; slot++) {
        let row = owned_row(lane, slot);
        if row >= column && row < count {
            if column > 0u {
                group_solved[s][row] -= group_lower[s][row][column - 1u] * group_solved[s][column - 1u];
            }
            var value = group_lower[s][row][column];
            for (var k = 0u; k < column; k++) {
                value -= group_lower[s][row][k] * group_lower[s][column][k];
            }
            values[slot] = value;
            if row == column {
                let pivot = max(value, MIN_PIVOT);
                group_lower[s][row][row] = sqrt(pivot);
                group_inverse_roots[s][row] = inverseSqrt(pivot);
            }
        }
    }
    workgroupBarrier();
    for (var slot = 0u; slot < ROWS_PER_LANE; slot++) {
        let row = owned_row(lane, slot);
        if row == column && row < count {
            group_solved[s][row] /= group_lower[s][row][row];
        }
        if row > column && row < count {
            group_lower[s][row][column] = values[slot] * group_inverse_roots[s][column];
        }
    }
    workgroupBarrier();
}

// mirrors solved_by_cholesky's backward pass
fn backward_row(lane: Lane, step: u32, count: u32) {
    let s = lane.strand;
    for (var slot = 0u; slot < ROWS_PER_LANE; slot++) {
        let row = owned_row(lane, slot);
        if step < count && row == count - 1u - step {
            for (var k = row + 1u; k < count; k++) {
                group_solved[s][row] -= group_lower[s][k][row] * group_solved[s][k];
            }
            group_solved[s][row] /= group_lower[s][row][row];
        }
    }
    workgroupBarrier();
}

// mirrors active_forces, cholesky_solved, for the strands where `solving`
fn share_active_forces(lane: Lane, solving: bool) {
    let s = lane.strand;
    let marking = solving && lane.vertex < BRISTLE_SEGMENTS;
    if marking {
        mark_active_rows(lane);
    }
    workgroupBarrier();
    if marking {
        list_active_rows(lane);
    }
    workgroupBarrier();
    if lane.vertex == 0u {
        var count = 0u;
        if solving {
            count = place_active_system(s);
        }
        group_count[s] = count;
    }
    workgroupBarrier();
    if lane.strand == 0u && lane.vertex == 0u {
        group_most_rows = most_active_rows(lane.group);
    }
    let most = workgroupUniformLoad(&group_most_rows);
    if most == 0u {
        return;
    }
    let count = select(0u, group_count[s], lane.live);
    for (var slot = 0u; slot < ROWS_PER_LANE; slot++) {
        let i = owned_row(lane, slot);
        if i < count {
            for (var j = 0u; j < i; j++) {
                group_lower[s][i][j] = group_matrix[s][group_rows[s][i]][group_rows[s][j]];
            }
        }
    }
    workgroupBarrier();
    for (var column = 0u; column < most; column++) {
        factor_column(lane, column, count);
    }
    for (var step = 0u; step < most; step++) {
        backward_row(lane, step, count);
    }
    for (var slot = 0u; slot < ROWS_PER_LANE; slot++) {
        let i = owned_row(lane, slot);
        if i < count {
            group_forces[s][group_rows[s][i]] = group_solved[s][i];
        }
    }
    workgroupBarrier();
}
