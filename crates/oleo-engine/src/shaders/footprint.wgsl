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

fn bristle_footprint(position: vec2<f32>) -> f32 {
    let normalized_distance = length(position / semi_axes());
    let edge = smoothstep(0.0, EDGE_SOFTNESS_PX, (1.0 - normalized_distance) * dab.radius);
    let phase = TAU * dab.bristle_count * position.y / (2.0 * dab.radius);
    let bristles = 1.0 - dab.bristle_contrast * 0.5 * (1.0 + sin(phase));
    return edge * bristles;
}

fn footprint(position: vec2<f32>) -> f32 {
    if (dab.tool_mode == TOOL_MODE_KNIFE) {
        return blade_footprint(position);
    }
    return bristle_footprint(position);
}

fn inside_outline(offset: vec2<f32>) -> bool {
    if (dab.tool_mode == TOOL_MODE_KNIFE) {
        return all(abs(offset) <= semi_axes());
    }
    return length(offset / semi_axes()) <= 1.0;
}
