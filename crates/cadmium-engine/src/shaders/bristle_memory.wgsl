fn lean_of(direction: vec3<f32>, pose: Pose) -> vec2<f32> {
    let local = untilted(direction, pose.axis);
    return unspun(local.xy, pose.orientation) / max(-local.z, MIN_LEAN_DROP);
}

fn limited_bend(bend: vec2<f32>) -> vec2<f32> {
    let size = length(bend);
    return bend * min(1.0, MAX_PLASTIC_LEAN / max(size, MIN_SEGMENT_SPAN_PX));
}

fn remembered_bend(
    bend: vec2<f32>,
    lean: vec2<f32>,
    span: vec3<f32>,
    pose: Pose,
    seconds: f32,
) -> vec2<f32> {
    let resting = hanging_along(lean + bend, pose);
    let bent = heading_of(span, resting);
    let angle = acos(clamp(dot(bent, resting), -1.0, 1.0));
    if angle <= PLASTIC_BEND_THRESHOLD_RAD {
        return bend;
    }
    let held = lean_of(bent, pose) - lean;
    let creep = 1.0 - exp(-tuning.plasticity * max(seconds, 0.0));
    let kept = creep * (1.0 - PLASTIC_BEND_THRESHOLD_RAD / angle);
    return limited_bend(mix(bend, held, kept));
}
