alias Chain = array<vec3<f32>, BRISTLE_VERTICES>;
alias Normals = array<vec3<f32>, BRISTLE_VERTICES>;
alias Edges = array<vec3<f32>, BRISTLE_SEGMENTS>;
alias Turns = array<vec3<f32>, BRISTLE_SEGMENTS>;
alias HingeCompliances = array<f32, BRISTLE_SEGMENTS>;

const METRES_PER_PX: f32 = RELIEF_UM_PER_PX * 1.0e-6;
/// Largest turn a hinge takes in one iteration: a guard for steps that leave the linear
/// range, not physics.
const MAX_HINGE_TURN_RAD: f32 = 0.3;
/// Below this sine two directions count as parallel or antiparallel.
const PARALLEL_SINE: f32 = 1.0e-6;
/// Below this angle a turn uses its first-order series instead of its trigonometry.
const SERIES_ANGLE_RAD: f32 = 1.0e-6;
/// Smallest 1 + cos θ the parallel transport divides by: it keeps a near half turn finite.
const MIN_ONE_PLUS_COSINE: f32 = 1.0e-3;
/// Above this |x| a unit vector is too close to x to build a perpendicular from it.
const NEAR_X_AXIS: f32 = 0.9;
const PI: f32 = 3.14159265358979;
/// Most contact-only steps that push a strand out of the relief at the end of a dab.
const FLOOR_CORRECTIONS: u32 = 2u;
/// How far under its floor a dab may leave a vertex, px. The corrections push a strand
/// only to this depth, so one the two solves see either side of it moves the same.
const FLOOR_SLACK_PX: f32 = 1.0e-2;

/// The relief under each vertex: its height, px, and its slope (∂f/∂x, ∂f/∂y).
struct Floors {
    heights_px: array<f32, BRISTLE_VERTICES>,
    slopes: array<vec2<f32>, BRISTLE_VERTICES>,
};

/// The unit normal of the relief under each vertex, pointing out of it: the gradient of
/// z − f(x, y), normalised.
fn floor_normals(floors: Floors) -> Normals {
    var normals: Normals;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        normals[vertex] = normalize(vec3<f32>(-floors.slopes[vertex], 1.0));
    }
    return normals;
}

/// What the solve needs to know about one strand at the current ferrule pose.
struct StrandShape {
    /// Rest direction of the first edge, world.
    clamp: vec3<f32>,
    /// Rest turn of every hinge past the clamp, world, at the clamp.
    curl: vec3<f32>,
    /// Length of every segment, px: the chain is rebuilt with it exactly.
    segment_px: f32,
    /// Bending compliance per hinge, rad per N·m.
    compliances: HingeCompliances,
    /// A load on the tip, N. Zero in the app; the lab uses it.
    tip_force_n: vec3<f32>,
    friction: Friction,
    /// The plastic shift of every hinge's rest turn, world, at the clamp.
    plastic: Turns,
};

/// A unit vector perpendicular to unit `v`.
fn perpendicular(v: vec3<f32>) -> vec3<f32> {
    let helper = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(v.x) > NEAR_X_AXIS);
    return normalize(cross(v, helper));
}

/// The rotation vector of the shortest turn from unit `start` to unit `end`; a half turn
/// about some perpendicular when they are opposite.
fn rotation_between(start: vec3<f32>, end: vec3<f32>) -> vec3<f32> {
    let axis = cross(start, end);
    let sine = length(axis);
    if sine > PARALLEL_SINE {
        return axis / sine * atan2(sine, dot(start, end));
    }
    if dot(start, end) < 0.0 {
        return perpendicular(start) * PI;
    }
    return vec3<f32>(0.0);
}

/// `v` turned by the rotation vector `turn` (Rodrigues).
fn turned_by(v: vec3<f32>, turn: vec3<f32>) -> vec3<f32> {
    let angle = length(turn);
    if angle <= SERIES_ANGLE_RAD {
        return v + cross(turn, v);
    }
    let k = turn / angle;
    let c = cos(angle);
    return v * c + cross(k, v) * sin(angle) + k * dot(k, v) * (1.0 - c);
}

/// `v` carried by the shortest turn from unit `start` to unit `end`: parallel transport.
fn transported(v: vec3<f32>, start: vec3<f32>, end: vec3<f32>) -> vec3<f32> {
    let c = dot(start, end);
    let w = cross(start, end);
    return v * c + cross(w, v) + w * (dot(w, v) / max(1.0 + c, MIN_ONE_PLUS_COSINE));
}

fn edges_of(chain: Chain, clamp: vec3<f32>) -> Edges {
    var edges: Edges;
    var previous = clamp;
    for (var edge = 0u; edge < BRISTLE_SEGMENTS; edge++) {
        let span = chain[edge + 1u] - chain[edge];
        let size = length(span);
        let heading = span / max(size, MIN_SEGMENT_SPAN_PX);
        edges[edge] = select(previous, heading, size > MIN_SEGMENT_SPAN_PX);
        previous = edges[edge];
    }
    return edges;
}

/// The step of the bending energy alone: each hinge turns back to its rest turn, plus
/// what the tip load bends it, before the canvas pushes back.
fn free_turns(chain: Chain, edges: Edges, shape: StrandShape) -> Turns {
    var turns: Turns;
    var before = shape.clamp;
    let tip = chain[BRISTLE_VERTICES - 1u];
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        let curl = transported(shape.curl, shape.clamp, before);
        let rest = select(vec3<f32>(0.0), curl, hinge > 0u) + transported(shape.plastic[hinge], shape.clamp, before);
        let lever_m = (tip - chain[hinge]) * METRES_PER_PX;
        let loaded = shape.compliances[hinge] * cross(lever_m, shape.tip_force_n);
        turns[hinge] = rest - rotation_between(before, edges[hinge]) + loaded;
        before = edges[hinge];
    }
    return turns;
}

/// `turns` scaled together so the largest is at most `MAX_HINGE_TURN_RAD`: a uniform
/// scale keeps the step a descent direction, which capping each hinge alone would not.
fn capped(turns: Turns) -> Turns {
    var largest = 0.0;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        largest = max(largest, length(turns[hinge]));
    }
    let scale = MAX_HINGE_TURN_RAD / max(largest, MAX_HINGE_TURN_RAD);
    var scaled: Turns;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        scaled[hinge] = turns[hinge] * scale;
    }
    return scaled;
}

fn quaternion_of(turn: vec3<f32>) -> vec4<f32> {
    let angle = length(turn);
    if angle <= SERIES_ANGLE_RAD {
        return normalize(vec4<f32>(0.5 * turn, 1.0));
    }
    return vec4<f32>(turn / angle * sin(0.5 * angle), cos(0.5 * angle));
}

/// The quaternion product `outer` ⊗ `inner`: `inner` turns first.
fn composed(outer: vec4<f32>, inner: vec4<f32>) -> vec4<f32> {
    let axis = outer.w * inner.xyz + inner.w * outer.xyz + cross(outer.xyz, inner.xyz);
    return vec4<f32>(axis, outer.w * inner.w - dot(outer.xyz, inner.xyz));
}

fn rotated(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

/// Turns every edge by its hinge and all hinges above it, and rebuilds the chain from the
/// root with exact segment lengths. The turns are world vectors at the current pose, so
/// edge e turns by R₀⋯R_e: its own hinge first, then each hinge above carries it, axis
/// included.
fn turn_chain(chain: ptr<function, Chain>, edges: Edges, turns: Turns, segment_px: f32) {
    var total = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    for (var edge = 0u; edge < BRISTLE_SEGMENTS; edge++) {
        total = normalize(composed(total, quaternion_of(turns[edge])));
        let direction = normalize(rotated(total, edges[edge]));
        (*chain)[edge + 1u] = (*chain)[edge] + direction * segment_px;
    }
}

/// The canvas's total normal force, N: the sum of the magnitudes along each normal.
fn summed(forces: PerContact) -> f32 {
    var total = 0.0;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        total += forces[row];
    }
    return total;
}

/// One Gauss-Newton step of the strand's bending energy under its canvas contacts and
/// friction, from `chain` (vertex 0 already on the ferrule). Moves `anchors` on with the
/// contacts. The forces are the equilibrium of the full step, exact only once the cap on
/// the turns no longer engages.
fn solve_iteration(chain: ptr<function, Chain>, shape: StrandShape, floors: Floors, anchors: ptr<function, Anchors>) -> Contacts {
    let edges = edges_of(*chain, shape.clamp);
    return pushed_step(chain, edges, free_turns(*chain, edges, shape), shape, shape.friction, floors, anchors);
}

/// The contact step alone: the strand keeps its bend and the canvas only pushes it out,
/// frictionless, leaving no anchors.
fn lifted_onto(chain: ptr<function, Chain>, shape: StrandShape, floors: Floors) -> Contacts {
    var none: Anchors;
    return pushed_step(chain, edges_of(*chain, shape.clamp), Turns(), shape, Friction(0.0, 0.0, 0.0), floors, &none);
}

fn pushed_step(chain: ptr<function, Chain>, edges: Edges, free: Turns, shape: StrandShape, friction: Friction, floors: Floors, anchors: ptr<function, Anchors>) -> Contacts {
    let normals = floor_normals(floors);
    let contacts = contact_forces(*chain, free, shape.compliances, floors, normals, friction, *anchors);
    var turns = kinetic_turns(free, *chain, shape.compliances, contacts.kinetic);
    for (var row = 0u; row < MAX_CONTACT_ROWS; row++) {
        let force = contacts.forces[row];
        let pushed = row_vertex(row);
        let along = row_direction(*chain, normals, row);
        for (var hinge = 0u; hinge < pushed; hinge++) {
            let arm_m = ((*chain)[pushed] - (*chain)[hinge]) * METRES_PER_PX;
            turns[hinge] += shape.compliances[hinge] * force * cross(arm_m, along);
        }
    }
    turn_chain(chain, edges, capped(turns), shape.segment_px);
    if friction.static_mu > 0.0 {
        *anchors = next_anchors(*chain, contacts, *anchors);
    } else {
        *anchors = Anchors();
    }
    return contacts;
}

/// The last step of a dab moves the strand onto floors read before it moved, so it can end
/// inside the relief, past a wall the ±1 px slope did not see. Each correction reads the
/// floors where the step left it and pushes out only a strand left under them, straight
/// up: along a wall's tilted normal a push barely moves a vertex out of it.
fn rested(chain: ptr<function, Chain>, sampled: Chain, shape: StrandShape, radius_px: f32) -> PerContact {
    var swept = sampled;
    var forces: PerContact;
    for (var correction = 0u; correction < FLOOR_CORRECTIONS; correction++) {
        var floors: Floors;
        for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
            floors.heights_px[vertex] = swept_floor_px(swept[vertex].xy, (*chain)[vertex].xy, radius_px);
        }
        if !ends_under(*chain, floors.heights_px) {
            break;
        }
        for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
            floors.heights_px[vertex] -= FLOOR_SLACK_PX;
        }
        swept = *chain;
        let lifted = normal_loads(lifted_onto(chain, shape, floors));
        for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
            forces[row] += lifted[row];
        }
    }
    return forces;
}
