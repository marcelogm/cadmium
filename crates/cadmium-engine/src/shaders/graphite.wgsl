const GRAPHITE_MAX_COVERAGE: f32 = 0.85;
const GRAPHITE_BANDS_LOW: vec4<f32> = vec4<f32>(0.14, 0.135, 0.13, 0.125);
const GRAPHITE_BANDS_HIGH: vec4<f32> = vec4<f32>(0.12, 0.118, 0.116, 0.115);

fn under_graphite(below: vec4<f32>, bands: vec4<f32>, density: f32) -> vec4<f32> {
    return mix(below, bands, clamp(density, 0.0, 1.0) * GRAPHITE_MAX_COVERAGE);
}
