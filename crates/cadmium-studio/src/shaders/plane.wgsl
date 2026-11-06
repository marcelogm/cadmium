// The studio canvas as a slab in the scene: its top face shows the canvas,
// its four sides are a thin dark body. The geometry comes from the vertex
// index: six vertices for the top face, then six per side.

struct PlaneParams {
    view_proj: mat4x4<f32>,
    // The canvas rectangle: min x, min y, max x, max y, in canvas px.
    rect: vec4<f32>,
    // x: the body's depth below the canvas, in px (z = -x); yzw unused.
    body: vec4<f32>,
};

struct PlaneVertex {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) side: u32,
};

const EDGE_COLOUR: vec4<f32> = vec4<f32>(0.035, 0.03, 0.028, 1.0);
const TOP_VERTICES: u32 = 6u;
const VERTICES_PER_SIDE: u32 = 6u;

@group(0) @binding(0) var<uniform> params: PlaneParams;
@group(0) @binding(1) var canvas: texture_2d<f32>;
@group(0) @binding(2) var canvas_sampler: sampler;

// The two triangles of a quad, as (along, across) corner weights.
fn quad_corner(index: u32) -> vec2<f32> {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    return corners[index];
}

// The rectangle's corners, in order around it.
fn rect_corner(index: u32) -> vec2<f32> {
    let r = params.rect;
    var corners = array<vec2<f32>, 4>(
        vec2<f32>(r.x, r.y),
        vec2<f32>(r.z, r.y),
        vec2<f32>(r.z, r.w),
        vec2<f32>(r.x, r.w),
    );
    return corners[index % 4u];
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> PlaneVertex {
    var out: PlaneVertex;
    var position: vec3<f32>;
    if (index < TOP_VERTICES) {
        let weights = quad_corner(index);
        position = vec3<f32>(mix(params.rect.xy, params.rect.zw, weights), 0.0);
        out.uv = weights;
        out.side = 0u;
    } else {
        let side = (index - TOP_VERTICES) / VERTICES_PER_SIDE;
        let weights = quad_corner((index - TOP_VERTICES) % VERTICES_PER_SIDE);
        let along = mix(rect_corner(side), rect_corner(side + 1u), weights.x);
        position = vec3<f32>(along, -params.body.x * weights.y);
        out.uv = vec2<f32>(0.0);
        out.side = 1u;
    }
    out.clip = params.view_proj * vec4<f32>(position, 1.0);
    return out;
}

@fragment
fn fs_main(in: PlaneVertex) -> @location(0) vec4<f32> {
    // Sampled outside the branch: textureSample needs uniform control flow.
    let colour = vec4<f32>(textureSample(canvas, canvas_sampler, in.uv).rgb, 1.0);
    if (in.side != 0u) {
        return EDGE_COLOUR;
    }
    return colour;
}
