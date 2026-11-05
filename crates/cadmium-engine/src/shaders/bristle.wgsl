struct HandleState {
    ferrule: vec3<f32>,
    substep_seconds: f32,
    axis: vec3<f32>,
    _pad0: f32,
    velocity: vec3<f32>,
    _pad1: f32,
    orientation: vec2<f32>,
    _pad2: vec2<f32>,
};

struct Frame {
    across: vec3<f32>,
    along: vec3<f32>,
    axis: vec3<f32>,
};

struct Pose {
    ferrule: vec3<f32>,
    axis: vec3<f32>,
    orientation: vec2<f32>,
};

fn state_slot(bristle: u32) -> u32 {
    return bristle * BRISTLE_STATE_VECTORS;
}

fn position_slot(bristle: u32, vertex: u32) -> u32 {
    return state_slot(bristle) + vertex;
}

fn previous_slot(bristle: u32, vertex: u32) -> u32 {
    return state_slot(bristle) + BRISTLE_VERTICES + vertex;
}

fn anchor_slot(bristle: u32, vertex: u32) -> u32 {
    return state_slot(bristle) + 2u * BRISTLE_VERTICES + vertex;
}

fn bend_slot(bristle: u32) -> u32 {
    return state_slot(bristle) + 3u * BRISTLE_VERTICES;
}

fn segment_length(rest: BristleRest) -> f32 {
    return rest.length_px / f32(BRISTLE_SEGMENTS);
}

fn frame_of(axis: vec3<f32>) -> Frame {
    let lean = 1.0 / (1.0 + axis.z);
    let across = vec3<f32>(1.0 - axis.x * axis.x * lean, -axis.x * axis.y * lean, -axis.x);
    let along = vec3<f32>(-axis.x * axis.y * lean, 1.0 - axis.y * axis.y * lean, -axis.y);
    return Frame(across, along, axis);
}

fn tilted(local: vec3<f32>, axis: vec3<f32>) -> vec3<f32> {
    let frame = frame_of(axis);
    return frame.across * local.x + frame.along * local.y + frame.axis * local.z;
}

fn untilted(world: vec3<f32>, axis: vec3<f32>) -> vec3<f32> {
    let frame = frame_of(axis);
    return vec3<f32>(dot(world, frame.across), dot(world, frame.along), dot(world, frame.axis));
}

fn spun(local: vec2<f32>, orientation: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        -local.x * orientation.y - local.y * orientation.x,
        local.x * orientation.x - local.y * orientation.y,
    );
}

fn unspun(world: vec2<f32>, orientation: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        world.y * orientation.x - world.x * orientation.y,
        -world.x * orientation.x - world.y * orientation.y,
    );
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
