const TAU: f32 = 6.283185307;
const EDGE_SOFTNESS_PX: f32 = 1.5;

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
    let edges = smoothstep(vec2<f32>(0.0), vec2<f32>(EDGE_SOFTNESS_PX), inset);
    return edges.x * edges.y;
}

fn tuft_footprint(position: vec2<f32>) -> f32 {
    let normalized_distance = length(position / semi_axes());
    return smoothstep(0.0, EDGE_SOFTNESS_PX, (1.0 - normalized_distance) * dab.radius);
}

fn bristles(position: vec2<f32>) -> f32 {
    let phase = TAU * dab.bristle_count * position.y / (2.0 * dab.radius);
    return 1.0 - dab.bristle_contrast * 0.5 * (1.0 + sin(phase));
}

fn outline_footprint(position: vec2<f32>) -> f32 {
    if (dab.tool_mode == TOOL_MODE_KNIFE) {
        return blade_footprint(position);
    }
    return tuft_footprint(position);
}

fn footprint(position: vec2<f32>) -> f32 {
    if (dab.tool_mode == TOOL_MODE_KNIFE) {
        return blade_footprint(position);
    }
    return tuft_footprint(position) * bristles(position);
}

fn pressure_envelope_gradient(position: vec2<f32>) -> vec2<f32> {
    let semi = semi_axes();
    let normalized = position / semi;
    let slope = -2.0 * dab.pressure * dab.radius * normalized / semi;
    var gradient = vec2<f32>(0.0);
    if (dab.tool_mode == TOOL_MODE_KNIFE) {
        if (all(abs(normalized) < vec2<f32>(1.0))) {
            gradient = slope * (vec2<f32>(1.0) - normalized * normalized).yx;
        }
    } else if (dot(normalized, normalized) < 1.0) {
        gradient = slope;
    }
    let axis = dab.orientation;
    return gradient.x * axis + gradient.y * vec2<f32>(-axis.y, axis.x);
}

fn inside_outline(offset: vec2<f32>) -> bool {
    if (dab.tool_mode == TOOL_MODE_KNIFE) {
        return all(abs(offset) <= semi_axes());
    }
    return length(offset / semi_axes()) <= 1.0;
}
