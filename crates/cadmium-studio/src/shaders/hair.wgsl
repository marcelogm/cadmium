// The live bristles as camera-facing ribbons: one instance per chain segment,
// six vertices (a quad) each. Follows the engine's bristle view module, which
// supplies `BristleRest`, `bristle_vertex` and `BRISTLE_SEGMENTS`.

struct ViewParams {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    toward_light: vec4<f32>,
};

struct HairParams {
    // Linear RGB; w unused.
    tint: vec4<f32>,
    // x: half-width per rest radius; y: least half-width, px; z: lowest
    // height, px; w unused.
    width: vec4<f32>,
};

struct HairVertex {
    @builtin(position) clip: vec4<f32>,
    // -1 at one edge of the ribbon, 1 at the other.
    @location(0) across: f32,
    // Unit vectors: from one edge toward the other, and toward the eye.
    @location(1) side: vec3<f32>,
    @location(2) facing: vec3<f32>,
};

const AMBIENT: f32 = 0.15;
const RIM: f32 = 0.2;

@group(0) @binding(0) var<uniform> view: ViewParams;
@group(1) @binding(0) var<uniform> params: HairParams;
@group(1) @binding(1) var<storage, read> states: array<vec4<f32>>;
@group(1) @binding(2) var<storage, read> rests: array<BristleRest>;

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

// `v` at unit length, or `fallback` when it has (almost) none.
fn unit_or(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    if (len > 1e-6) {
        return v / len;
    }
    return fallback;
}

@vertex
fn vs_main(
    @builtin(vertex_index) index: u32,
    @builtin(instance_index) instance: u32,
) -> HairVertex {
    let bristle = instance / BRISTLE_SEGMENTS;
    let segment = instance % BRISTLE_SEGMENTS;
    let start = bristle_vertex(bristle, segment).xyz;
    let end = bristle_vertex(bristle, segment + 1u).xyz;
    let weights = quad_corner(index);
    let centre = mix(start, end, weights.x);
    let along = unit_or(end - start, vec3<f32>(0.0, 0.0, 1.0));
    let facing = unit_or(view.eye.xyz - centre, vec3<f32>(0.0, 0.0, 1.0));
    let side = unit_or(cross(along, facing), vec3<f32>(1.0, 0.0, 0.0));
    let half_width = max(rests[bristle].radius_px * params.width.x, params.width.y);
    let across = weights.y * 2.0 - 1.0;
    var position = centre + side * (across * half_width);
    position.z = max(position.z, params.width.z);
    var out: HairVertex;
    out.clip = view.view_proj * vec4<f32>(position, 1.0);
    out.across = across;
    out.side = side;
    out.facing = facing;
    return out;
}

@fragment
fn fs_main(in: HairVertex) -> @location(0) vec4<f32> {
    // The ribbon stands in for a cylinder: its normal turns from the eye at
    // the centre line to the side at the edges.
    let across = clamp(in.across, -1.0, 1.0);
    let normal = normalize(in.side * across + in.facing * sqrt(1.0 - across * across));
    let light = max(dot(normal, view.toward_light.xyz), 0.0);
    let rim = RIM * pow(1.0 - max(dot(normal, normalize(in.facing)), 0.0), 3.0);
    return vec4<f32>(params.tint.rgb * (light + AMBIENT) + vec3<f32>(rim), 1.0);
}
