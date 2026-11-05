const EDGE_SOFTNESS_PX: f32 = 1.5;
const ALONG_HEADING_SOFTNESS_SHARE: f32 = 0.2;
const BLADE_COVERAGE_HALF_PX: f32 = 0.5;

fn brush_space(pixel: vec2<i32>) -> vec2<f32> {
    let offset = vec2<f32>(pixel) + vec2<f32>(0.5) - dab.center;
    let axis = dab.orientation;
    return vec2<f32>(dot(offset, axis), dot(offset, vec2<f32>(-axis.y, axis.x)));
}

fn semi_axes() -> vec2<f32> {
    return vec2<f32>(dab.radius * dab.elongation, dab.radius);
}

fn blade_footprint(position: vec2<f32>) -> f32 {
    let inset = semi_axes() - abs(position);
    let covered = clamp(inset + vec2<f32>(BLADE_COVERAGE_HALF_PX), vec2<f32>(0.0), vec2<f32>(1.0));
    return covered.x * covered.y;
}

fn canvas_offset(position: vec2<f32>) -> vec2<f32> {
    let axis = dab.orientation;
    return position.x * axis + position.y * vec2<f32>(-axis.y, axis.x);
}

fn heading() -> vec2<f32> {
    return vec2<f32>(cos(dab.direction), sin(dab.direction));
}

fn along_heading_share(position: vec2<f32>) -> f32 {
    let offset = canvas_offset(position);
    let reach = length(offset);
    if (reach <= 0.0) {
        return 0.0;
    }
    let along = dot(offset, heading()) / reach;
    return along * along;
}

fn tuft_footprint(position: vec2<f32>) -> f32 {
    let normalized_distance = length(position / semi_axes());
    let thinnest = min(semi_axes().x, semi_axes().y);
    let softness = mix(EDGE_SOFTNESS_PX, max(EDGE_SOFTNESS_PX, ALONG_HEADING_SOFTNESS_SHARE * thinnest), along_heading_share(position));
    return smoothstep(0.0, softness, (1.0 - normalized_distance) * thinnest);
}

fn footprint(position: vec2<f32>) -> f32 {
    if (dab.tool_mode == TOOL_MODE_KNIFE) {
        return blade_footprint(position);
    }
    return tuft_footprint(position);
}

fn blade_pressure_gradient(position: vec2<f32>) -> vec2<f32> {
    let normalized = position / semi_axes();
    var gradient = vec2<f32>(0.0);
    if (all(abs(normalized) < vec2<f32>(1.0))) {
        let slope = -2.0 * dab.pressure * normalized;
        gradient = slope * (vec2<f32>(1.0) - normalized * normalized).yx;
    }
    return canvas_offset(gradient);
}

fn inside_outline(offset: vec2<f32>) -> bool {
    if (dab.tool_mode == TOOL_MODE_KNIFE) {
        return all(abs(offset) <= semi_axes());
    }
    return length(offset / semi_axes()) <= 1.0;
}

fn inside_spread_region(pixel: vec2<i32>) -> bool {
    let local = pixel - dab.spread_origin;
    return all(local >= vec2<i32>(0)) && all(local < vec2<i32>(dab.spread_size));
}
