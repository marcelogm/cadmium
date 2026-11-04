const OPTICAL_DEPTH_LIMIT: f32 = 20.0;
const NEGLIGIBLE_BETA: f32 = 1e-7;

struct Layer {
    reflectance: f32,
    transmittance: f32,
};

fn km_layer(absorption: f32, scattering: f32, thickness_um: f32) -> Layer {
    let k = max(absorption, 0.0);
    let s = max(scattering, 0.0);
    let x = max(thickness_um, 0.0);
    let beta = sqrt(k * (k + 2.0 * s));
    if (beta < NEGLIGIBLE_BETA) {
        let sx = s * x;
        return Layer(sx / (1.0 + sx), 1.0 / (1.0 + sx));
    }
    let optical_depth = beta * x;
    let bx = min(optical_depth, OPTICAL_DEPTH_LIMIT);
    let d = (k + s) * sinh(bx) + beta * cosh(bx);
    var transmittance = beta / d;
    if (optical_depth >= OPTICAL_DEPTH_LIMIT) {
        transmittance = 0.0;
    }
    return Layer(s * sinh(bx) / d, transmittance);
}

fn composite_over(top: Layer, below_reflectance: f32) -> f32 {
    return top.reflectance
        + top.transmittance * top.transmittance * below_reflectance
            / (1.0 - top.reflectance * below_reflectance);
}

fn slot_lane(low: vec4<f32>, high: vec4<f32>, index: u32) -> f32 {
    if (index < 4u) {
        return low[index];
    }
    return high[index - 4u];
}

fn mixture_absorption(fractions_low: vec4<f32>, fractions_high: vec4<f32>, band: u32) -> f32 {
    var k = 0.0;
    for (var slot = 0u; slot < slots.count; slot++) {
        k += slot_lane(fractions_low, fractions_high, slot) * slots.absorption[2u * slot + band / 4u][band % 4u];
    }
    return k;
}

fn mixture_scattering(fractions_low: vec4<f32>, fractions_high: vec4<f32>, band: u32) -> f32 {
    var s = 0.0;
    for (var slot = 0u; slot < slots.count; slot++) {
        s += slot_lane(fractions_low, fractions_high, slot) * slots.scattering[2u * slot + band / 4u][band % 4u];
    }
    return s;
}
