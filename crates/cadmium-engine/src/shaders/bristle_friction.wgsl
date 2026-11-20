/// The canvas's Coulomb friction, and Baxter & Lin's anisotropy C_η.
struct Friction {
    static_mu: f32,
    kinetic_mu: f32,
    anisotropy: f32,
};

/// One strand's contacts after a solve.
struct Contacts {
    /// Each row's force, N: along the normal, or friction along a tangent.
    forces: RowValues,
    engaged: RowFlags,
    /// Kinetic friction on each sliding vertex, N, world.
    kinetic: VertexForces,
    sliding: VertexFlags,
    /// The canvas's friction on each vertex, N, world: the stuck rows' force plus the kinetic.
    friction: VertexForces,
};

fn tangential(v: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    return v - normal * dot(normal, v);
}

/// The direction toward the root in the relief's tangent plane: the way a pulled vertex
/// slides.
fn pull_direction(chain: Chain, normals: Normals, vertex: u32) -> vec3<f32> {
    let toward_root = tangential(chain[vertex - 1u] - chain[vertex], normals[vertex]);
    return toward_root / max(length(toward_root), MIN_SEGMENT_SPAN_PX);
}

/// Baxter & Lin: μ·(1 − C_η·max(0, d_pull·ŝ)²). A pulled vertex slides more easily than a
/// pushed one.
fn anisotropic_mu(mu: f32, friction: Friction, pull: vec3<f32>, slip: vec3<f32>) -> f32 {
    let along = max(dot(pull, slip), 0.0);
    return mu * (1.0 - friction.anisotropy * along * along);
}

fn kinetic_turns(free: Turns, chain: Chain, compliances: HingeCompliances, kinetic: VertexForces) -> Turns {
    var turns = free;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let force = kinetic[vertex];
        if any(force != vec3<f32>(0.0)) {
            for (var hinge = 0u; hinge < vertex; hinge++) {
                let arm_m = (chain[vertex] - chain[hinge]) * METRES_PER_PX;
                turns[hinge] += compliances[hinge] * cross(arm_m, force);
            }
        }
    }
    return turns;
}

/// Held contacts whose friction leaves their cone, or whose normal force pulls (outside
/// every cone), slide: they lose their tangent rows and take the kinetic force along the
/// friction they held, against the slip ŝ = −f̂ₜ. The cone is μₛ for a stuck contact and μₖ
/// for a sliding one, so a sliding contact sticks again only once holding it takes no more
/// than kinetic friction. The cone is on this solve's own normal force, as Coulomb's is;
/// the kinetic force takes the last iteration's instead, since a pinning solve's normal can
/// be orders of magnitude off and a kinetic force built on it throws the vertex off the
/// canvas. True if any did.
fn slipped(contacts: ptr<function, Contacts>, problem: ptr<function, ContactProblem>, friction: Friction, anchors: Anchors) -> bool {
    var any_slipped = false;
    let chain = (*problem).chain;
    let normals = (*problem).normals;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        if (*contacts).engaged[row + 1u] || (*contacts).engaged[row + 2u] {
            let held = (*contacts).forces[row + 1u] * row_direction(chain, normals, row + 1u)
                + (*contacts).forces[row + 2u] * row_direction(chain, normals, row + 2u);
            let size = length(held);
            let anchor = anchors[vertex].w;
            let load = max((*contacts).forces[row], 0.0);
            let lagged = abs(anchor);
            let cone = select(friction.kinetic_mu, friction.static_mu, anchor > 0.0);
            let pull = pull_direction(chain, normals, vertex);
            let along = select(vec3<f32>(0.0), held / size, size > 0.0);
            let pulling = (*contacts).forces[row] < 0.0;
            if pulling || (size > 0.0 && size > anisotropic_mu(cone, friction, pull, -along) * load) {
                (*contacts).engaged[row + 1u] = false;
                (*contacts).engaged[row + 2u] = false;
                (*contacts).kinetic[vertex] = anisotropic_mu(friction.kinetic_mu, friction, pull, -along) * lagged * along;
                (*contacts).sliding[vertex] = true;
                any_slipped = true;
            }
        }
    }
    return any_slipped;
}

/// The canvas's forces on a strand: one-sided normal contact and Coulomb friction, as an
/// active set over normal and tangent rows (Baxter & Lin's and Chu & Tai's quasi-static
/// brushes). With μₛ = 0 it is the frictionless contact of phase 1.
fn contact_forces(chain: Chain, free: Turns, compliances: HingeCompliances, floors: Floors, normals: Normals, friction: Friction, anchors: Anchors) -> Contacts {
    var contacts: Contacts;
    let frictional = friction.static_mu > 0.0;
    let gaps = gaps_after(chain, free, floors, normals, anchors);
    if !any_under(gaps) {
        return Contacts();
    }
    var problem = contact_problem(chain, compliances, normals, gaps);
    // Every anchored contact first tries to hold, stuck or sliding. Of two held neighbours,
    // the lower one's toward-root row is dropped: the segment between them already ties
    // that motion to the upper one, and the row would leave the matrix near singular.
    var held_above = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let engaged = gaps[row] > 0.0 && liftable(&problem, row);
        let held = engaged && frictional && anchors[vertex].w != 0.0;
        contacts.engaged[row] = engaged;
        contacts.engaged[row + 1u] = held && !held_above;
        contacts.engaged[row + 2u] = held;
        held_above = held;
    }
    var settled = false;
    for (var round = 0u; round < CONTACT_ROUNDS && !settled; round++) {
        contacts.forces = active_forces(&problem, contacts.engaged);
        // naga's SPIR-V backend fails on a pointer to a struct member passed down.
        var engaged = contacts.engaged;
        let dropped = released(&engaged, contacts.forces);
        contacts.engaged = engaged;
        var slid = false;
        if !dropped && frictional && slipped(&contacts, &problem, friction, anchors) {
            problem.gaps = gaps_after(chain, kinetic_turns(free, chain, compliances, contacts.kinetic), floors, normals, anchors);
            slid = true;
        }
        engaged = contacts.engaged;
        settled = !dropped && !slid && !landed(&problem, &engaged, contacts.forces);
        contacts.engaged = engaged;
    }
    if !settled {
        contacts.forces = active_forces(&problem, contacts.engaged);
    }
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let engaged = contacts.engaged[row];
        contacts.forces[row] = select(0.0, max(contacts.forces[row], 0.0), engaged);
        for (var tangent = 1u; tangent < ROW_KINDS; tangent++) {
            contacts.engaged[row + tangent] = engaged && contacts.engaged[row + tangent];
            contacts.forces[row + tangent] = select(0.0, contacts.forces[row + tangent], contacts.engaged[row + tangent]);
        }
        if !engaged {
            contacts.kinetic[vertex] = vec3<f32>(0.0);
            contacts.sliding[vertex] = false;
        }
        contacts.friction[vertex] = contacts.forces[row + 1u] * row_direction(chain, normals, row + 1u)
            + contacts.forces[row + 2u] * row_direction(chain, normals, row + 2u)
            + contacts.kinetic[vertex];
    }
    return contacts;
}

/// Where friction holds each vertex next: a sliding contact's anchor moves with it; a
/// stuck one keeps its anchor; a contact without tangent rows (just landed) is anchored
/// where the step left it; a free vertex has none.
fn next_anchors(chain: Chain, contacts: Contacts, anchors: Anchors) -> Anchors {
    var next: Anchors;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let load = contacts.forces[row];
        if contacts.engaged[row] && load > 0.0 {
            if contacts.sliding[vertex] {
                next[vertex] = vec4<f32>(chain[vertex], -load);
            } else if contacts.engaged[row + 1u] || contacts.engaged[row + 2u] {
                next[vertex] = vec4<f32>(anchors[vertex].xyz, load);
            } else {
                next[vertex] = vec4<f32>(chain[vertex], load);
            }
        }
    }
    return next;
}

fn normal_loads(contacts: Contacts) -> PerContact {
    var forces: PerContact;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        forces[vertex - 1u] = contacts.forces[normal_row(vertex)];
    }
    return forces;
}
