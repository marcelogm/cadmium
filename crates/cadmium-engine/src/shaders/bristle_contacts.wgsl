alias PerContact = array<f32, BRISTLE_SEGMENTS>;
alias Anchors = array<vec4<f32>, BRISTLE_VERTICES>;
alias VertexForces = array<vec3<f32>, BRISTLE_VERTICES>;
alias VertexFlags = array<bool, BRISTLE_VERTICES>;
/// Rows per vertex: its floor normal, then the two tangents friction holds it along.
const ROW_KINDS: u32 = 3u;
const MAX_CONTACT_ROWS: u32 = ROW_KINDS * BRISTLE_SEGMENTS;
alias RowValues = array<f32, MAX_CONTACT_ROWS>;
alias RowFlags = array<bool, MAX_CONTACT_ROWS>;
alias ContactMatrix = array<array<f32, MAX_CONTACT_ROWS>, MAX_CONTACT_ROWS>;
alias ActiveRows = array<u32, MAX_CONTACT_ROWS>;

const CONTACT_ROUNDS: u32 = 8u;
/// How far under its floor a vertex left out of the active set may end, px.
const CONTACT_TOLERANCE_PX: f32 = 1.0e-3;
/// Share of the contact compliance added to every active pivot, so the Cholesky never
/// divides by zero. A vertex whose own compliance is no larger than the ridge has no
/// bending lever (a straight strand pressed along its length): no force can lift it, so
/// it takes none and sinks instead. In the app every strand has a rest curl, which gives
/// it the lever to buckle.
const CONTACT_RIDGE_SHARE: f32 = 1.0e-5;
const MIN_PIVOT: f32 = 1.0e-20;
/// The hair–canvas contact's shear compliance, a share of the vertex's own normal
/// compliance. It bounds the force a held vertex takes along directions bending cannot move
/// it (a straight strand's axis), and lets a held vertex creep by that share of the drive.
const TANGENT_RIDGE_SHARE: f32 = 1.0e-3;

fn row_vertex(row: u32) -> u32 {
    return row / ROW_KINDS + 1u;
}

fn normal_row(vertex: u32) -> u32 {
    return (vertex - 1u) * ROW_KINDS;
}

/// The direction a row's force pushes its vertex and its gap is measured along. The first
/// tangent points toward the root, so the row a stuck neighbour makes redundant is one row.
fn row_direction(chain: Chain, normals: Normals, row: u32) -> vec3<f32> {
    let vertex = row_vertex(row);
    let normal = normals[vertex];
    let kind = row % ROW_KINDS;
    if kind == 0u {
        return normal;
    }
    let toward_root = tangential(chain[vertex - 1u] - chain[vertex], normal);
    let size = length(toward_root);
    let first = select(perpendicular(normal), toward_root / size, size > MIN_SEGMENT_SPAN_PX);
    if kind == 1u {
        return first;
    }
    return cross(normal, first);
}

/// How far row `row`'s vertex moves along its direction, px, under 1 N on row `column`'s
/// vertex along its own: Σ over the hinges above both of α·(arm × d_column)·(lever × d_row).
fn row_compliance(chain: Chain, compliances: HingeCompliances, normals: Normals, row: u32, column: u32) -> f32 {
    let vertex = row_vertex(row);
    let pushed = row_vertex(column);
    let along = row_direction(chain, normals, row);
    let push = row_direction(chain, normals, column);
    var rise = 0.0;
    for (var hinge = 0u; hinge < min(vertex, pushed); hinge++) {
        let lever = chain[vertex] - chain[hinge];
        let arm_m = (chain[pushed] - chain[hinge]) * METRES_PER_PX;
        rise += compliances[hinge] * dot(cross(arm_m, push), cross(lever, along));
    }
    return rise;
}

fn moved_by(chain: Chain, turns: Turns, vertex: u32) -> vec3<f32> {
    var moved = vec3<f32>(0.0);
    for (var hinge = 0u; hinge < vertex; hinge++) {
        moved += cross(turns[hinge], chain[vertex] - chain[hinge]);
    }
    return moved;
}

/// Each row's gap after `turns`, px: for a normal row how far inside the relief the vertex
/// ends (the linearised z − f(x, y) ≥ 0 over the gradient's length); for a tangent row how
/// far it ends from its anchor along that tangent, negated.
fn gaps_after(chain: Chain, turns: Turns, floors: Floors, normals: Normals, anchors: Anchors) -> RowValues {
    var gaps: RowValues;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let moved = moved_by(chain, turns, vertex);
        let normal = normals[vertex];
        let row = normal_row(vertex);
        gaps[row] = (floors.heights_px[vertex] - chain[vertex].z) * normal.z - dot(normal, moved);
        let off_anchor = chain[vertex] + moved - anchors[vertex].xyz;
        gaps[row + 1u] = -dot(row_direction(chain, normals, row + 1u), off_anchor);
        gaps[row + 2u] = -dot(row_direction(chain, normals, row + 2u), off_anchor);
    }
    return gaps;
}

struct ContactProblem {
    chain: Chain,
    compliances: HingeCompliances,
    normals: Normals,
    /// Each row's own compliance: the diagonal of the contact matrix.
    diagonal: RowValues,
    ridge: f32,
    gaps: RowValues,
};

fn entry(problem: ptr<function, ContactProblem>, row: u32, column: u32) -> f32 {
    return row_compliance((*problem).chain, (*problem).compliances, (*problem).normals, row, column);
}

/// The compliance the ridge is a share of, px per N: the largest normal diagonal, but never
/// less than the tip hinge's rise under a one-segment lever, so a strand with no lever
/// keeps a finite answer.
fn ridge_scale(diagonal: RowValues, chain: Chain, compliances: HingeCompliances) -> f32 {
    let segment_px = distance(chain[1], chain[0]);
    var largest = compliances[BRISTLE_SEGMENTS - 1u] * segment_px * segment_px * METRES_PER_PX;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        largest = max(largest, diagonal[normal_row(vertex)]);
    }
    return largest;
}

/// Whether a force on the row's vertex can move it by bending at all.
fn liftable(problem: ptr<function, ContactProblem>, row: u32) -> bool {
    return (*problem).diagonal[row] > (*problem).ridge;
}

/// A row's ridged pivot: its own compliance, the ridge, and for a tangent row the contact's
/// shear compliance.
fn pivot(problem: ptr<function, ContactProblem>, row: u32) -> f32 {
    var own = (*problem).diagonal[row] + (*problem).ridge;
    if row % ROW_KINDS != 0u {
        own += TANGENT_RIDGE_SHARE * (*problem).diagonal[normal_row(row_vertex(row))];
    }
    return own;
}

fn active_rows(engaged: RowFlags, rows: ptr<function, ActiveRows>) -> u32 {
    var count = 0u;
    for (var row = 0u; row < MAX_CONTACT_ROWS; row++) {
        if engaged[row] {
            (*rows)[count] = row;
            count++;
        }
    }
    return count;
}

/// Solves the ridged system of the `count` active rows by Cholesky, in place: the same
/// arithmetic as the full system with the inactive rows left out, since their entries are
/// zero. Returns the forces in the order of `rows`.
fn cholesky_solved(problem: ptr<function, ContactProblem>, rows: ActiveRows, count: u32) -> RowValues {
    var lower: ContactMatrix;
    var rhs: RowValues;
    for (var i = 0u; i < count; i++) {
        rhs[i] = (*problem).gaps[rows[i]];
        for (var j = 0u; j < i; j++) {
            lower[i][j] = entry(problem, rows[i], rows[j]);
        }
        lower[i][i] = pivot(problem, rows[i]);
    }
    return solved_by_cholesky(lower, rhs, count);
}

fn solved_by_cholesky(matrix: ContactMatrix, rhs: RowValues, count: u32) -> RowValues {
    var lower = matrix;
    var x = rhs;
    for (var column = 0u; column < count; column++) {
        var pivot = lower[column][column];
        for (var k = 0u; k < column; k++) {
            pivot -= lower[column][k] * lower[column][k];
        }
        let root = sqrt(max(pivot, MIN_PIVOT));
        lower[column][column] = root;
        for (var row = column + 1u; row < count; row++) {
            var value = lower[row][column];
            for (var k = 0u; k < column; k++) {
                value -= lower[row][k] * lower[column][k];
            }
            lower[row][column] = value / root;
        }
    }
    for (var row = 0u; row < count; row++) {
        for (var k = 0u; k < row; k++) {
            x[row] -= lower[row][k] * x[k];
        }
        x[row] /= lower[row][row];
    }
    for (var step = 0u; step < count; step++) {
        let row = count - 1u - step;
        for (var k = row + 1u; k < count; k++) {
            x[row] -= lower[k][row] * x[k];
        }
        x[row] /= lower[row][row];
    }
    return x;
}

fn active_forces(problem: ptr<function, ContactProblem>, engaged: RowFlags) -> RowValues {
    var rows: ActiveRows;
    let count = active_rows(engaged, &rows);
    var forces: RowValues;
    if count == 1u {
        let row = rows[0];
        forces[row] = (*problem).gaps[row] / pivot(problem, row);
        return forces;
    }
    let solved = cholesky_solved(problem, rows, count);
    for (var i = 0u; i < count; i++) {
        forces[rows[i]] = solved[i];
    }
    return forces;
}

/// Drops the contacts that would pull the strand down. A held contact (with tangent rows)
/// slides first instead, in `slipped`. True if any was dropped.
fn released(engaged: ptr<function, RowFlags>, forces: RowValues) -> bool {
    var dropped = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let held = (*engaged)[row + 1u] || (*engaged)[row + 2u];
        if (*engaged)[row] && !held && forces[row] < 0.0 {
            (*engaged)[row] = false;
            dropped = true;
        }
    }
    return dropped;
}

/// How far the row forces lift each vertex (1..) along its normal, px.
fn rises(problem: ptr<function, ContactProblem>, forces: RowValues) -> PerContact {
    var turns: Turns;
    for (var row = 0u; row < MAX_CONTACT_ROWS; row++) {
        let force = forces[row];
        if force != 0.0 {
            let pushed = row_vertex(row);
            let along = row_direction((*problem).chain, (*problem).normals, row);
            for (var hinge = 0u; hinge < pushed; hinge++) {
                let arm_m = ((*problem).chain[pushed] - (*problem).chain[hinge]) * METRES_PER_PX;
                turns[hinge] += (*problem).compliances[hinge] * force * cross(arm_m, along);
            }
        }
    }
    var lifted: PerContact;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        lifted[vertex - 1u] = dot((*problem).normals[vertex], moved_by((*problem).chain, turns, vertex));
    }
    return lifted;
}

/// Adds the vertices the forces leave under their floor, without tangent rows: a contact
/// lands frictionless and is anchored where the step leaves it. True if any was added.
fn landed(problem: ptr<function, ContactProblem>, engaged: ptr<function, RowFlags>, forces: RowValues) -> bool {
    let lifted = rises(problem, forces);
    var added = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let under = (*problem).gaps[row] - lifted[vertex - 1u] > CONTACT_TOLERANCE_PX;
        if !(*engaged)[row] && under && liftable(problem, row) {
            (*engaged)[row] = true;
            added = true;
        }
    }
    return added;
}

fn any_under(gaps: RowValues) -> bool {
    var under = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        under = under || gaps[normal_row(vertex)] > 0.0;
    }
    return under;
}

fn ends_under(chain: Chain, heights_px: array<f32, BRISTLE_VERTICES>) -> bool {
    var under = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        under = under || heights_px[vertex] - chain[vertex].z > FLOOR_SLACK_PX;
    }
    return under;
}

fn contact_problem(chain: Chain, compliances: HingeCompliances, normals: Normals, gaps: RowValues) -> ContactProblem {
    var problem = ContactProblem(chain, compliances, normals, RowValues(), 0.0, gaps);
    for (var row = 0u; row < MAX_CONTACT_ROWS; row++) {
        let vertex = row_vertex(row);
        let along = row_direction(chain, normals, row);
        var own = 0.0;
        for (var hinge = 0u; hinge < vertex; hinge++) {
            let lever = cross(chain[vertex] - chain[hinge], along);
            own += compliances[hinge] * dot(lever, lever);
        }
        problem.diagonal[row] = own * METRES_PER_PX;
    }
    problem.ridge = CONTACT_RIDGE_SHARE * ridge_scale(problem.diagonal, chain, compliances);
    return problem;
}
