struct Site {
    pixel: vec2<i32>,
    volume: f32,
    dry: f32,
    fractions: array<f32, 8>,
};

struct Ledger {
    pickup_left_q: i32,
    deposit_total_q: i32,
    pickup_total_q: i32,
    deposit_slots_q: array<i32, 8>,
    pickup_slots_q: array<i32, 8>,
};

const MAX_DEPOSIT_SHARE: f32 = 0.45;
const PICKUP_SHARE_PER_PIXEL: f32 = 0.45;
const SLOW_TRANSFER_SHARE: f32 = 0.3;
const MIN_VELOCITY_CUTOFF: f32 = 1e-6;
const EMPTY_VOLUME_UM: f32 = 1e-4;
const QUANTIZE_LIMIT: f32 = 2.0e9;
const MIN_KEPT_SHARE: f32 = 1e-6;
const ROUNDING_MARGIN_SHARE: f32 = 1e-6;
const KEPT_BELOW_ROUNDING: f32 = 1.0 - ROUNDING_MARGIN_SHARE;
const REFERENCE_THREADS: u32 = 256u;

var<workgroup> weighted_heights: array<f32, REFERENCE_THREADS>;
var<workgroup> weights: array<f32, REFERENCE_THREADS>;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var wet_pig0: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(2) var wet_pig1: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, read_write>;
@group(0) @binding(4) var dry_height: texture_storage_2d<r32float, read>;
@group(0) @binding(5) var<storage, read> tip: array<f32>;
@group(0) @binding(6) var<storage, read_write> tip_delta: array<atomic<i32>>;
@group(0) @binding(7) var<storage, read_write> contact_reference: ContactReference;
@group(0) @binding(8) var<storage, read> contact_mask: array<u32>;
@group(0) @binding(9) var<storage, read> texel_load: array<u32>;
@group(0) @binding(10) var<storage, read> cell_grids: array<u32>;
@group(0) @binding(11) var<storage, read> hair_loads: array<f32>;
@group(0) @binding(12) var<storage, read> contact_extent: array<i32, CONTACT_EXTENT_WORDS>;

fn contact_mask_word(index: u32) -> u32 {
    return contact_mask[index];
}

fn quantize(amount: f32) -> i32 {
    return i32(round(clamp(amount * DELTA_SCALE, -QUANTIZE_LIMIT, QUANTIZE_LIMIT)));
}

fn quantize_down(amount: f32) -> i32 {
    return i32(floor(clamp(amount * DELTA_SCALE, -QUANTIZE_LIMIT, QUANTIZE_LIMIT)));
}

fn dequantize(amount: i32) -> f32 {
    return f32(amount) / DELTA_SCALE;
}

fn surface_height(pixel: vec2<i32>) -> f32 {
    return textureLoad(dry_height, pixel).x + textureLoad(wet_vol, pixel).x;
}

fn contact(height: f32, hanging_film: f32, bristle: u32) -> f32 {
    return contact_reach(height, hanging_film) * min(hair_loads[bristle], dab.hair_load_limit);
}

fn velocity_share() -> f32 {
    let ramp = smoothstep(0.0, max(dab.velocity_cutoff, MIN_VELOCITY_CUTOFF), dab.speed);
    return mix(SLOW_TRANSFER_SHARE, 1.0, ramp);
}

struct Transfer {
    deposit_q: i32,
    pickup: f32,
};

struct FilmSharing {
    deposit_pixels: f32,
    pickups: f32,
};

fn deposit_budget_q(film_um: f32) -> f32 {
    return f32(max(quantize_down(film_um), 0));
}

fn bristle_transfer(site: Site, hanging_um: f32, sharing: FilmSharing, touch: Contact) -> Transfer {
    let coverage = touch.coverage;
    let height = site.dry + site.volume;
    let pressed = contact(height, hanging_um, touch.bristle);
    let reference_rate = dab.transfer_fraction * pressed * velocity_share();
    let rate = 1.0 - pow(max(1.0 - reference_rate, MIN_KEPT_SHARE), dab.step_ratio);
    let capped_share = min(rate, MAX_DEPOSIT_SHARE) / MAX_DEPOSIT_SHARE;
    let budget_share = capped_share * coverage / sharing.deposit_pixels * KEPT_BELOW_ROUNDING;
    let deposit_q = i32(floor(deposit_budget_q(hanging_um) * budget_share));
    let pickup = rate * max(site.volume - dab.residual_um, 0.0) / sharing.pickups * coverage;
    return Transfer(deposit_q, pickup);
}

fn read_fractions(pixel: vec2<i32>) -> array<f32, 8> {
    let low = textureLoad(wet_pig0, pixel);
    let high = textureLoad(wet_pig1, pixel);
    return array<f32, 8>(low.x, low.y, low.z, low.w, high.x, high.y, high.z, high.w);
}

fn centre_height() -> f32 {
    let size = vec2<i32>(textureDimensions(wet_vol));
    let pixel = clamp(vec2<i32>(floor(dab.center)), vec2<i32>(0), size - vec2<i32>(1));
    return surface_height(pixel);
}

fn first_lane_index(low: u32, thread: u32) -> u32 {
    return low + (thread + REFERENCE_THREADS - low % REFERENCE_THREADS) % REFERENCE_THREADS;
}

@compute @workgroup_size(REFERENCE_THREADS)
fn reference(@builtin(local_invocation_index) thread: u32) {
    var weighted = 0.0;
    var weight = 0.0;
    let width = dab.canvas_size.x;
    let first = vec2<i32>(contact_extent[CONTACT_BOX_FIRST], contact_extent[CONTACT_BOX_FIRST + 1u]);
    let last = vec2<i32>(contact_extent[CONTACT_BOX_LAST], contact_extent[CONTACT_BOX_LAST + 1u]);
    for (var row = first.y; row <= last.y; row++) {
        let row_start = u32(row - dab.canvas_origin.y) * width;
        let low = row_start + u32(first.x - dab.canvas_origin.x);
        let high = row_start + u32(last.x - dab.canvas_origin.x);
        for (var index = first_lane_index(low, thread); index <= high; index += REFERENCE_THREADS) {
            let pixel = dab.canvas_origin + vec2<i32>(vec2<u32>(index % width, index / width));
            let cover = contact_at(pixel).coverage;
            weighted += cover * surface_height(pixel);
            weight += cover;
        }
    }
    weighted_heights[thread] = weighted;
    weights[thread] = weight;
    workgroupBarrier();
    for (var stride = REFERENCE_THREADS / 2u; stride > 0u; stride /= 2u) {
        if (thread < stride) {
            weighted_heights[thread] += weighted_heights[thread + stride];
            weights[thread] += weights[thread + stride];
        }
        workgroupBarrier();
    }
    if (thread == 0u) {
        var found = centre_height();
        if (weights[0] > 0.0) {
            found = weighted_heights[0] / weights[0];
        }
        // A full follow replaces the height outright: mix() at 1.0 can still
        // carry the previous stroke's height in its rounding.
        contact_reference.height = select(
            mix(contact_reference.height, found, dab.rest_follow),
            found,
            dab.rest_follow >= 1.0,
        );
    }
}

fn trade(ledger: ptr<function, Ledger>, site: Site, base: u32, transfer: Transfer) {
    let tip_volume = tip[base + VOLUME_CHANNEL];
    let deposit_q = transfer.deposit_q;
    let deposit = dequantize(deposit_q);
    let pickup_q = clamp(quantize(transfer.pickup), 0, (*ledger).pickup_left_q);
    if (deposit_q == 0 && pickup_q == 0) {
        return;
    }
    (*ledger).pickup_left_q -= pickup_q;
    let pickup_amount = dequantize(pickup_q);
    for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
        var tip_share = 0.0;
        if (tip_volume > EMPTY_VOLUME_UM) {
            tip_share = max(tip[base + slot], 0.0) / tip_volume * KEPT_BELOW_ROUNDING;
        }
        let deposit_slot_q = max(quantize_down(deposit * tip_share), 0);
        let pickup_slot_q = max(quantize(pickup_amount * site.fractions[slot]), 0);
        (*ledger).deposit_slots_q[slot] += deposit_slot_q;
        (*ledger).pickup_slots_q[slot] += pickup_slot_q;
        let tip_change = pickup_slot_q - deposit_slot_q;
        if (tip_change != 0) {
            atomicAdd(&tip_delta[base + slot], tip_change);
        }
    }
    atomicAdd(&tip_delta[base + VOLUME_CHANNEL], pickup_q - deposit_q);
    (*ledger).deposit_total_q += deposit_q;
    (*ledger).pickup_total_q += pickup_q;
}

fn film_sharing(slot: u32) -> FilmSharing {
    let pixels = f32(texel_load[slot]) / f32(CONTACT_LEVELS);
    return FilmSharing(max(pixels, 1.0 / MAX_DEPOSIT_SHARE), max(PICKUP_SHARE_PER_PIXEL * pixels, 1.0));
}

fn trade_through_bristle(ledger: ptr<function, Ledger>, site: Site) {
    let contact = contact_at(site.pixel);
    if (contact.coverage <= 0.0) {
        return;
    }
    let slot = cell_slot(contact);
    let base = slot * TIP_CHANNELS;
    let film_um = tip[base + VOLUME_CHANNEL];
    let transfer = bristle_transfer(site, film_um, film_sharing(slot), contact);
    trade(ledger, site, base, transfer);
}

fn store_traded(site: Site, ledger: Ledger) {
    let new_volume = site.volume - dequantize(ledger.pickup_total_q) + dequantize(ledger.deposit_total_q);
    var new_fractions = array<f32, 8>();
    if (new_volume >= EMPTY_VOLUME_UM) {
        for (var slot = 0u; slot < PIGMENT_CHANNELS; slot++) {
            let amount = site.fractions[slot] * site.volume
                + dequantize(ledger.deposit_slots_q[slot])
                - dequantize(ledger.pickup_slots_q[slot]);
            new_fractions[slot] = nearest_half(max(amount, 0.0) / new_volume);
        }
        new_fractions = within_whole_volume(new_fractions);
    }
    textureStore(wet_vol, site.pixel, vec4<f32>(new_volume, 0.0, 0.0, 0.0));
    textureStore(wet_pig0, site.pixel, vec4<f32>(new_fractions[0], new_fractions[1], new_fractions[2], new_fractions[3]));
    textureStore(wet_pig1, site.pixel, vec4<f32>(new_fractions[4], new_fractions[5], new_fractions[6], new_fractions[7]));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let pixel = dab.canvas_origin + vec2<i32>(id.xy);
    if (contact_at(pixel).coverage <= 0.0) {
        return;
    }
    let volume = textureLoad(wet_vol, pixel).x;
    let site = Site(pixel, volume, textureLoad(dry_height, pixel).x, read_fractions(pixel));
    var ledger = Ledger(max(quantize_down(volume), 0), 0, 0, array<i32, 8>(), array<i32, 8>());
    trade_through_bristle(&ledger, site);
    if (ledger.deposit_total_q == 0 && ledger.pickup_total_q == 0) {
        return;
    }
    store_traded(site, ledger);
}
