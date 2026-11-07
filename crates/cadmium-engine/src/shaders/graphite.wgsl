const GRAPHITE_MAX: f32 = 0.85;
const GRAPHITE_BANDS_LOW: vec4<f32> = vec4<f32>(0.14, 0.135, 0.13, 0.125);
const GRAPHITE_BANDS_HIGH: vec4<f32> = vec4<f32>(0.12, 0.118, 0.116, 0.115);

fn under_graphite_low(below: vec4<f32>, density: f32) -> vec4<f32> {
    return mix(below, GRAPHITE_BANDS_LOW, density * GRAPHITE_MAX);
}

fn under_graphite_high(below: vec4<f32>, density: f32) -> vec4<f32> {
    return mix(below, GRAPHITE_BANDS_HIGH, density * GRAPHITE_MAX);
}
