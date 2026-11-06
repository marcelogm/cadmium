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
