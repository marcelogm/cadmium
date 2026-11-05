fn tuft_press() -> f32 {
    let lifted_px = max(dab.handle_rest_height_px, MIN_SEGMENT_SPAN_PX);
    return clamp(1.0 - dab.handle_height_px / lifted_px, 0.0, 1.0);
}

fn bridging(wetness: f32) -> f32 {
    return clamp(wetness / tuning.point_film, 0.0, 1.0);
}

fn gathering(wetness: f32) -> f32 {
    let held = clamp(1.0 - tuft_press() / tuning.point_release, 0.0, 1.0);
    return tuning.pointing * bridging(wetness) * held;
}

fn resting_lean(rest: BristleRest, gathered: f32) -> vec2<f32> {
    let axes = vec2<f32>(tuning.point_axis_across, tuning.point_axis_along);
    let toward_axis = -rest.root_px * axes / max(rest.length_px, MIN_SEGMENT_SPAN_PX);
    return base_lean(rest) + toward_axis * gathered;
}
