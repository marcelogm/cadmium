const SHADING_REACH_PX: i32 = 1;
const BLOCK_TILES_PER_SIDE: u32 = 2u;
const BLOCK_TILES: u32 = BLOCK_TILES_PER_SIDE * BLOCK_TILES_PER_SIDE;

var<workgroup> block_changes: array<atomic<u32>, BLOCK_TILES>;

fn tile_of(pixel: vec2<i32>) -> vec2<i32> {
    return vec2<i32>(floor(vec2<f32>(pixel) / f32(TILE_SIZE)));
}

fn tile_grid() -> vec2<i32> {
    return vec2<i32>(i32(atomicLoad(&tile_changes[0])), i32(atomicLoad(&tile_changes[1])));
}

fn block_first_tile(block_origin: vec2<i32>) -> vec2<i32> {
    return max(tile_of(block_origin - vec2<i32>(SHADING_REACH_PX)), vec2<i32>(0));
}

fn note_change(pixel: vec2<i32>, block_origin: vec2<i32>) {
    let first = block_first_tile(block_origin);
    let low = max(tile_of(pixel - vec2<i32>(SHADING_REACH_PX)), vec2<i32>(0));
    let high = tile_of(pixel + vec2<i32>(SHADING_REACH_PX));
    for (var y = low.y; y <= high.y; y++) {
        for (var x = low.x; x <= high.x; x++) {
            let local = vec2<u32>(vec2<i32>(x, y) - first);
            atomicOr(&block_changes[local.y * BLOCK_TILES_PER_SIDE + local.x], 1u);
        }
    }
}

fn publish_changes(thread: u32, block_origin: vec2<i32>) {
    if (thread >= BLOCK_TILES || atomicLoad(&block_changes[thread]) == 0u) {
        return;
    }
    let offset = vec2<u32>(thread % BLOCK_TILES_PER_SIDE, thread / BLOCK_TILES_PER_SIDE);
    let tile = block_first_tile(block_origin) + vec2<i32>(offset);
    let grid = tile_grid();
    if (any(tile >= grid)) {
        return;
    }
    atomicStore(&tile_changes[TILE_CHANGE_HEADER_WORDS + u32(tile.y * grid.x + tile.x)], 1u);
}
