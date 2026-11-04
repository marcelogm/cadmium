const HALF_SIGNIFICAND_BITS: i32 = 11;
const HALF_SMALLEST_STEP_EXPONENT: i32 = -24;

fn nearest_half(value: f32) -> f32 {
    let parts = frexp(value);
    let step = max(parts.exp - HALF_SIGNIFICAND_BITS, HALF_SMALLEST_STEP_EXPONENT);
    return ldexp(round(ldexp(value, -step)), step);
}

fn half_step(value: f32) -> f32 {
    let parts = frexp(value);
    return ldexp(1.0, max(parts.exp - HALF_SIGNIFICAND_BITS, HALF_SMALLEST_STEP_EXPONENT));
}

fn fraction_total(fractions: array<f32, 8>) -> f32 {
    var total = 0.0;
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        total += fractions[slot];
    }
    return total;
}

fn within_whole_volume(rounded: array<f32, 8>) -> array<f32, 8> {
    var fractions = rounded;
    let excess = fraction_total(fractions) - 1.0;
    if (excess <= 0.0) {
        return fractions;
    }
    var largest = 0u;
    for (var slot = 1u; slot < PIGMENT_CHANNELS; slot++) {
        if (fractions[slot] > fractions[largest]) {
            largest = slot;
        }
    }
    fractions[largest] = nearest_half(max(fractions[largest] - excess, 0.0));
    if (fraction_total(fractions) > 1.0) {
        fractions[largest] = max(fractions[largest] - half_step(fractions[largest]), 0.0);
    }
    return fractions;
}
