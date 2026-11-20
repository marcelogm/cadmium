/// The lab writes, per strand, every vertex as xyz px and w = the canvas's force on it, N,
/// then the total force in x, then per vertex the friction on it, N, with w = its anchor
/// mode, then per hinge the plastic shift, rad, with w = the excess turn, rad, then per
/// vertex its anchor, px, then per hinge the summed excess over the yield its creep steps saw.
///
/// One strand for the lab, all vec4 so the layout is the same on both sides:
/// root.xyz and w = segment px; clamp.xyz and w = floor height px; curl.xyz and
/// w = press px; tip_force.xyz N and w = press steps; floor_slope.xy the floor plane's
/// slope, so the floor is z = height + slope · xy, and zw how far the press slides the
/// strand, px; step, when its w is 1, replaces the plane with a step of height y for
/// x > x, read as a hair of radius z rests on it; friction = μₛ, μₖ, anisotropy C_η, creep per
/// second; drag.xy how far the drag phase moves the strand, px, drag.z its steps, drag.w 1 to
/// drop the tip load for it; timing.x seconds per press step, timing.y per drag and settle step,
/// timing.z the steps of a settle phase after the drag and w how far it raises the strand,
/// px; compliances rad per N·m.
struct LabStrand {
    root: vec4<f32>,
    clamp: vec4<f32>,
    curl: vec4<f32>,
    tip_force: vec4<f32>,
    floor_slope: vec4<f32>,
    step: vec4<f32>,
    friction: vec4<f32>,
    drag: vec4<f32>,
    timing: vec4<f32>,
    compliances: array<vec4<f32>, 2>,
};

@group(0) @binding(0) var<storage, read> lab_strands: array<LabStrand>;
@group(0) @binding(1) var<storage, read_write> lab_out: array<vec4<f32>>;

const LAB_ITERATIONS_PER_STEP: u32 = 2u;
const LAB_OUT_VECTORS: u32 = 3u * BRISTLE_VERTICES + 1u + 2u * BRISTLE_SEGMENTS;

fn lab_shape(strand: LabStrand) -> StrandShape {
    var compliances: HingeCompliances;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        compliances[hinge] = strand.compliances[hinge / 4u][hinge % 4u];
    }
    let friction = Friction(strand.friction.x, strand.friction.y, strand.friction.z);
    return StrandShape(strand.clamp.xyz, strand.curl.xyz, strand.root.w, compliances, strand.tip_force.xyz, friction, Turns());
}

fn lab_rest_chain(shape: StrandShape, root: vec3<f32>) -> Chain {
    var chain: Chain;
    var edge = shape.clamp;
    chain[0] = root;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        if vertex > 1u {
            edge = turned_by(edge, shape.curl);
        }
        chain[vertex] = chain[vertex - 1u] + edge * shape.segment_px;
    }
    return chain;
}

/// The floor plane z = height + slope · xy under each vertex of `chain`.
fn plane_floors(strand: LabStrand, chain: Chain) -> Floors {
    var floors: Floors;
    let slope = strand.floor_slope.xy;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        floors.heights_px[vertex] = strand.clamp.w + dot(slope, chain[vertex].xy);
        floors.slopes[vertex] = slope;
    }
    return floors;
}

/// The step the lab's `surface_px` stands for: edge x, height and radius px.
var<private> lab_step: vec4<f32>;

/// The relief `bristle_floor.wgsl` reads: the lab's step.
fn surface_px(at: vec2<f32>) -> f32 {
    return select(0.0, lab_step.y, at.x > lab_step.x);
}

fn lab_floors(strand: LabStrand, previous: Chain, chain: Chain) -> Floors {
    if strand.step.w != 0.0 {
        return floors_under(previous, chain, strand.step.z);
    }
    return plane_floors(strand, chain);
}

struct LabState {
    chain: Chain,
    sampled: Chain,
    anchors: Anchors,
    contacts: Contacts,
    plastic: Turns,
    /// Each hinge's excess at the last solve, against the shift it solved with: the
    /// strand's own, as a solve after the creep would find it again under a held load.
    excess: PerContact,
    crept_excess: PerContact,
};

/// Moves the strand by `shift` and solves: one press or drag step lasting `step_s`, after
/// which the hinges creep. When `spread`, each iteration takes its share of the shift
/// first, as each substep of a dab does. Works on locals, since naga's SPIR-V backend fails
/// on pointers into a struct passed down.
fn advance_lab(state: ptr<function, LabState>, strand: LabStrand, shape: StrandShape, shift: vec3<f32>, spread: bool, step_s: f32) {
    var chain = (*state).chain;
    var sampled = (*state).sampled;
    var anchors = (*state).anchors;
    var contacts = (*state).contacts;
    var current = shape;
    current.plastic = (*state).plastic;
    let share = select(shift, shift / f32(LAB_ITERATIONS_PER_STEP), spread);
    for (var iteration = 0u; iteration < LAB_ITERATIONS_PER_STEP; iteration++) {
        if spread || iteration == 0u {
            for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
                chain[vertex] += share;
            }
        }
        let floors = lab_floors(strand, sampled, chain);
        sampled = chain;
        contacts = solve_iteration(&chain, current, floors, &anchors);
    }
    if strand.step.w != 0.0 {
        let lifted = rested(&chain, sampled, current, strand.step.z);
        for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
            contacts.forces[normal_row(vertex)] += lifted[vertex - 1u];
        }
    }
    let creep = creep_share(strand.friction.w, step_s);
    let excess = excess_sizes(chain, current);
    var crept_excess = (*state).crept_excess;
    if creep > 0.0 {
        for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
            crept_excess[hinge] += max(excess[hinge] - PLASTIC_YIELD_RAD, 0.0);
        }
    }
    *state = LabState(chain, sampled, anchors, contacts, crept(chain, current, creep), excess, crept_excess);
}

@compute @workgroup_size(1)
fn solve_lab(@builtin(workgroup_id) id: vec3<u32>) {
    let strand = lab_strands[id.x];
    lab_step = strand.step;
    var shape = lab_shape(strand);
    let chain = lab_rest_chain(shape, strand.root.xyz);
    var state = LabState(chain, chain, Anchors(), Contacts(), Turns(), PerContact(), PerContact());
    let steps = max(u32(strand.tip_force.w), 1u);
    let drop = vec3<f32>(-strand.floor_slope.zw, strand.curl.w) / f32(steps);
    for (var step = 0u; step < steps; step++) {
        advance_lab(&state, strand, shape, -drop, false, strand.timing.x);
    }
    let drag_steps = u32(strand.drag.z);
    if strand.drag.w != 0.0 {
        shape.tip_force_n = vec3<f32>(0.0);
    }
    for (var step = 0u; step < drag_steps; step++) {
        advance_lab(&state, strand, shape, vec3<f32>(strand.drag.xy / f32(drag_steps), 0.0), true, strand.timing.y);
    }
    let settle_steps = u32(strand.timing.z);
    for (var step = 0u; step < settle_steps; step++) {
        advance_lab(&state, strand, shape, vec3<f32>(0.0, 0.0, strand.timing.w / f32(settle_steps)), true, strand.timing.y);
    }
    let first = id.x * LAB_OUT_VECTORS;
    let normal = normal_loads(state.contacts);
    let friction = state.contacts.friction;
    lab_out[first] = vec4<f32>(state.chain[0], 0.0);
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        lab_out[first + vertex] = vec4<f32>(state.chain[vertex], normal[vertex - 1u]);
    }
    lab_out[first + BRISTLE_VERTICES] = vec4<f32>(summed(normal), 0.0, 0.0, 0.0);
    let friction_first = first + BRISTLE_VERTICES + 1u;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        lab_out[friction_first + vertex] = vec4<f32>(friction[vertex], state.anchors[vertex].w);
    }
    let plastic_first = friction_first + BRISTLE_VERTICES;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        lab_out[plastic_first + hinge] = vec4<f32>(state.plastic[hinge], state.excess[hinge]);
    }
    let anchor_first = plastic_first + BRISTLE_SEGMENTS;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        lab_out[anchor_first + vertex] = vec4<f32>(state.anchors[vertex].xyz, 0.0);
    }
    let crept_first = anchor_first + BRISTLE_VERTICES;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        lab_out[crept_first + hinge] = vec4<f32>(state.crept_excess[hinge], 0.0, 0.0, 0.0);
    }
}
