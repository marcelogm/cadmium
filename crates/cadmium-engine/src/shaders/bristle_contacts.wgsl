alias PerContact = array<f32, BRISTLE_SEGMENTS>;
alias Engaged = array<bool, BRISTLE_SEGMENTS>;
alias ContactMatrix = array<array<f32, BRISTLE_SEGMENTS>, BRISTLE_SEGMENTS>;

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

fn contact_matrix(chain: Chain, compliances: HingeCompliances, normals: Normals) -> ContactMatrix {
    var matrix: ContactMatrix;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        for (var column = 0u; column < BRISTLE_SEGMENTS; column++) {
            matrix[row][column] = contact_compliance(chain, compliances, normals, row + 1u, column + 1u);
        }
    }
    return matrix;
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

fn cholesky_solved(system: ContactMatrix, rhs: PerContact) -> PerContact {
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

/// The compliance the ridge is a share of, px per N: the largest diagonal, but never less
/// than the tip hinge's rise under a one-segment lever, so a strand with no lever keeps a
/// finite answer.
fn ridge_scale(matrix: ContactMatrix, chain: Chain, compliances: HingeCompliances) -> f32 {
    let segment_px = distance(chain[1], chain[0]);
    var largest = compliances[BRISTLE_SEGMENTS - 1u] * segment_px * segment_px * METRES_PER_PX;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        largest = max(largest, matrix[row][row]);
    }
    return largest;
}

/// The forces that put every active vertex on its floor; inactive rows solve to 0.
fn active_forces(matrix: ContactMatrix, ridge: f32, gaps: PerContact, engaged: Engaged) -> PerContact {
    var system: ContactMatrix;
    var rhs: PerContact;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        for (var column = 0u; column < BRISTLE_SEGMENTS; column++) {
            system[row][column] = select(0.0, matrix[row][column], engaged[row] && engaged[column]);
        }
        let ridged = matrix[row][row] + ridge;
        system[row][row] = select(1.0, ridged, engaged[row]);
        rhs[row] = select(0.0, gaps[row], engaged[row]);
    }
    return cholesky_solved(system, rhs);
}

/// Whether a force on vertex `row` + 1 can lift it by bending at all.
fn liftable(matrix: ContactMatrix, ridge: f32, row: u32) -> bool {
    return matrix[row][row] > ridge;
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

/// Adds the vertices the forces leave under their floor. True if any was added.
fn landed(engaged: ptr<function, Engaged>, matrix: ContactMatrix, ridge: f32, gaps: PerContact, forces: PerContact) -> bool {
    var added = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        var rise = 0.0;
        for (var column = 0u; column < BRISTLE_SEGMENTS; column++) {
            rise += select(0.0, matrix[row][column] * forces[column], (*engaged)[column]);
        }
        let under = gaps[row] - rise > CONTACT_TOLERANCE_PX;
        if !(*engaged)[row] && under && liftable(matrix, ridge, row) {
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

/// The canvas's force on each vertex (1..) along its floor normal, N: the active set of a
/// frictionless, one-sided contact (Baxter & Lin's and Chu & Tai's quasi-static brushes).
fn contact_forces(chain: Chain, turns: Turns, compliances: HingeCompliances, floors: Floors, normals: Normals) -> PerContact {
    let gaps = gaps_after(chain, turns, floors, normals);
    var forces: PerContact;
    if !any_under(gaps) {
        return forces;
    }
    let matrix = contact_matrix(chain, compliances, normals);
    let ridge = CONTACT_RIDGE_SHARE * ridge_scale(matrix, chain, compliances);
    var engaged: Engaged;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        engaged[row] = gaps[row] > 0.0 && liftable(matrix, ridge, row);
    }
    var settled = false;
    for (var round = 0u; round < CONTACT_ROUNDS && !settled; round++) {
        forces = active_forces(matrix, ridge, gaps, engaged);
        let dropped = released(&engaged, forces);
        settled = !dropped && !landed(&engaged, matrix, ridge, gaps, forces);
    }
    if !settled {
        forces = active_forces(matrix, ridge, gaps, engaged);
    }
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        forces[row] = select(0.0, max(forces[row], 0.0), engaged[row]);
    }
    return forces;
}
