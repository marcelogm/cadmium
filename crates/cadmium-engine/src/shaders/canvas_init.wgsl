struct InitParams {
    primer_bands_low: vec4<f32>,
    primer_bands_high: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: InitParams;
@group(0) @binding(1) var wet_pig0: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var wet_pig1: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, write>;
@group(0) @binding(4) var dry_refl0: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var dry_refl1: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var dry_height: texture_storage_2d<r32float, write>;
@group(0) @binding(7) var dry_top_pig0: texture_storage_2d<rgba16float, write>;
@group(0) @binding(8) var dry_top_pig1: texture_storage_2d<rgba16float, write>;
@group(0) @binding(9) var display: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(dry_height);
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let pixel = vec2<i32>(id.xy);
    let empty = vec4<f32>(0.0);
    textureStore(wet_pig0, pixel, empty);
    textureStore(wet_pig1, pixel, empty);
    textureStore(wet_vol, pixel, empty);
    textureStore(dry_refl0, pixel, params.primer_bands_low);
    textureStore(dry_refl1, pixel, params.primer_bands_high);
    textureStore(dry_height, pixel, empty);
    textureStore(dry_top_pig0, pixel, empty);
    textureStore(dry_top_pig1, pixel, empty);
    textureStore(display, pixel, empty);
}
