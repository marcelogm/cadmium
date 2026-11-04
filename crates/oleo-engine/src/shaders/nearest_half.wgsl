const HALF_SIGNIFICAND_BITS: i32 = 11;
const HALF_SMALLEST_STEP_EXPONENT: i32 = -24;

fn nearest_half(value: f32) -> f32 {
    let parts = frexp(value);
    let step = max(parts.exp - HALF_SIGNIFICAND_BITS, HALF_SMALLEST_STEP_EXPONENT);
    return ldexp(round(ldexp(value, -step)), step);
}
