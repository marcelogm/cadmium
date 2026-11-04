const DRAG_SPEED_GAIN: f32 = 4.0;
const MAX_COURANT: f32 = 0.9;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var wet_pig0: texture_storage_2d<rgba16float, read>;
@group(0) @binding(2) var wet_pig1: texture_storage_2d<rgba16float, read>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, read>;
@group(0) @binding(4) var scratch_pig0: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var scratch_pig1: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var scratch_vol: texture_storage_2d<r32float, write>;
@group(0) @binding(7) var scratch_motion: texture_storage_2d<rgba32float, write>;

fn kept_at_canvas_border(pixel: vec2<i32>, velocity: vec2<f32>) -> vec2<f32> {
    let last = vec2<i32>(textureDimensions(wet_vol)) - vec2<i32>(1);
    var inward = velocity;
    if (pixel.x == 0) {
        inward.x = max(inward.x, 0.0);
    }
    if (pixel.x == last.x) {
        inward.x = min(inward.x, 0.0);
    }
    if (pixel.y == 0) {
        inward.y = max(inward.y, 0.0);
    }
    if (pixel.y == last.y) {
        inward.y = min(inward.y, 0.0);
    }
    return inward;
}

fn outflow(velocity: vec2<f32>) -> f32 {
    return abs(velocity.x) + abs(velocity.y);
}

fn motion(pixel: vec2<i32>, volume: f32) -> vec4<f32> {
    let position = brush_space(pixel);
    let outline = outline_footprint(position);
    let squeeze_gradient = pressure_envelope_gradient(position);
    if (outline <= 0.0 && all(squeeze_gradient == vec2<f32>(0.0))) {
        return vec4<f32>(0.0);
    }
    let heading = vec2<f32>(cos(dab.direction), sin(dab.direction));
    let drag = dab.advection * min(dab.speed * DRAG_SPEED_GAIN, 1.0);
    var velocity = kept_at_canvas_border(pixel, outline * drag * heading - dab.squeeze * squeeze_gradient);
    let courant = outflow(velocity);
    if (courant > MAX_COURANT) {
        velocity *= MAX_COURANT / courant;
    }
    var movable_share = 0.0;
    if (volume > dab.residual_um) {
        movable_share = (volume - dab.residual_um) / volume;
    }
    let leaving = outflow(velocity);
    if (leaving > movable_share) {
        velocity *= movable_share / leaving;
    }
    let diffusion = min(dab.diffusion * outline, 1.0 - outflow(velocity));
    return vec4<f32>(velocity, diffusion, 0.0);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let local = vec2<i32>(id.xy);
    let pixel = dab.canvas_origin + local;
    let volume = textureLoad(wet_vol, pixel);
    textureStore(scratch_pig0, local, textureLoad(wet_pig0, pixel));
    textureStore(scratch_pig1, local, textureLoad(wet_pig1, pixel));
    textureStore(scratch_vol, local, volume);
    textureStore(scratch_motion, local, motion(pixel, volume.x));
}
