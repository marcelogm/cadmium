alias Floors = array<f32, BRISTLE_VERTICES>;

const PIXEL_CENTRE_PX: f32 = 0.5;

fn surface_um_at(texel: vec2<i32>) -> f32 {
    let last = vec2<i32>(textureDimensions(dry_height)) - vec2<i32>(1);
    let inside = clamp(texel, vec2<i32>(0), last);
    return load_bearing_um(textureLoad(dry_height, inside).x, textureLoad(wet_vol, inside).x);
}

fn surface_px(at: vec2<f32>) -> f32 {
    let last = vec2<f32>(textureDimensions(dry_height)) - vec2<f32>(1.0);
    let centred = clamp(at - vec2<f32>(PIXEL_CENTRE_PX), vec2<f32>(0.0), last);
    let corner = floor(centred);
    let share = centred - corner;
    let texel = vec2<i32>(corner);
    let top = mix(surface_um_at(texel), surface_um_at(texel + vec2<i32>(1, 0)), share.x);
    let bottom = mix(
        surface_um_at(texel + vec2<i32>(0, 1)),
        surface_um_at(texel + vec2<i32>(1, 1)),
        share.x,
    );
    return mix(top, bottom, share.y) / RELIEF_UM_PER_PX;
}

fn floors_under(chain: Chain) -> Floors {
    var floors: Floors;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        floors[vertex] = surface_px(chain[vertex].xy);
    }
    return floors;
}

const BASE_BISECTIONS: u32 = 16u;
const BISECTIONS_PER_ROUND: u32 = 2u;
const NO_FLOOR_PX: f32 = 3.0e38;
const FLAT_HAIR_LOAD: f32 = 1.0;

struct SupportFloors {
    heights_px: array<f32, BRISTLES_PER_THREAD>,
    owned: array<bool, BRISTLES_PER_THREAD>,
};

var<workgroup> partials: array<f32, BRISTLE_WORKGROUP_SIZE>;
var<workgroup> upper_partials: array<f32, BRISTLE_WORKGROUP_SIZE>;
var<workgroup> level_partials: array<vec4<f32>, BRISTLE_WORKGROUP_SIZE>;

struct LevelRound {
    middle_px: f32,
    lower_px: f32,
    upper_px: f32,
};

fn workgroup_sums(thread: u32, values: vec3<f32>) -> vec3<f32> {
    level_partials[thread] = vec4<f32>(values, 0.0);
    workgroupBarrier();
    for (var stride = BRISTLE_WORKGROUP_SIZE / 2u; stride > 0u; stride /= 2u) {
        if thread < stride {
            level_partials[thread] += level_partials[thread + stride];
        }
        workgroupBarrier();
    }
    let sums = workgroupUniformLoad(&level_partials[0]);
    workgroupBarrier();
    return sums.xyz;
}

fn round_levels(bounds: vec2<f32>) -> LevelRound {
    let middle_px = 0.5 * (bounds.x + bounds.y);
    return LevelRound(middle_px, 0.5 * (bounds.x + middle_px), 0.5 * (middle_px + bounds.y));
}

fn bisected(bounds: vec2<f32>, level_px: f32, too_deep: bool) -> vec2<f32> {
    return select(vec2<f32>(bounds.x, level_px), vec2<f32>(level_px, bounds.y), too_deep);
}

fn workgroup_range(thread: u32, range: vec2<f32>) -> vec2<f32> {
    partials[thread] = range.x;
    upper_partials[thread] = range.y;
    workgroupBarrier();
    for (var stride = BRISTLE_WORKGROUP_SIZE / 2u; stride > 0u; stride /= 2u) {
        if thread < stride {
            partials[thread] = min(partials[thread], partials[thread + stride]);
            upper_partials[thread] = max(upper_partials[thread], upper_partials[thread + stride]);
        }
        workgroupBarrier();
    }
    let low = workgroupUniformLoad(&partials[0]);
    let high = workgroupUniformLoad(&upper_partials[0]);
    workgroupBarrier();
    return vec2<f32>(low, high);
}

fn owned_support_floors(thread: u32, wetness: Wetness) -> SupportFloors {
    var floors: SupportFloors;
    for (var share = 0u; share < BRISTLES_PER_THREAD; share++) {
        let bristle = owned_bristle(thread, share);
        floors.owned[share] = bristle < dab.bristle_total;
        if floors.owned[share] {
            floors.heights_px[share] = support_floor_px(bristle, wetness[share]);
        }
    }
    return floors;
}

fn floor_range(floors: SupportFloors) -> vec2<f32> {
    var range = vec2<f32>(NO_FLOOR_PX, -NO_FLOOR_PX);
    for (var share = 0u; share < BRISTLES_PER_THREAD; share++) {
        if floors.owned[share] {
            let height = floors.heights_px[share];
            range = vec2<f32>(min(range.x, height), max(range.y, height));
        }
    }
    return range;
}

fn penetration_px(floors: SupportFloors, level_px: f32, press_px: f32) -> f32 {
    var sum = 0.0;
    for (var share = 0u; share < BRISTLES_PER_THREAD; share++) {
        if floors.owned[share] {
            sum += max(floors.heights_px[share] - level_px + press_px, 0.0);
        }
    }
    return sum;
}

fn resting_base_px(thread: u32, wetness: Wetness) -> f32 {
    let floors = owned_support_floors(thread, wetness);
    var bounds = workgroup_range(thread, floor_range(floors));
    if bounds.x > bounds.y {
        return 0.0;
    }
    let press_px = handle_press_px();
    let flat_penetration_px = f32(dab.bristle_total) * press_px;
    for (var round = 0u; round < BASE_BISECTIONS / BISECTIONS_PER_ROUND; round++) {
        let levels = round_levels(bounds);
        let pressed_px = workgroup_sums(thread, vec3<f32>(
            penetration_px(floors, levels.middle_px, press_px),
            penetration_px(floors, levels.lower_px, press_px),
            penetration_px(floors, levels.upper_px, press_px),
        ));
        let deep_middle = pressed_px.x > flat_penetration_px;
        bounds = bisected(bounds, levels.middle_px, deep_middle);
        let next_px = select(levels.lower_px, levels.upper_px, deep_middle);
        let next_pressed_px = select(pressed_px.y, pressed_px.z, deep_middle);
        bounds = bisected(bounds, next_px, next_pressed_px > flat_penetration_px);
    }
    return 0.5 * (bounds.x + bounds.y);
}

fn handle_press_px() -> f32 {
    return max(dab.handle_rest_height_px - dab.handle_height_px, 0.0);
}

fn hair_load(support_px: f32, base_px: f32) -> f32 {
    let press_px = handle_press_px();
    if press_px <= 0.0 {
        return FLAT_HAIR_LOAD;
    }
    return max(FLAT_HAIR_LOAD + (support_px - base_px) / press_px, 0.0);
}
