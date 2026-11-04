@group(0) @binding(0) var<uniform> slots: PigmentSlots;
@group(0) @binding(1) var<storage, read> tiles: array<vec2<u32>>;
@group(0) @binding(2) var wet_pig0: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(3) var wet_pig1: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(4) var wet_vol: texture_storage_2d<r32float, read_write>;
@group(0) @binding(5) var dry_refl0: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(6) var dry_refl1: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(7) var dry_height: texture_storage_2d<r32float, read_write>;
@group(0) @binding(8) var dry_top_pig0: texture_storage_2d<rgba16float, write>;
@group(0) @binding(9) var dry_top_pig1: texture_storage_2d<rgba16float, write>;

fn dry_band(fractions_low: vec4<f32>, fractions_high: vec4<f32>, volume: f32, below: f32, band: u32) -> f32 {
    let wet = km_layer(
        mixture_absorption(fractions_low, fractions_high, band),
        mixture_scattering(fractions_low, fractions_high, band),
        volume,
    );
    return nearest_half(composite_over(wet, below));
}

fn dry_bands(fractions_low: vec4<f32>, fractions_high: vec4<f32>, volume: f32, below: vec4<f32>, first_band: u32) -> vec4<f32> {
    return vec4<f32>(
        dry_band(fractions_low, fractions_high, volume, below.x, first_band),
        dry_band(fractions_low, fractions_high, volume, below.y, first_band + 1u),
        dry_band(fractions_low, fractions_high, volume, below.z, first_band + 2u),
        dry_band(fractions_low, fractions_high, volume, below.w, first_band + 3u),
    );
}

@compute @workgroup_size(DRY_WORKGROUP_SIZE, DRY_WORKGROUP_SIZE)
fn main(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_id) local: vec3<u32>,
) {
    let size = textureDimensions(wet_vol);
    let texel = tiles[group.z] * TILE_SIZE + group.xy * DRY_WORKGROUP_SIZE + local.xy;
    if (texel.x >= size.x || texel.y >= size.y) {
        return;
    }
    let pixel = vec2<i32>(texel);
    let volume = textureLoad(wet_vol, pixel).x;
    if (volume > 0.0) {
        let fractions_low = textureLoad(wet_pig0, pixel);
        let fractions_high = textureLoad(wet_pig1, pixel);
        textureStore(dry_refl0, pixel, dry_bands(fractions_low, fractions_high, volume, textureLoad(dry_refl0, pixel), 0u));
        textureStore(dry_refl1, pixel, dry_bands(fractions_low, fractions_high, volume, textureLoad(dry_refl1, pixel), 4u));
        textureStore(dry_height, pixel, vec4<f32>(textureLoad(dry_height, pixel).x + volume, 0.0, 0.0, 0.0));
        if (volume > MIN_TOP_LAYER_UM) {
            textureStore(dry_top_pig0, pixel, fractions_low);
            textureStore(dry_top_pig1, pixel, fractions_high);
        }
    }
    textureStore(wet_pig0, pixel, vec4<f32>(0.0));
    textureStore(wet_pig1, pixel, vec4<f32>(0.0));
    textureStore(wet_vol, pixel, vec4<f32>(0.0));
}
