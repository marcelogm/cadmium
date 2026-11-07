const TOOTH_LIGHT_UM: f32 = 2.0;
const TOOTH_FIRM_UM: f32 = -8.0;
const TOOTH_SOFTNESS_UM: f32 = 2.0;
const TOOTH_REACH_PX: i32 = 3;
const NEIGHBOUR_SHARE: f32 = 0.25;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var graphite: texture_storage_2d<r32float, read_write>;
@group(0) @binding(2) var dry_height: texture_storage_2d<r32float, read_write>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, read_write>;

fn surface_um(pixel: vec2<i32>) -> f32 {
    let size = vec2<i32>(textureDimensions(graphite));
    let p = clamp(pixel, vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(dry_height, p).x + textureLoad(wet_vol, p).x;
}

fn relief_um(pixel: vec2<i32>) -> f32 {
    let across = vec2<i32>(TOOTH_REACH_PX, 0);
    let down = vec2<i32>(0, TOOTH_REACH_PX);
    let around = surface_um(pixel + across) + surface_um(pixel - across)
        + surface_um(pixel + down) + surface_um(pixel - down);
    return surface_um(pixel) - NEIGHBOUR_SHARE * around;
}

fn tooth(pixel: vec2<i32>) -> f32 {
    let threshold = mix(TOOTH_LIGHT_UM, TOOTH_FIRM_UM, dab.pressure);
    return smoothstep(
        threshold - TOOTH_SOFTNESS_UM,
        threshold + TOOTH_SOFTNESS_UM,
        relief_um(pixel)
    );
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let pixel = dab.canvas_origin + vec2<i32>(id.xy);
    let mask = footprint(brush_space(pixel));
    if (mask <= 0.0) {
        return;
    }
    let deposit = clamp(dab.tool_strength * dab.pressure * mask * tooth(pixel), 0.0, 1.0);
    let density = textureLoad(graphite, pixel).x;
    textureStore(graphite, pixel, vec4<f32>(density + deposit * (1.0 - density), 0.0, 0.0, 0.0));
}
