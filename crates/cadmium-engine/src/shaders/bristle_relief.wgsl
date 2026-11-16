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

/// Half the span of the central differences that measure the relief's slope, px.
const SLOPE_PROBE_PX: f32 = 1.0;

/// The relief's slope (∂f/∂x, ∂f/∂y) at `at`, by central differences.
fn surface_slope(at: vec2<f32>) -> vec2<f32> {
    let dx = vec2<f32>(SLOPE_PROBE_PX, 0.0);
    let dy = vec2<f32>(0.0, SLOPE_PROBE_PX);
    return vec2<f32>(
        surface_px(at + dx) - surface_px(at - dx),
        surface_px(at + dy) - surface_px(at - dy),
    ) / (2.0 * SLOPE_PROBE_PX);
}

fn floors_under(chain: Chain) -> Floors {
    var floors: Floors;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        floors.heights_px[vertex] = surface_px(chain[vertex].xy);
        floors.slopes[vertex] = surface_slope(chain[vertex].xy);
    }
    return floors;
}

/// Larger than any height: the identity of a max-reduction that starts at its negative.
const NO_FLOOR_PX: f32 = 3.0e38;

var<workgroup> partials: array<f32, BRISTLE_WORKGROUP_SIZE>;
var<workgroup> upper_partials: array<f32, BRISTLE_WORKGROUP_SIZE>;

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
