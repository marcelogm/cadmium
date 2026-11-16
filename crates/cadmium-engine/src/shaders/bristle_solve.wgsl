alias Chain = array<vec3<f32>, BRISTLE_VERTICES>;
alias Floors = array<f32, BRISTLE_VERTICES>;
alias Edges = array<vec3<f32>, BRISTLE_SEGMENTS>;
alias Turns = array<vec3<f32>, BRISTLE_SEGMENTS>;
alias HingeCompliances = array<f32, BRISTLE_SEGMENTS>;

const METRES_PER_PX: f32 = RELIEF_UM_PER_PX * 1.0e-6;
const UP: vec3<f32> = vec3<f32>(0.0, 0.0, 1.0);
/// Largest turn a hinge takes in one iteration: a guard for steps that leave the linear
/// range, not physics.
const MAX_HINGE_TURN_RAD: f32 = 0.3;
const MIN_TURN_RAD: f32 = 1.0e-6;
const MIN_EDGE_PX: f32 = 1.0e-4;
const MIN_TRANSPORT_COSINE: f32 = 1.0e-3;

/// What the solve needs to know about one strand at the current ferrule pose.
struct StrandShape {
    /// Rest direction of the first edge, world.
    clamp: vec3<f32>,
    /// Rest turn of every hinge past the clamp, world, at the clamp.
    curl: vec3<f32>,
    segment_px: f32,
    /// Bending compliance per hinge, rad per N·m.
    compliances: HingeCompliances,
    /// A load on the tip, N. Zero in the app; the lab uses it.
    tip_force_n: vec3<f32>,
};

/// The rotation vector of the shortest turn from unit `start` to unit `end`.
fn rotation_between(start: vec3<f32>, end: vec3<f32>) -> vec3<f32> {
    let axis = cross(start, end);
    let sine = length(axis);
    if sine <= MIN_TURN_RAD {
        return vec3<f32>(0.0);
    }
    return axis / sine * atan2(sine, dot(start, end));
}

/// `v` turned by the rotation vector `turn` (Rodrigues).
fn turned_by(v: vec3<f32>, turn: vec3<f32>) -> vec3<f32> {
    let angle = length(turn);
    if angle <= MIN_TURN_RAD {
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
    return v * c + cross(w, v) + w * (dot(w, v) / max(1.0 + c, MIN_TRANSPORT_COSINE));
}

fn edges_of(chain: Chain, clamp: vec3<f32>) -> Edges {
    var edges: Edges;
    var previous = clamp;
    for (var edge = 0u; edge < BRISTLE_SEGMENTS; edge++) {
        let span = chain[edge + 1u] - chain[edge];
        let size = length(span);
        edges[edge] = select(previous, span / max(size, MIN_EDGE_PX), size > MIN_EDGE_PX);
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
        let rest = select(vec3<f32>(0.0), curl, hinge > 0u);
        let lever_m = (tip - chain[hinge]) * METRES_PER_PX;
        let loaded = shape.compliances[hinge] * cross(lever_m, shape.tip_force_n);
        turns[hinge] = rest - rotation_between(before, edges[hinge]) + loaded;
        before = edges[hinge];
    }
    return turns;
}

fn capped(turn: vec3<f32>) -> vec3<f32> {
    return turn * min(1.0, MAX_HINGE_TURN_RAD / max(length(turn), MIN_TURN_RAD));
}

fn quaternion_of(turn: vec3<f32>) -> vec4<f32> {
    let angle = length(turn);
    if angle <= MIN_TURN_RAD {
        return normalize(vec4<f32>(0.5 * turn, 1.0));
    }
    return vec4<f32>(turn / angle * sin(0.5 * angle), cos(0.5 * angle));
}

fn composed(outer: vec4<f32>, inner: vec4<f32>) -> vec4<f32> {
    let axis = outer.w * inner.xyz + inner.w * outer.xyz + cross(outer.xyz, inner.xyz);
    return vec4<f32>(axis, outer.w * inner.w - dot(outer.xyz, inner.xyz));
}

fn rotated(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

/// Turns every edge by its hinge and all hinges above it, and rebuilds the chain from the
/// root with exact segment lengths.
fn turn_chain(chain: ptr<function, Chain>, edges: Edges, turns: Turns, segment_px: f32) {
    var total = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    for (var edge = 0u; edge < BRISTLE_SEGMENTS; edge++) {
        total = normalize(composed(quaternion_of(capped(turns[edge])), total));
        let direction = normalize(rotated(total, edges[edge]));
        (*chain)[edge + 1u] = (*chain)[edge] + direction * segment_px;
    }
}

/// One Gauss-Newton step of the strand's bending energy under its canvas contacts, from
/// `chain` (vertex 0 already on the ferrule). Returns the canvas's normal force, N.
fn solve_iteration(chain: ptr<function, Chain>, shape: StrandShape, floors: Floors) -> f32 {
    let edges = edges_of(*chain, shape.clamp);
    var turns = free_turns(*chain, edges, shape);
    let forces = contact_forces(*chain, turns, shape.compliances, floors);
    var total = 0.0;
    for (var pushed = 1u; pushed < BRISTLE_VERTICES; pushed++) {
        let force = forces[pushed - 1u];
        total += force;
        for (var hinge = 0u; hinge < pushed; hinge++) {
            let arm_m = ((*chain)[pushed] - (*chain)[hinge]) * METRES_PER_PX;
            turns[hinge] += shape.compliances[hinge] * force * cross(arm_m, UP);
        }
    }
    turn_chain(chain, edges, turns, shape.segment_px);
    return total;
}
