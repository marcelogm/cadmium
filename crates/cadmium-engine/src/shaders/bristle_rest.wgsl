struct BristleRest {
    root_px: vec2<f32>,
    splay: vec2<f32>,
    length_px: f32,
    stiffness: f32,
    radius_px: f32,
    cluster: u32,
    lane_start: vec2<f32>,
    lane_end: vec2<f32>,
};

fn texel_slot(texel: vec2<u32>) -> u32 {
    return texel.y * TIP_SIZE + texel.x;
}
