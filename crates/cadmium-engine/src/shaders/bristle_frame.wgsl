/// A pose as the handle buffer stores it.
struct StoredPose {
    ferrule: vec3<f32>,
    spare_0: f32,
    axis: vec3<f32>,
    spare_1: f32,
    orientation: vec2<f32>,
    spare_2: vec2<f32>,
};

/// The current dab's ferrule path: where it starts, and where it ends, which is where the
/// next dab starts.
struct HandleState {
    start: StoredPose,
    end: StoredPose,
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

fn pose_of(stored: StoredPose) -> Pose {
    return Pose(stored.ferrule, stored.axis, stored.orientation);
}

fn stored_pose(pose: Pose) -> StoredPose {
    return StoredPose(pose.ferrule, 0.0, pose.axis, 0.0, pose.orientation, vec2<f32>(0.0));
}

fn state_slot(bristle: u32) -> u32 {
    return bristle * BRISTLE_STATE_VECTORS;
}

/// A vertex's position in `xyz` and the floor under it in `w`.
fn position_slot(bristle: u32, vertex: u32) -> u32 {
    return state_slot(bristle) + vertex;
}

/// A vertex's friction anchor in `xyz`, and in `w` the normal force holding it: 0 free,
/// > 0 stuck, < 0 sliding.
fn anchor_slot(bristle: u32, vertex: u32) -> u32 {
    return state_slot(bristle) + BRISTLE_VERTICES + vertex;
}

/// A hinge's plastic rest shift in `xyz`: a rotation vector in the ferrule frame.
fn plastic_slot(bristle: u32, hinge: u32) -> u32 {
    return state_slot(bristle) + 2u * BRISTLE_VERTICES + hinge;
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

fn to_ferrule(world: vec3<f32>, pose: Pose) -> vec3<f32> {
    let local = untilted(world - pose.ferrule, pose.axis);
    return vec3<f32>(unspun(local.xy, pose.orientation), local.z);
}

fn from_ferrule(local: vec3<f32>, pose: Pose) -> vec3<f32> {
    return pose.ferrule + direction_at(local, pose);
}

fn direction_at(local: vec3<f32>, pose: Pose) -> vec3<f32> {
    return tilted(vec3<f32>(spun(local.xy, pose.orientation), local.z), pose.axis);
}

/// `world` moved rigidly with the ferrule from pose `start` to pose `end`.
fn carried(world: vec3<f32>, start: Pose, end: Pose) -> vec3<f32> {
    return from_ferrule(to_ferrule(world, start), end);
}

fn direction_in_ferrule(world: vec3<f32>, pose: Pose) -> vec3<f32> {
    let local = untilted(world, pose.axis);
    return vec3<f32>(unspun(local.xy, pose.orientation), local.z);
}
