// Paint moved between a blade and the surface under it, shared by the knife's
// blade and the mixing knife's face. The including shader declares `dab` and the
// wet and dry textures.

const QUANTIZE_LIMIT: f32 = 2.0e9;
const EMPTY_VOLUME_UM: f32 = 1e-4;
const GRANT_SAFETY: f32 = 0.999999;
const NO_PEAK: f32 = -3.0e38;

fn quantize(amount: f32) -> i32 {
    return i32(round(clamp(amount * BLADE_DELTA_SCALE, -QUANTIZE_LIMIT, QUANTIZE_LIMIT)));
}

fn quantize_down(amount: f32) -> i32 {
    return i32(floor(clamp(amount * BLADE_DELTA_SCALE, -QUANTIZE_LIMIT, QUANTIZE_LIMIT)));
}

fn dequantize(amount: i32) -> f32 {
    return f32(amount) / BLADE_DELTA_SCALE;
}

fn region_pixel(index: u32) -> vec2<i32> {
    let width = dab.canvas_size.x;
    return dab.canvas_origin + vec2<i32>(vec2<u32>(index % width, index / width));
}

fn lateral_cover(position: vec2<f32>) -> f32 {
    let inset = semi_axes() - abs(position);
    if (inset.x + BLADE_COVERAGE_HALF_PX <= 0.0) {
        return 0.0;
    }
    return clamp(inset.y + BLADE_COVERAGE_HALF_PX, 0.0, 1.0);
}

fn cell_of(position: vec2<f32>) -> u32 {
    return u32(clamp(i32(floor(position.y)) + TIP_HALF, 0, i32(TIP_SIZE) - 1));
}

fn read_fractions(pixel: vec2<i32>) -> array<f32, 8> {
    let low = textureLoad(wet_pig0, pixel);
    let high = textureLoad(wet_pig1, pixel);
    return array<f32, 8>(low.x, low.y, low.z, low.w, high.x, high.y, high.z, high.w);
}

fn store_paint(pixel: vec2<i32>, volume: f32, amounts: array<f32, 8>) {
    var fractions = array<f32, 8>();
    if (volume >= EMPTY_VOLUME_UM) {
        for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
            fractions[slot] = nearest_half(max(amounts[slot], 0.0) / volume);
        }
        fractions = within_whole_volume(fractions);
    }
    textureStore(wet_vol, pixel, vec4<f32>(volume, 0.0, 0.0, 0.0));
    textureStore(wet_pig0, pixel, vec4<f32>(fractions[0], fractions[1], fractions[2], fractions[3]));
    textureStore(wet_pig1, pixel, vec4<f32>(fractions[4], fractions[5], fractions[6], fractions[7]));
}

fn centre_dry_height() -> f32 {
    let size = vec2<i32>(textureDimensions(dry_height));
    let pixel = clamp(vec2<i32>(floor(dab.center)), vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(dry_height, pixel).x;
}

/// A pigment's share of what a blade cell holds, or nothing while it is empty.
fn held_share(amount: f32, held: f32) -> f32 {
    if (held <= EMPTY_VOLUME_UM) {
        return 0.0;
    }
    return max(amount, 0.0) / held;
}

/// Lays `deposit_q` of a blade cell's paint, mixed as `shares`, onto `pixel`
/// holding `volume`, and takes `pickup_q` of the pixel's own paint up. Stores the
/// pixel and returns what the cell gains per channel, quantised, for the caller
/// to add to its deltas: every unit leaving one side reaches the other.
fn trade(
    pixel: vec2<i32>,
    volume: f32,
    deposit_q: i32,
    pickup_q: i32,
    shares: array<f32, 8>,
) -> array<i32, TIP_CHANNELS> {
    let deposit = dequantize(deposit_q);
    let pickup = dequantize(pickup_q);
    let fractions = read_fractions(pixel);
    var amounts = array<f32, 8>();
    var gained = array<i32, TIP_CHANNELS>();
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        let deposit_slot_q = max(quantize_down(deposit * shares[slot]), 0);
        let pickup_slot_q = max(quantize(pickup * fractions[slot]), 0);
        amounts[slot] = fractions[slot] * volume + dequantize(deposit_slot_q) - dequantize(pickup_slot_q);
        gained[slot] = pickup_slot_q - deposit_slot_q;
    }
    gained[VOLUME_CHANNEL] = pickup_q - deposit_q;
    store_paint(pixel, volume - pickup + deposit, amounts);
    return gained;
}
