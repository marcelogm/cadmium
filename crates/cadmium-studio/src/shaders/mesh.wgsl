// The ferrule and the handle, posed from the engine's handle state. Follows
// the engine's bristle view module, which supplies `HandleState`, `pose_of`,
// `tilted` and `spun`.

struct ViewParams {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    toward_light: vec4<f32>,
};

struct MeshParams {
    // Linear RGB; w unused.
    colour: vec4<f32>,
    // x: highlight strength; y: highlight exponent; zw unused.
    highlight: vec4<f32>,
};

struct MeshIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct MeshVertex {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

const AMBIENT: f32 = 0.15;

@group(0) @binding(0) var<uniform> view: ViewParams;
@group(1) @binding(0) var<uniform> params: MeshParams;
@group(1) @binding(1) var<storage, read> handle: HandleState;
// Read only by the module's `bristle_vertex`, which the meshes never call.
@group(1) @binding(2) var<storage, read> states: array<vec4<f32>>;

// Ferrule-local (x across, y along, z up the axis) to canvas space, turning
// but not moving.
fn posed(local: vec3<f32>, pose: Pose) -> vec3<f32> {
    return tilted(vec3<f32>(spun(local.xy, pose.orientation), local.z), pose.axis);
}

@vertex
fn vs_main(in: MeshIn) -> MeshVertex {
    let pose = pose_of(handle);
    let world = pose.ferrule + posed(in.position, pose);
    var out: MeshVertex;
    out.clip = view.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.normal = posed(in.normal, pose);
    return out;
}

@fragment
fn fs_main(in: MeshVertex) -> @location(0) vec4<f32> {
    let toward_eye = normalize(view.eye.xyz - in.world);
    // Both faces are drawn (the tubes are open), so light the side in view.
    var normal = normalize(in.normal);
    if (dot(normal, toward_eye) < 0.0) {
        normal = -normal;
    }
    let toward_light = view.toward_light.xyz;
    let light = max(dot(normal, toward_light), 0.0);
    let halfway = normalize(toward_light + toward_eye);
    let shine = params.highlight.x * pow(max(dot(normal, halfway), 0.0), params.highlight.y);
    return vec4<f32>(params.colour.rgb * (light + AMBIENT) + vec3<f32>(shine), 1.0);
}
