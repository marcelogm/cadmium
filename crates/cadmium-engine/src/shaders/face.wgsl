// The mixing knife's face: a 2D layer of paint riding under the blade, FACE_DEPTH
// cells across it (blade x, the direction of travel) by TIP_SIZE cells along it
// (blade y). Each dab every pixel under the blade trades paint with the cell over
// it: the cell lays a share of what it carries and scrapes up a share of what
// stands above the blade's plane. Cells keep their place on the blade, so what is
// picked up is carried forward and laid further on, stretching the paint along
// the stroke while keeping its streaks across it; partial shares mean each pass
// mixes only a little more.

struct FaceSettings {
    pickup_rate: f32,
    lay_rate: f32,
    smear: f32,
    edge_unevenness: f32,
    edge_seed: u32,
    padding_a: u32,
    padding_b: u32,
    padding_c: u32,
};

struct FaceState {
    rest: f32,
    plane: f32,
};

const WANT_CHANNEL: u32 = TIP_CHANNELS;
const GRANT_LAY: u32 = 0u;
const GRANT_ROOM: u32 = 1u;
const GRANT_SHARES: u32 = 2u;
const FACE_THREADS: u32 = 256u;
/// Paint this far above the blade's plane is scraped at the full rate; thinner
/// paint proportionally less, so the knife takes the heaps and leaves the film.
const FULL_SCRAPE_UM: f32 = 100.0;

var<workgroup> reduced: array<f32, FACE_THREADS>;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var wet_pig0: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(2) var wet_pig1: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, read_write>;
@group(0) @binding(4) var dry_height: texture_storage_2d<r32float, read>;
@group(0) @binding(5) var<storage, read_write> face_load: array<f32>;
@group(0) @binding(6) var<storage, read_write> face_delta: array<atomic<i32>>;
@group(0) @binding(7) var<storage, read_write> face_grant: array<f32>;
@group(0) @binding(8) var<storage, read_write> face: FaceState;
@group(0) @binding(9) var<uniform> settings: FaceSettings;

/// The columns this blade's width spreads over: about a pixel each, so every column
/// meets paint as the blade moves, up to FACE_DEPTH on the widest blades. A blade
/// resized mid-life keeps whatever sits in columns it no longer reaches until it
/// grows back or is cleaned.
fn face_columns() -> u32 {
    let wide = round(2.0 * semi_axes().x / FACE_COLUMN_PX);
    return u32(clamp(wide, 1.0, f32(FACE_DEPTH)));
}

fn face_column(position: vec2<f32>) -> u32 {
    let columns = face_columns();
    let across = position.x / semi_axes().x * 0.5 + 0.5;
    return u32(clamp(i32(floor(across * f32(columns))), 0, i32(columns) - 1));
}

fn face_cell(position: vec2<f32>) -> u32 {
    return cell_of(position) * FACE_DEPTH + face_column(position);
}

fn upright_share() -> f32 {
    let t = clamp(
        (dab.altitude - FACE_LAID_ALTITUDE_RAD) / (FACE_UPRIGHT_ALTITUDE_RAD - FACE_LAID_ALTITUDE_RAD),
        0.0,
        1.0,
    );
    return t * t * (3.0 - 2.0 * t);
}

/// A share given per reference step, compounded over this dab's step.
fn over_step(rate: f32) -> f32 {
    let kept = 1.0 - clamp(rate, 0.0, 1.0);
    if (kept <= 0.0) {
        return 1.0;
    }
    return 1.0 - pow(kept, max(dab.step_ratio, 0.0));
}

/// Laid flat and pressed lightly, the face leaves more of what it carries; held
/// fully upright it lays nothing, only scraping up.
fn lay_share() -> f32 {
    let pressure = clamp(dab.pressure, 0.0, 1.0);
    return over_step(settings.lay_rate * (1.0 - upright_share()) * (1.0 - 0.5 * pressure));
}

/// Upright and pressed hard, the edge scrapes more up onto the face.
fn pickup_share() -> f32 {
    let pressure = clamp(dab.pressure, 0.0, 1.0);
    return over_step(settings.pickup_rate * (0.5 + 0.5 * upright_share()) * (0.5 + 0.5 * pressure));
}

fn cell_area_px() -> f32 {
    return 2.0 * semi_axes().x / f32(face_columns());
}

/// The face cells over the blade's rows, as (cell, whether it is on the blade).
fn touched_cell(id: vec2<u32>) -> vec2<u32> {
    let reach = i32(ceil(dab.radius)) + 1;
    let first = max(TIP_HALF - reach, 0);
    let last = min(TIP_HALF + reach, i32(TIP_SIZE) - 1);
    let row = first + i32(id.y);
    return vec2<u32>(u32(row) * FACE_DEPTH + id.x, u32(row <= last && id.x < FACE_DEPTH));
}

@compute @workgroup_size(FACE_THREADS)
fn rest(@builtin(local_invocation_index) thread: u32) {
    var peak = NO_PEAK;
    let count = dab.canvas_size.x * dab.canvas_size.y;
    for (var index = thread; index < count; index += FACE_THREADS) {
        let pixel = region_pixel(index);
        if (all(abs(brush_space(pixel)) <= semi_axes())) {
            peak = max(peak, textureLoad(dry_height, pixel).x);
        }
    }
    reduced[thread] = peak;
    workgroupBarrier();
    for (var stride = FACE_THREADS / 2u; stride > 0u; stride /= 2u) {
        if (thread < stride) {
            reduced[thread] = max(reduced[thread], reduced[thread + stride]);
        }
        workgroupBarrier();
    }
    if (thread == 0u) {
        var found = reduced[0];
        if (found <= NO_PEAK) {
            found = centre_dry_height();
        }
        let settled = followed_rest(face.rest, found, dab.rest_follow, dab.first_rest);
        face.rest = settled;
        face.plane = settled + dab.blade_gap_um;
    }
}

@compute @workgroup_size(8, 8)
fn want(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let position = brush_space(dab.canvas_origin + vec2<i32>(id.xy));
    let cover_q = quantize(lateral_cover(position));
    if (cover_q > 0) {
        atomicAdd(&face_delta[face_cell(position) * BLADE_DELTA_CHANNELS + WANT_CHANNEL], cover_q);
    }
}

@compute @workgroup_size(8, 8)
fn grant(@builtin(global_invocation_id) id: vec3<u32>) {
    let touched = touched_cell(id.xy);
    if (touched.y == 0u) {
        return;
    }
    let cell = touched.x;
    let weight = dequantize(atomicExchange(&face_delta[cell * BLADE_DELTA_CHANNELS + WANT_CHANNEL], 0));
    let base = cell * TIP_CHANNELS;
    let grant_base = cell * FACE_GRANT_CHANNELS;
    let held = face_load[base + VOLUME_CHANNEL];
    // The cell lays its share among the pixels under it by cover. A cell over less
    // than one whole pixel lays only that part of its share: dividing by the
    // smaller cover would pour the whole share into a pixel the cell barely meets.
    var lay = 0.0;
    if (weight > 0.0 && held > EMPTY_VOLUME_UM) {
        lay = lay_share() * held / max(weight, 1.0) * GRANT_SAFETY;
    }
    let capacity = FACE_CAPACITY_UM * cell_area_px();
    face_grant[grant_base + GRANT_LAY] = lay;
    face_grant[grant_base + GRANT_ROOM] = clamp(1.0 - held / max(capacity, EMPTY_VOLUME_UM), 0.0, 1.0);
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        face_grant[grant_base + GRANT_SHARES + slot] = held_share(face_load[base + slot], held);
    }
}

@compute @workgroup_size(8, 8)
fn exchange(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let pixel = dab.canvas_origin + vec2<i32>(id.xy);
    let position = brush_space(pixel);
    let cover = dequantize(quantize(lateral_cover(position)));
    if (cover <= 0.0) {
        return;
    }
    let cell = face_cell(position);
    let grant_base = cell * FACE_GRANT_CHANNELS;
    let delta_base = cell * BLADE_DELTA_CHANNELS;
    let volume = textureLoad(wet_vol, pixel).x;
    let surface = textureLoad(dry_height, pixel).x + volume;
    let contact = cover * edge_contact(position.y);
    let deposit_q = max(quantize_down(contact * face_grant[grant_base + GRANT_LAY]), 0);
    let above = min(max(surface - face.plane, 0.0), max(volume - dab.residual_um, 0.0));
    let heap = min(above / FULL_SCRAPE_UM, 1.0);
    let scraped = pickup_share() * contact * face_grant[grant_base + GRANT_ROOM] * above * heap;
    let pickup_q = clamp(quantize(scraped), 0, max(quantize_down(volume), 0));
    if (deposit_q == 0 && pickup_q == 0) {
        return;
    }
    var shares = array<f32, PIGMENT_CHANNELS>();
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        shares[slot] = face_grant[grant_base + GRANT_SHARES + slot];
    }
    let gained = trade(pixel, volume, deposit_q, pickup_q, shares);
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        if (gained[channel] != 0) {
            atomicAdd(&face_delta[delta_base + channel], gained[channel]);
        }
    }
}

@compute @workgroup_size(8, 8)
fn settle(@builtin(global_invocation_id) id: vec3<u32>) {
    let touched = touched_cell(id.xy);
    if (touched.y == 0u) {
        return;
    }
    let base = touched.x * TIP_CHANNELS;
    let delta_base = touched.x * BLADE_DELTA_CHANNELS;
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        let change = atomicExchange(&face_delta[delta_base + channel], 0);
        if (change != 0) {
            face_load[base + channel] += dequantize(change);
        }
    }
}

fn even_out(first: u32, second: u32) {
    let share = 0.5 * over_step(settings.smear);
    if (share <= 0.0) {
        return;
    }
    let a = first * TIP_CHANNELS;
    let b = second * TIP_CHANNELS;
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        let moved = share * (face_load[b + channel] - face_load[a + channel]);
        face_load[a + channel] += moved;
        face_load[b + channel] -= moved;
    }
}

/// Works a share of the paint on the face between neighbouring columns, pairing
/// them from `parity`; with the other parity's pass, every neighbour pair is worked
/// without two threads touching one cell at once.
fn smear_across(id: vec2<u32>, parity: u32) {
    let column = parity + 2u * id.x;
    let touched = touched_cell(vec2<u32>(column, id.y));
    if (touched.y == 1u && column + 1u < face_columns()) {
        even_out(touched.x, touched.x + 1u);
    }
}

/// As `smear_across`, between neighbouring rows: this is what fades the face's
/// streaks pass after pass.
fn smear_along(id: vec2<u32>, parity: u32) {
    let row = parity + 2u * id.y;
    let touched = touched_cell(vec2<u32>(id.x, row + 1u));
    if (touched.y == 1u) {
        even_out(touched.x - FACE_DEPTH, touched.x);
    }
}

@compute @workgroup_size(8, 8)
fn smear_across_even(@builtin(global_invocation_id) id: vec3<u32>) {
    smear_across(id.xy, 0u);
}

@compute @workgroup_size(8, 8)
fn smear_across_odd(@builtin(global_invocation_id) id: vec3<u32>) {
    smear_across(id.xy, 1u);
}

@compute @workgroup_size(8, 8)
fn smear_along_even(@builtin(global_invocation_id) id: vec3<u32>) {
    smear_along(id.xy, 0u);
}

@compute @workgroup_size(8, 8)
fn smear_along_odd(@builtin(global_invocation_id) id: vec3<u32>) {
    smear_along(id.xy, 1u);
}
