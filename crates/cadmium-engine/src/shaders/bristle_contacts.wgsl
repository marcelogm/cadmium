alias Gaps = array<f32, BRISTLE_SEGMENTS>;
alias Engaged = array<bool, BRISTLE_SEGMENTS>;
alias ContactMatrix = array<array<f32, BRISTLE_SEGMENTS>, BRISTLE_SEGMENTS>;

const CONTACT_ROUNDS: u32 = 8u;
/// How far under its floor a vertex left out of the active set may end, px.
const CONTACT_TOLERANCE_PX: f32 = 1.0e-3;
/// Share of the largest diagonal added to every active pivot: a straight strand pressed
/// along its length has no bending answer, and this keeps it from dividing by zero.
const CONTACT_RIDGE_SHARE: f32 = 1.0e-5;
const MIN_PIVOT: f32 = 1.0e-20;

/// How far vertex `vertex` rises, px, under 1 N pushing vertex `pushed` up (indices are
/// vertices, 1-based): Σ over the hinges above both of α·(lever · arm) in the canvas plane.
fn contact_compliance(chain: Chain, compliances: HingeCompliances, vertex: u32, pushed: u32) -> f32 {
    var rise = 0.0;
    for (var hinge = 0u; hinge < min(vertex, pushed); hinge++) {
        let lever = (chain[vertex] - chain[hinge]).xy;
        let arm_m = (chain[pushed] - chain[hinge]).xy * METRES_PER_PX;
        rise += compliances[hinge] * dot(lever, arm_m);
    }
    return rise;
}

fn contact_matrix(chain: Chain, compliances: HingeCompliances) -> ContactMatrix {
    var matrix: ContactMatrix;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        for (var column = 0u; column < BRISTLE_SEGMENTS; column++) {
            matrix[row][column] = contact_compliance(chain, compliances, row + 1u, column + 1u);
        }
    }
    return matrix;
}

/// How far under its floor each vertex (1..) would end after `turns`, px.
fn gaps_after(chain: Chain, turns: Turns, floors: Floors) -> Gaps {
    var gaps: Gaps;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        var z = chain[vertex].z;
        for (var hinge = 0u; hinge < vertex; hinge++) {
            z += cross(turns[hinge], chain[vertex] - chain[hinge]).z;
        }
        gaps[vertex - 1u] = floors[vertex] - z;
    }
    return gaps;
}

fn factorised(system: ContactMatrix) -> ContactMatrix {
    var lower = system;
    for (var column = 0u; column < BRISTLE_SEGMENTS; column++) {
        var pivot = lower[column][column];
        for (var k = 0u; k < column; k++) {
            pivot -= lower[column][k] * lower[column][k];
        }
        let root = sqrt(max(pivot, MIN_PIVOT));
        lower[column][column] = root;
        for (var row = column + 1u; row < BRISTLE_SEGMENTS; row++) {
            var entry = lower[row][column];
            for (var k = 0u; k < column; k++) {
                entry -= lower[row][k] * lower[column][k];
            }
            lower[row][column] = entry / root;
        }
    }
    return lower;
}

fn cholesky_solved(system: ContactMatrix, rhs: Gaps) -> Gaps {
    let lower = factorised(system);
    var x = rhs;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        for (var k = 0u; k < row; k++) {
            x[row] -= lower[row][k] * x[k];
        }
        x[row] /= lower[row][row];
    }
    for (var step = 0u; step < BRISTLE_SEGMENTS; step++) {
        let row = BRISTLE_SEGMENTS - 1u - step;
        for (var k = row + 1u; k < BRISTLE_SEGMENTS; k++) {
            x[row] -= lower[k][row] * x[k];
        }
        x[row] /= lower[row][row];
    }
    return x;
}

/// The forces that put every active vertex on its floor; inactive rows solve to 0.
fn active_forces(matrix: ContactMatrix, gaps: Gaps, engaged: Engaged) -> Gaps {
    var largest = 0.0;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        largest = max(largest, matrix[row][row]);
    }
    var system: ContactMatrix;
    var rhs: Gaps;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        for (var column = 0u; column < BRISTLE_SEGMENTS; column++) {
            system[row][column] = select(0.0, matrix[row][column], engaged[row] && engaged[column]);
        }
        let ridged = matrix[row][row] + CONTACT_RIDGE_SHARE * largest;
        system[row][row] = select(1.0, ridged, engaged[row]);
        rhs[row] = select(0.0, gaps[row], engaged[row]);
    }
    return cholesky_solved(system, rhs);
}

/// Drops the contacts that would pull the strand down. True if any was dropped.
fn released(engaged: ptr<function, Engaged>, forces: Gaps) -> bool {
    var dropped = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        if (*engaged)[row] && forces[row] < 0.0 {
            (*engaged)[row] = false;
            dropped = true;
        }
    }
    return dropped;
}

/// Adds the vertices the forces leave under their floor. True if any was added.
fn landed(engaged: ptr<function, Engaged>, matrix: ContactMatrix, gaps: Gaps, forces: Gaps) -> bool {
    var added = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        var rise = 0.0;
        for (var column = 0u; column < BRISTLE_SEGMENTS; column++) {
            rise += select(0.0, matrix[row][column] * forces[column], (*engaged)[column]);
        }
        if !(*engaged)[row] && gaps[row] - rise > CONTACT_TOLERANCE_PX {
            (*engaged)[row] = true;
            added = true;
        }
    }
    return added;
}

/// The canvas's normal force on each vertex (1..), N: the active set of a frictionless,
/// one-sided contact (Baxter & Lin's and Chu & Tai's quasi-static brushes).
fn contact_forces(chain: Chain, turns: Turns, compliances: HingeCompliances, floors: Floors) -> Gaps {
    let matrix = contact_matrix(chain, compliances);
    let gaps = gaps_after(chain, turns, floors);
    var engaged: Engaged;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        engaged[row] = gaps[row] > 0.0;
    }
    var forces: Gaps;
    for (var round = 0u; round < CONTACT_ROUNDS; round++) {
        forces = active_forces(matrix, gaps, engaged);
        let dropped = released(&engaged, forces);
        if !dropped && !landed(&engaged, matrix, gaps, forces) {
            break;
        }
    }
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        forces[row] = select(0.0, max(forces[row], 0.0), engaged[row]);
    }
    return forces;
}
