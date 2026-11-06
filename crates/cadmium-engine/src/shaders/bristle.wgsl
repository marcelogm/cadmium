fn segment_length(rest: BristleRest) -> f32 {
    return rest.length_px / f32(BRISTLE_SEGMENTS);
}

fn root_position(rest: BristleRest, pose: Pose) -> vec3<f32> {
    let offset = spun(rest.root_px, pose.orientation);
    return pose.ferrule + tilted(vec3<f32>(offset, 0.0), pose.axis);
}

fn base_lean(rest: BristleRest) -> vec2<f32> {
    let flare = rest.root_px / max(dab.ferrule_radius_px, 1.0) * BRISTLE_FLARE_SHARE;
    return rest.splay * FAN_REST_SPLAY_SHARE + flare;
}

fn hanging_along(lean: vec2<f32>, pose: Pose) -> vec3<f32> {
    let spun_lean = spun(lean, pose.orientation);
    return tilted(normalize(vec3<f32>(spun_lean, -1.0)), pose.axis);
}

fn turned(departure: vec2<f32>, arrival: vec2<f32>, share: f32) -> vec2<f32> {
    let start = atan2(departure.y, departure.x);
    let sweep = atan2(departure.x * arrival.y - departure.y * arrival.x, dot(departure, arrival));
    let angle = start + sweep * share;
    return vec2<f32>(cos(angle), sin(angle));
}

fn pose_between(start: Pose, end: Pose, share: f32) -> Pose {
    return Pose(
        mix(start.ferrule, end.ferrule, share),
        normalize(mix(start.axis, end.axis, share)),
        turned(start.orientation, end.orientation, share),
    );
}
