fn bristle_vertex(bristle: u32, vertex: u32) -> vec4<f32> {
    return states[position_slot(bristle, vertex)];
}

fn handle_pose() -> Pose {
    return Pose(handle.ferrule, handle.axis, handle.orientation);
}
