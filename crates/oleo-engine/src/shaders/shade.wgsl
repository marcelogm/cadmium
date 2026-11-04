struct ShadeParams {
    xyz_weights: array<vec4<f32>, 6>,
    impasto_scale: f32,
    light_azimuth_deg: f32,
    light_elevation_deg: f32,
    wet_gloss: f32,
    dry_roughness: f32,
    view_mode: u32,
    view_slot: u32,
    _pad: u32,
};

const TILE_SIZE: u32 = 64u;
const WORKGROUP_SIZE: u32 = 8u;

const VIEW_COLOR: u32 = 0u;
const VIEW_HEIGHT: u32 = 1u;
const VIEW_WETNESS: u32 = 2u;

const SURFACE_REFLECTION_K1: f32 = 0.04;
const INTERNAL_REFLECTION_K2: f32 = 0.6;

const XYZ_TO_LINEAR_SRGB: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>(3.2406, -0.9689, 0.0557),
    vec3<f32>(-1.5372, 1.8758, -0.2040),
    vec3<f32>(-0.4986, 0.0415, 1.0570),
);

const HEIGHT_UM_PER_PIXEL: f32 = 100.0;
const AMBIENT: f32 = 0.35;
const MIN_LIGHT_ELEVATION_SINE: f32 = 0.05;
const MAX_DIFFUSE: f32 = 1.6;
const F0: f32 = 0.04;
const WET_GLOSS_SMOOTHING: f32 = 0.85;
const FULLY_WET_UM: f32 = 5.0;
const MIN_COSINE: f32 = 1e-4;
const PI: f32 = 3.141592654;

const HEIGHT_VIEW_FULL_SCALE_UM: f32 = 100.0;
const VOLUME_VIEW_FULL_SCALE_UM: f32 = 200.0;

@group(0) @binding(0) var<uniform> slots: PigmentSlots;
@group(0) @binding(1) var<uniform> params: ShadeParams;
@group(0) @binding(2) var<storage, read> tiles: array<vec2<u32>>;
@group(0) @binding(3) var wet_pig0: texture_storage_2d<rgba16float, read>;
@group(0) @binding(4) var wet_pig1: texture_storage_2d<rgba16float, read>;
@group(0) @binding(5) var wet_vol: texture_storage_2d<r32float, read>;
@group(0) @binding(6) var dry_refl0: texture_storage_2d<rgba16float, read>;
@group(0) @binding(7) var dry_refl1: texture_storage_2d<rgba16float, read>;
@group(0) @binding(8) var dry_height: texture_storage_2d<r32float, read>;
@group(0) @binding(9) var display: texture_storage_2d<rgba16float, write>;

fn saunderson_display(reflectance: f32) -> f32 {
    return (1.0 - SURFACE_REFLECTION_K1) * (1.0 - INTERNAL_REFLECTION_K2) * reflectance
        / (1.0 - INTERNAL_REFLECTION_K2 * reflectance);
}

fn clamped(pixel: vec2<i32>, size: vec2<u32>) -> vec2<i32> {
    return clamp(pixel, vec2<i32>(0), vec2<i32>(size) - vec2<i32>(1));
}

fn surface_height(pixel: vec2<i32>, size: vec2<u32>) -> f32 {
    let p = clamped(pixel, size);
    return textureLoad(dry_height, p).x + textureLoad(wet_vol, p).x;
}

fn albedo(fractions_low: vec4<f32>, fractions_high: vec4<f32>, volume: f32, below_low: vec4<f32>, below_high: vec4<f32>) -> vec3<f32> {
    var xyz = vec3<f32>(0.0);
    for (var b = 0u; b < 8u; b++) {
        let band_vec = b / 4u;
        let lane = b % 4u;
        let k = mixture_absorption(fractions_low, fractions_high, b);
        let s = mixture_scattering(fractions_low, fractions_high, b);
        let below = slot_lane(below_low, below_high, b);
        let shown = saunderson_display(composite_over(km_layer(k, s, volume), below));
        xyz += shown * vec3<f32>(
            params.xyz_weights[band_vec][lane],
            params.xyz_weights[2u + band_vec][lane],
            params.xyz_weights[4u + band_vec][lane],
        );
    }
    return XYZ_TO_LINEAR_SRGB * xyz;
}

fn light_direction() -> vec3<f32> {
    let azimuth = radians(params.light_azimuth_deg);
    let elevation = radians(params.light_elevation_deg);
    return vec3<f32>(
        cos(azimuth) * cos(elevation),
        -sin(azimuth) * cos(elevation),
        sin(elevation),
    );
}

fn ggx_specular(normal: vec3<f32>, light: vec3<f32>, roughness: f32) -> f32 {
    let view = vec3<f32>(0.0, 0.0, 1.0);
    let n_dot_l = dot(normal, light);
    let n_dot_v = max(dot(normal, view), MIN_COSINE);
    if (n_dot_l <= 0.0) {
        return 0.0;
    }
    let half_vector = normalize(light + view);
    let n_dot_h = max(dot(normal, half_vector), 0.0);
    let v_dot_h = max(dot(view, half_vector), 0.0);
    let alpha = roughness * roughness;
    let alpha2 = alpha * alpha;
    let d_denominator = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
    let distribution = alpha2 / (PI * d_denominator * d_denominator);
    let fresnel = F0 + (1.0 - F0) * pow(1.0 - v_dot_h, 5.0);
    let k = alpha / 2.0;
    let shadowing = (n_dot_l / (n_dot_l * (1.0 - k) + k)) * (n_dot_v / (n_dot_v * (1.0 - k) + k));
    return distribution * fresnel * shadowing / (4.0 * n_dot_l * n_dot_v) * n_dot_l;
}

@compute @workgroup_size(8, 8)
fn main(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_id) local: vec3<u32>,
) {
    let size = textureDimensions(display);
    let tile = tiles[group.z];
    let texel = tile * TILE_SIZE + group.xy * WORKGROUP_SIZE + local.xy;
    if (texel.x >= size.x || texel.y >= size.y) {
        return;
    }
    let pixel = vec2<i32>(texel);
    let fractions_low = textureLoad(wet_pig0, pixel);
    let fractions_high = textureLoad(wet_pig1, pixel);
    let volume = textureLoad(wet_vol, pixel).x;
    let height = textureLoad(dry_height, pixel).x + volume;

    if (params.view_mode == VIEW_HEIGHT) {
        textureStore(display, pixel, vec4<f32>(vec3<f32>(height / HEIGHT_VIEW_FULL_SCALE_UM), 1.0));
        return;
    }
    if (params.view_mode == VIEW_WETNESS) {
        textureStore(display, pixel, vec4<f32>(vec3<f32>(volume / VOLUME_VIEW_FULL_SCALE_UM), 1.0));
        return;
    }
    if (params.view_mode != VIEW_COLOR) {
        let amount = slot_lane(fractions_low, fractions_high, params.view_slot) * volume;
        textureStore(display, pixel, vec4<f32>(vec3<f32>(amount / VOLUME_VIEW_FULL_SCALE_UM), 1.0));
        return;
    }

    let rgb = albedo(fractions_low, fractions_high, volume, textureLoad(dry_refl0, pixel), textureLoad(dry_refl1, pixel));

    let slope_scale = params.impasto_scale / (2.0 * HEIGHT_UM_PER_PIXEL);
    let dhdx = (surface_height(pixel + vec2<i32>(1, 0), size) - surface_height(pixel - vec2<i32>(1, 0), size)) * slope_scale;
    let dhdy = (surface_height(pixel + vec2<i32>(0, 1), size) - surface_height(pixel - vec2<i32>(0, 1), size)) * slope_scale;
    let normal = normalize(vec3<f32>(-dhdx, -dhdy, 1.0));
    let light = light_direction();
    let lambert = max(dot(normal, light), 0.0) / max(light.z, MIN_LIGHT_ELEVATION_SINE);
    let diffuse = clamp(AMBIENT + (1.0 - AMBIENT) * lambert, 0.0, MAX_DIFFUSE);

    let wetness = smoothstep(0.0, FULLY_WET_UM, volume);
    let roughness = mix(params.dry_roughness, 1.0 - WET_GLOSS_SMOOTHING * params.wet_gloss, wetness);
    let specular = ggx_specular(normal, light, roughness);

    textureStore(display, pixel, vec4<f32>(rgb * diffuse + vec3<f32>(specular), 1.0));
}
