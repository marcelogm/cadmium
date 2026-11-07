struct SpreadShape {
    outline: f32,
    drag: f32,
    squeeze_gradient: vec2<f32>,
};

const DRAG_SPEED_GAIN: f32 = 4.0;
const MAX_COURANT: f32 = 0.9;
const SLUMP_RIM_SHARE: f32 = 0.5;
const DRAG_HANGING_FILM_UM: f32 = 0.0;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var wet_pig0: texture_storage_2d<rgba16float, read>;
@group(0) @binding(2) var wet_pig1: texture_storage_2d<rgba16float, read>;
@group(0) @binding(3) var wet_vol: texture_storage_2d<r32float, read>;
@group(0) @binding(4) var scratch_pig0: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var scratch_pig1: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var scratch_vol: texture_storage_2d<r32float, write>;
@group(0) @binding(7) var scratch_motion: texture_storage_2d<rgba32float, write>;
@group(0) @binding(8) var<storage, read> contact_mask: array<u32>;
@group(0) @binding(9) var<storage, read> contact_extent: array<i32, CONTACT_EXTENT_WORDS>;
@group(0) @binding(10) var dry_height: texture_storage_2d<r32float, read>;
@group(0) @binding(11) var<storage, read> contact_reference: ContactReference;

fn contact_mask_word(index: u32) -> u32 {
    return contact_mask[index];
}

fn spread_shape(pixel: vec2<i32>) -> SpreadShape {
    if (dab.bristle_contact != 0u) {
        let coverage = contact_at(pixel).coverage;
        return SpreadShape(coverage, coverage, contact_press_gradient(pixel));
    }
    let position = brush_space(pixel);
    let blade = blade_footprint(position);
    return SpreadShape(blade, blade, blade_pressure_gradient(position));
}

fn kept_at_canvas_border(pixel: vec2<i32>, velocity: vec2<f32>) -> vec2<f32> {
    let last = vec2<i32>(textureDimensions(wet_vol)) - vec2<i32>(1);
    var inward = velocity;
    if (pixel.x == 0) {
        inward.x = max(inward.x, 0.0);
    }
    if (pixel.x == last.x) {
        inward.x = min(inward.x, 0.0);
    }
    if (pixel.y == 0) {
        inward.y = max(inward.y, 0.0);
    }
    if (pixel.y == last.y) {
        inward.y = min(inward.y, 0.0);
    }
    return inward;
}

fn kept_inside_spread(pixel: vec2<i32>, velocity: vec2<f32>) -> vec2<f32> {
    if (!inside_spread_region(pixel)) {
        return vec2<f32>(0.0);
    }
    let first = dab.spread_origin;
    let last = first + vec2<i32>(dab.spread_size) - vec2<i32>(1);
    let toward_first = vec2<f32>(select(vec2<f32>(0.0), vec2<f32>(1.0), pixel > first));
    let toward_last = vec2<f32>(select(vec2<f32>(0.0), vec2<f32>(1.0), pixel < last));
    return max(velocity, vec2<f32>(0.0)) * toward_last + min(velocity, vec2<f32>(0.0)) * toward_first;
}

fn wet_volume_at(pixel: vec2<i32>) -> f32 {
    let last = vec2<i32>(textureDimensions(wet_vol)) - vec2<i32>(1);
    return max(textureLoad(wet_vol, clamp(pixel, vec2<i32>(0), last)).x, 0.0);
}

fn reach_at(pixel: vec2<i32>) -> f32 {
    let last = vec2<i32>(textureDimensions(dry_height)) - vec2<i32>(1);
    let dry = textureLoad(dry_height, clamp(pixel, vec2<i32>(0), last)).x;
    return contact_reach(dry + wet_volume_at(pixel), DRAG_HANGING_FILM_UM);
}

fn touching_hairs_only(pixel: vec2<i32>, velocity: vec2<f32>) -> vec2<f32> {
    if (dab.bristle_contact == 0u || all(velocity == vec2<f32>(0.0))) {
        return velocity;
    }
    let own = reach_at(pixel);
    let toward_x = reach_at(pixel + vec2<i32>(i32(sign(velocity.x)), 0));
    let toward_y = reach_at(pixel + vec2<i32>(0, i32(sign(velocity.y))));
    return velocity * vec2<f32>(min(own, toward_x), min(own, toward_y));
}

fn beside_or_behind_contact(pixel: vec2<i32>) -> bool {
    if (dab.bristle_contact == 0u) {
        return false;
    }
    let rim_px = SLUMP_RIM_SHARE * dab.ferrule_radius_px;
    let centre = vec2<f32>(pixel) + vec2<f32>(0.5);
    let across_extent = contact_extent_px();
    let across = across_stroke_px(centre);
    let beside = across >= across_extent.x - rim_px && across <= across_extent.y + rim_px;
    let not_yet_reached = along_stroke_px(centre) > contact_along_extent_px().y + rim_px;
    return beside && !not_yet_reached;
}

fn slump_velocity(pixel: vec2<i32>, volume: f32) -> vec2<f32> {
    if (dab.slump_rate <= 0.0 || volume <= dab.residual_um || !beside_or_behind_contact(pixel)) {
        return vec2<f32>(0.0);
    }
    let step_x = vec2<i32>(1, 0);
    let step_y = vec2<i32>(0, 1);
    let slope = 0.5 * vec2<f32>(
        wet_volume_at(pixel + step_x) - wet_volume_at(pixel - step_x),
        wet_volume_at(pixel + step_y) - wet_volume_at(pixel - step_y),
    );
    let steepness = length(slope);
    let excess_um = steepness - dab.slump_yield_um2_per_px / volume;
    if (excess_um <= 0.0) {
        return vec2<f32>(0.0);
    }
    let downhill = -dab.slump_rate * excess_um / (volume * steepness) * slope;
    return kept_inside_spread(pixel, downhill);
}

fn outflow(velocity: vec2<f32>) -> f32 {
    return abs(velocity.x) + abs(velocity.y);
}

fn motion(pixel: vec2<i32>, volume: f32) -> vec4<f32> {
    let shape = spread_shape(pixel);
    let outline = shape.outline;
    let squeeze_gradient = shape.squeeze_gradient;
    let slumping = slump_velocity(pixel, volume);
    let still = all(squeeze_gradient == vec2<f32>(0.0)) && all(slumping == vec2<f32>(0.0));
    if (outline <= 0.0 && still) {
        return vec4<f32>(0.0);
    }
    let ahead = heading();
    let across = vec2<f32>(-ahead.y, ahead.x);
    var squeeze_push = squeeze_gradient;
    if (!is_blade_mode(dab.tool_mode)) {
        squeeze_push = dot(squeeze_gradient, across) * across;
    }
    let drag = dab.advection * min(dab.speed * DRAG_SPEED_GAIN, 1.0);
    let hairs = touching_hairs_only(pixel, shape.drag * drag * ahead - dab.squeeze * squeeze_push);
    var velocity = kept_at_canvas_border(pixel, hairs) + slumping;
    let courant = outflow(velocity);
    if (courant > MAX_COURANT) {
        velocity *= MAX_COURANT / courant;
    }
    var movable_share = 0.0;
    if (volume > dab.residual_um) {
        movable_share = (volume - dab.residual_um) / volume;
    }
    let leaving = outflow(velocity);
    if (leaving > movable_share) {
        velocity *= movable_share / leaving;
    }
    let diffusion = min(dab.diffusion * outline, 1.0 - outflow(velocity));
    return vec4<f32>(velocity, diffusion, 0.0);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let local = vec2<i32>(id.xy);
    let pixel = dab.canvas_origin + local;
    let volume = textureLoad(wet_vol, pixel);
    textureStore(scratch_pig0, local, textureLoad(wet_pig0, pixel));
    textureStore(scratch_pig1, local, textureLoad(wet_pig1, pixel));
    textureStore(scratch_vol, local, volume);
    textureStore(scratch_motion, local, motion(pixel, volume.x));
}
