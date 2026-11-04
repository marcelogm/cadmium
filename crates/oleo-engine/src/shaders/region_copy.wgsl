@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var wet_pig0: texture_storage_2d<rgba16float, read>;
@group(0) @binding(2) var wet_pig1: texture_storage_2d<rgba16float, read>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, read>;
@group(0) @binding(4) var scratch_pig0: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var scratch_pig1: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var scratch_vol: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let pixel = dab.canvas_origin + vec2<i32>(id.xy);
    textureStore(scratch_pig0, pixel, textureLoad(wet_pig0, pixel));
    textureStore(scratch_pig1, pixel, textureLoad(wet_pig1, pixel));
    textureStore(scratch_vol, pixel, textureLoad(wet_vol, pixel));
}
