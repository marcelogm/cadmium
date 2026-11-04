use std::collections::HashSet;

use crate::canvas::{Canvas, TILE_SIZE, TileCoord};

pub(super) fn tile_extent(canvas: &Canvas, tile: TileCoord) -> Option<wgpu::Extent3d> {
    let width = canvas
        .spec
        .width
        .saturating_sub(tile.x * TILE_SIZE)
        .min(TILE_SIZE);
    let height = canvas
        .spec
        .height
        .saturating_sub(tile.y * TILE_SIZE)
        .min(TILE_SIZE);
    (width > 0 && height > 0).then_some(wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    })
}

fn tile_rect(canvas: &Canvas, tile: TileCoord) -> Option<[f32; 4]> {
    let extent = tile_extent(canvas, tile)?;
    let x0 = (tile.x * TILE_SIZE) as f32;
    let y0 = (tile.y * TILE_SIZE) as f32;
    Some([
        x0,
        y0,
        x0 + (extent.width - 1) as f32,
        y0 + (extent.height - 1) as f32,
    ])
}

pub(super) fn mark_restored(canvas: &mut Canvas, tile: TileCoord) {
    if let Some([x0, y0, x1, y1]) = tile_rect(canvas, tile) {
        canvas.dirty.mark_rect(x0, y0, x1, y1);
    }
}

pub(super) fn mark_wet(canvas: &mut Canvas, tile: TileCoord) {
    if let Some([x0, y0, x1, y1]) = tile_rect(canvas, tile) {
        canvas.wet_tiles.mark_rect(x0, y0, x1, y1);
    }
}

pub(super) fn forget_wet(canvas: &mut Canvas, dried: &HashSet<TileCoord>) {
    let still_wet: Vec<TileCoord> = canvas
        .wet_tiles
        .take()
        .into_iter()
        .filter(|tile| !dried.contains(tile))
        .collect();
    for tile in still_wet {
        mark_wet(canvas, tile);
    }
}
