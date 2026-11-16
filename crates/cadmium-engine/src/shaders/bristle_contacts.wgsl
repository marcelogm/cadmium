alias PerContact = array<f32, BRISTLE_SEGMENTS>;
alias Engaged = array<bool, BRISTLE_SEGMENTS>;
alias ContactMatrix = array<array<f32, BRISTLE_SEGMENTS>, BRISTLE_SEGMENTS>;
alias ActiveRows = array<u32, BRISTLE_SEGMENTS>;

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

/// How far vertex `vertex` moves out along its floor normal, px, under 1 N pushing vertex
/// `pushed` along its own (indices are vertices, 1-based): Σ over the hinges above both of
/// α·(arm × n_pushed)·(lever × n_vertex). The same normals on both sides keep it symmetric;
/// on a flat floor it is α·(lever · arm) in the canvas plane.
fn contact_compliance(chain: Chain, compliances: HingeCompliances, normals: Normals, vertex: u32, pushed: u32) -> f32 {
    var rise = 0.0;
    for (var hinge = 0u; hinge < min(vertex, pushed); hinge++) {
        let lever = chain[vertex] - chain[hinge];
        let arm_m = (chain[pushed] - chain[hinge]) * METRES_PER_PX;
        rise += compliances[hinge] * dot(cross(arm_m, normals[pushed]), cross(lever, normals[vertex]));
    }
    return rise;
}

/// How far inside the relief each vertex (1..) would end after `turns`, px along its floor
/// normal: the linearised z − f(x, y) ≥ 0 divided by the gradient's length.
fn gaps_after(chain: Chain, turns: Turns, floors: Floors, normals: Normals) -> PerContact {
    var gaps: PerContact;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        var moved = vec3<f32>(0.0);
        for (var hinge = 0u; hinge < vertex; hinge++) {
            moved += cross(turns[hinge], chain[vertex] - chain[hinge]);
        }
        let normal = normals[vertex];
        let under = (floors.heights_px[vertex] - chain[vertex].z) * normal.z;
        gaps[vertex - 1u] = under - dot(normal, moved);
    }
    return gaps;
}

/// One strand's contact problem: what the compliances between its vertices are built from.
struct ContactProblem {
    chain: Chain,
    compliances: HingeCompliances,
    normals: Normals,
    /// Each vertex's own compliance: the diagonal of the contact matrix.
    diagonal: PerContact,
    ridge: f32,
    gaps: PerContact,
};

/// The contact matrix entry of rows `row` and `column` (0-based, vertex − 1).
fn entry(problem: ptr<function, ContactProblem>, row: u32, column: u32) -> f32 {
    return contact_compliance((*problem).chain, (*problem).compliances, (*problem).normals, row + 1u, column + 1u);
}

/// The compliance the ridge is a share of, px per N: the largest diagonal, but never less
/// than the tip hinge's rise under a one-segment lever, so a strand with no lever keeps a
/// finite answer.
fn ridge_scale(diagonal: PerContact, chain: Chain, compliances: HingeCompliances) -> f32 {
    let segment_px = distance(chain[1], chain[0]);
    var largest = compliances[BRISTLE_SEGMENTS - 1u] * segment_px * segment_px * METRES_PER_PX;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        largest = max(largest, diagonal[row]);
    }
    return largest;
}

/// Whether a force on vertex `row` + 1 can lift it by bending at all.
fn liftable(problem: ptr<function, ContactProblem>, row: u32) -> bool {
    return (*problem).diagonal[row] > (*problem).ridge;
}

/// The engaged rows in order, and how many there are.
fn active_rows(engaged: Engaged, rows: ptr<function, ActiveRows>) -> u32 {
    var count = 0u;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
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
fn cholesky_solved(problem: ptr<function, ContactProblem>, rows: ActiveRows, count: u32) -> PerContact {
    var lower: ContactMatrix;
    var x: PerContact;
    for (var i = 0u; i < count; i++) {
        x[i] = (*problem).gaps[rows[i]];
        for (var j = 0u; j < i; j++) {
            lower[i][j] = entry(problem, rows[i], rows[j]);
        }
        lower[i][i] = (*problem).diagonal[rows[i]] + (*problem).ridge;
    }
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

/// The forces that put every active vertex on its floor; inactive rows get 0.
fn active_forces(problem: ptr<function, ContactProblem>, engaged: Engaged) -> PerContact {
    var rows: ActiveRows;
    let count = active_rows(engaged, &rows);
    var forces: PerContact;
    if count == 1u {
        let row = rows[0];
        forces[row] = (*problem).gaps[row] / ((*problem).diagonal[row] + (*problem).ridge);
        return forces;
    }
    let solved = cholesky_solved(problem, rows, count);
    for (var i = 0u; i < count; i++) {
        forces[rows[i]] = solved[i];
    }
    return forces;
}

/// Drops the contacts that would pull the strand down. True if any was dropped.
fn released(engaged: ptr<function, Engaged>, forces: PerContact) -> bool {
    var dropped = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        if (*engaged)[row] && forces[row] < 0.0 {
            (*engaged)[row] = false;
            dropped = true;
        }
    }
    return dropped;
}

/// How far the forces lift each vertex (1..) along its normal, px: the turns they give
/// every hinge, Σ α·f·(arm × n), moving each vertex below it.
fn rises(problem: ptr<function, ContactProblem>, forces: PerContact) -> PerContact {
    var turns: Turns;
    for (var pushed = 1u; pushed < BRISTLE_VERTICES; pushed++) {
        let force = forces[pushed - 1u];
        if force != 0.0 {
            for (var hinge = 0u; hinge < pushed; hinge++) {
                let arm_m = ((*problem).chain[pushed] - (*problem).chain[hinge]) * METRES_PER_PX;
                turns[hinge] += (*problem).compliances[hinge] * force * cross(arm_m, (*problem).normals[pushed]);
            }
        }
    }
    var lifted: PerContact;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        var moved = vec3<f32>(0.0);
        for (var hinge = 0u; hinge < vertex; hinge++) {
            moved += cross(turns[hinge], (*problem).chain[vertex] - (*problem).chain[hinge]);
        }
        lifted[vertex - 1u] = dot((*problem).normals[vertex], moved);
    }
    return lifted;
}

/// Adds the vertices the forces leave under their floor. True if any was added.
fn landed(problem: ptr<function, ContactProblem>, engaged: ptr<function, Engaged>, forces: PerContact) -> bool {
    let lifted = rises(problem, forces);
    var added = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        let under = (*problem).gaps[row] - lifted[row] > CONTACT_TOLERANCE_PX;
        if !(*engaged)[row] && under && liftable(problem, row) {
            (*engaged)[row] = true;
            added = true;
        }
    }
    return added;
}

/// Whether the free step leaves any vertex under its floor: without one, no contact can
/// engage, since a vertex only lands when the forces leave it under.
fn any_under(gaps: PerContact) -> bool {
    var under = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        under = under || gaps[row] > 0.0;
    }
    return under;
}

fn contact_problem(chain: Chain, compliances: HingeCompliances, normals: Normals, gaps: PerContact) -> ContactProblem {
    var problem = ContactProblem(chain, compliances, normals, PerContact(), 0.0, gaps);
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        var own = 0.0;
        for (var hinge = 0u; hinge < vertex; hinge++) {
            let lever = cross(chain[vertex] - chain[hinge], normals[vertex]);
            own += compliances[hinge] * dot(lever, lever);
        }
        problem.diagonal[vertex - 1u] = own * METRES_PER_PX;
    }
    problem.ridge = CONTACT_RIDGE_SHARE * ridge_scale(problem.diagonal, chain, compliances);
    return problem;
}

/// The canvas's force on each vertex (1..) along its floor normal, N: the active set of a
/// frictionless, one-sided contact (Baxter & Lin's and Chu & Tai's quasi-static brushes).
fn contact_forces(chain: Chain, turns: Turns, compliances: HingeCompliances, floors: Floors, normals: Normals) -> PerContact {
    let gaps = gaps_after(chain, turns, floors, normals);
    var forces: PerContact;
    if !any_under(gaps) {
        return forces;
    }
    var problem = contact_problem(chain, compliances, normals, gaps);
    var engaged: Engaged;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        engaged[row] = gaps[row] > 0.0 && liftable(&problem, row);
    }
    var settled = false;
    for (var round = 0u; round < CONTACT_ROUNDS && !settled; round++) {
        forces = active_forces(&problem, engaged);
        let dropped = released(&engaged, forces);
        settled = !dropped && !landed(&problem, &engaged, forces);
    }
    if !settled {
        forces = active_forces(&problem, engaged);
    }
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        forces[row] = select(0.0, max(forces[row], 0.0), engaged[row]);
    }
    return forces;
}
