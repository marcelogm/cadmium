@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var<storage, read> contact_mask: array<u32>;
@group(0) @binding(2) var<storage, read_write> tile_changes: array<atomic<u32>>;

fn contact_mask_word(index: u32) -> u32 {
    return contact_mask[index];
}

@compute @workgroup_size(BLOCK_SIDE_PX, BLOCK_SIDE_PX)
fn main(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) thread: u32,
) {
    let block_origin = dab.canvas_origin + vec2<i32>(group.xy * BLOCK_SIDE_PX);
    let pixel = dab.canvas_origin + vec2<i32>(id.xy);
    let inside = id.x < dab.canvas_size.x && id.y < dab.canvas_size.y;
    if (inside && contact_at(pixel).coverage > 0.0) {
        note_change(pixel, block_origin);
    }
    workgroupBarrier();
    publish_changes(thread, block_origin);
}
