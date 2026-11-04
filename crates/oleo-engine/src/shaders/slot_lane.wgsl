fn slot_lane(low: vec4<f32>, high: vec4<f32>, index: u32) -> f32 {
    if (index < 4u) {
        return low[index];
    }
    return high[index - 4u];
}
