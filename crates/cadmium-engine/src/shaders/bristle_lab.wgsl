/// The lab writes, per strand, every vertex as xyz px and w = the canvas's force on it, N,
/// then the total force in x.
///
/// One strand for the lab, all vec4 so the layout is the same on both sides:
/// root.xyz and w = segment px; clamp.xyz and w = floor height px; curl.xyz and
/// w = press px; tip_force.xyz N and w = press steps; floor_slope.xy the floor plane's
/// slope, so the floor is z = height + slope · xy, and zw how far the press slides the
/// strand, px; step, when its w is 1, replaces the plane with a step of height y for
/// x > x, read as a hair of radius z rests on it; compliances rad per N·m.
struct LabStrand {
    root: vec4<f32>,
    clamp: vec4<f32>,
    curl: vec4<f32>,
    tip_force: vec4<f32>,
    floor_slope: vec4<f32>,
    step: vec4<f32>,
    compliances: array<vec4<f32>, 2>,
};

@group(0) @binding(0) var<storage, read> lab_strands: array<LabStrand>;
@group(0) @binding(1) var<storage, read_write> lab_out: array<vec4<f32>>;

const LAB_ITERATIONS_PER_STEP: u32 = 2u;
const LAB_OUT_VECTORS: u32 = BRISTLE_VERTICES + 1u;

fn lab_shape(strand: LabStrand) -> StrandShape {
    var compliances: HingeCompliances;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        compliances[hinge] = strand.compliances[hinge / 4u][hinge % 4u];
    }
    return StrandShape(strand.clamp.xyz, strand.curl.xyz, strand.root.w, compliances, strand.tip_force.xyz);
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

@compute @workgroup_size(1)
fn solve_lab(@builtin(workgroup_id) id: vec3<u32>) {
    let strand = lab_strands[id.x];
    lab_step = strand.step;
    let shape = lab_shape(strand);
    var chain = lab_rest_chain(shape, strand.root.xyz);
    let steps = max(u32(strand.tip_force.w), 1u);
    let drop = vec3<f32>(-strand.floor_slope.zw, strand.curl.w) / f32(steps);
    var forces: PerContact;
    var sampled = chain;
    for (var step = 0u; step < steps; step++) {
        for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
            chain[vertex] -= drop;
        }
        for (var iteration = 0u; iteration < LAB_ITERATIONS_PER_STEP; iteration++) {
            let floors = lab_floors(strand, sampled, chain);
            sampled = chain;
            forces = solve_iteration(&chain, shape, floors);
        }
        if strand.step.w != 0.0 {
            let lifted = rested(&chain, sampled, shape, strand.step.z);
            for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
                forces[row] += lifted[row];
            }
        }
    }
    let first = id.x * LAB_OUT_VECTORS;
    lab_out[first] = vec4<f32>(chain[0], 0.0);
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        lab_out[first + vertex] = vec4<f32>(chain[vertex], forces[vertex - 1u]);
    }
    lab_out[first + BRISTLE_VERTICES] = vec4<f32>(summed(forces), 0.0, 0.0, 0.0);
}
