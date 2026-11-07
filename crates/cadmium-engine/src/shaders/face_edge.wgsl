// How well each stretch of the mixing knife's edge meets the tray. A real blade
// never sits evenly: some stretches press into the paint, others ride over it. A
// smooth, seeded 1D noise along the blade gives every row a contact in
// [1 - unevenness, 1]; rows that press harder pick up and lay more, so a pass
// drags the paint in streaks. Each stroke draws a new pattern from its seed, so
// later passes cut across the earlier streaks and fold them finer.

const EDGE_COARSE_PX: f32 = 11.0;
const EDGE_FINE_PX: f32 = 4.0;
const EDGE_COARSE_SHARE: f32 = 0.65;
/// Where the noise lifts a stretch off the paint: a narrow band, so the edge
/// splits into stretches that press and stretches that ride, not a gentle ripple.
const EDGE_LIFT_FROM: f32 = 0.4;
const EDGE_LIFT_TO: f32 = 0.6;
const EDGE_FINE_SEED_STEP: u32 = 0x9E3779B9u;

fn edge_hash(value: u32) -> f32 {
    var x = value * 747796405u + 2891336453u;
    x = ((x >> ((x >> 28u) + 4u)) ^ x) * 277803737u;
    x = (x >> 22u) ^ x;
    return f32(x) / 4294967295.0;
}

fn edge_noise(row: f32, wavelength_px: f32, seed: u32) -> f32 {
    let t = row / wavelength_px;
    let knot = floor(t);
    let f = t - knot;
    let index = u32(i32(knot) + 0x10000);
    let low = edge_hash(seed ^ (index * 2654435761u));
    let high = edge_hash(seed ^ ((index + 1u) * 2654435761u));
    return mix(low, high, f * f * (3.0 - 2.0 * f));
}

/// The contact of the blade row at `along_px` from the blade's middle.
fn edge_contact(along_px: f32) -> f32 {
    let row = floor(along_px) + 0.5;
    let seed = settings.edge_seed;
    let coarse = edge_noise(row, EDGE_COARSE_PX, seed);
    let fine = edge_noise(row, EDGE_FINE_PX, seed + EDGE_FINE_SEED_STEP);
    let rough = mix(fine, coarse, EDGE_COARSE_SHARE);
    let lifted = smoothstep(EDGE_LIFT_FROM, EDGE_LIFT_TO, rough);
    return 1.0 - clamp(settings.edge_unevenness, 0.0, 1.0) * lifted;
}
