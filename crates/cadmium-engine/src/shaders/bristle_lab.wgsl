/// One strand for the lab, all vec4 so the layout is the same on both sides:
/// root.xyz and w = segment px; clamp.xyz and w = floor height px; curl.xyz and
/// w = press px; tip_force.xyz N and w = press steps; compliances rad per N·m.
struct LabStrand {
    root: vec4<f32>,
    clamp: vec4<f32>,
    curl: vec4<f32>,
    tip_force: vec4<f32>,
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

fn flat_floors(height_px: f32) -> Floors {
    var floors: Floors;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        floors[vertex] = height_px;
    }
    return floors;
}

@compute @workgroup_size(1)
fn solve_lab(@builtin(workgroup_id) id: vec3<u32>) {
    let strand = lab_strands[id.x];
    let shape = lab_shape(strand);
    var chain = lab_rest_chain(shape, strand.root.xyz);
    let steps = max(u32(strand.tip_force.w), 1u);
    let drop = vec3<f32>(0.0, 0.0, strand.curl.w / f32(steps));
    var force = 0.0;
    for (var step = 0u; step < steps; step++) {
        for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
            chain[vertex] -= drop;
        }
        for (var iteration = 0u; iteration < LAB_ITERATIONS_PER_STEP; iteration++) {
            force = solve_iteration(&chain, shape, flat_floors(strand.clamp.w));
        }
    }
    let first = id.x * LAB_OUT_VECTORS;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        lab_out[first + vertex] = vec4<f32>(chain[vertex], 0.0);
    }
    lab_out[first + BRISTLE_VERTICES] = vec4<f32>(force, 0.0, 0.0, 0.0);
}
