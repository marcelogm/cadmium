use crate::canvas::{Canvas, StateTexture, TILE_SIZE, TileCoord};
use crate::gpu::GpuContext;

use super::tiles::tile_extent;
use super::{ATLAS_SLOTS, ATLAS_TILES_PER_SIDE, History, MIN_SLOTS_AFTER_OUT_OF_MEMORY, Slot};

pub(super) struct Atlas {
    textures: Vec<wgpu::Texture>,
}

impl Atlas {
    #[expect(clippy::excessive_nesting)]
    pub(super) fn allocate_largest(gpu: &GpuContext, wanted_slots: u32) -> Option<(Atlas, u32)> {
        let mut slot_capacity = wanted_slots;
        while slot_capacity > 0 {
            if let Some(atlas) = Atlas::try_new(gpu, slot_capacity) {
                return Some((atlas, slot_capacity));
            }
            slot_capacity = if slot_capacity > MIN_SLOTS_AFTER_OUT_OF_MEMORY {
                (slot_capacity / 2).max(MIN_SLOTS_AFTER_OUT_OF_MEMORY)
            } else {
                0
            };
        }
        None
    }

    fn try_new(gpu: &GpuContext, slot_capacity: u32) -> Option<Atlas> {
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let atlas = Atlas::new(gpu, slot_capacity);
        pollster::block_on(scope.pop()).is_none().then_some(atlas)
    }

    fn new(gpu: &GpuContext, slot_capacity: u32) -> Atlas {
        let columns = slot_capacity.min(ATLAS_TILES_PER_SIDE);
        let rows = slot_capacity.div_ceil(ATLAS_TILES_PER_SIDE);
        let textures = StateTexture::PAINT_STATE
            .iter()
            .map(|which| {
                gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("undo_atlas"),
                    size: wgpu::Extent3d {
                        width: columns * TILE_SIZE,
                        height: rows * TILE_SIZE,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: which.format(),
                    usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                })
            })
            .collect();
        Atlas { textures }
    }
}

#[derive(Clone, Copy)]
pub(super) enum CopyDirection {
    CanvasToSlot,
    SlotToCanvas,
}

pub(super) fn slots_for_budget(budget_bytes: u64) -> u32 {
    (budget_bytes / bytes_per_tile_all_textures()).min(u64::from(ATLAS_SLOTS)) as u32
}

pub fn bytes_per_tile_all_textures() -> u64 {
    StateTexture::PAINT_STATE
        .iter()
        .map(|which| u64::from(TILE_SIZE * TILE_SIZE * which.bytes_per_texel()))
        .sum()
}

impl History {
    #[expect(clippy::excessive_nesting, clippy::too_many_arguments)]
    pub(super) fn copy_tile(
        &self,
        canvas: &Canvas,
        tile: TileCoord,
        slot: Slot,
        direction: CopyDirection,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let Some(extent) = tile_extent(canvas, tile) else {
            return;
        };
        let Some(atlas) = self.atlas.as_ref() else {
            return;
        };
        let canvas_origin = wgpu::Origin3d {
            x: tile.x * TILE_SIZE,
            y: tile.y * TILE_SIZE,
            z: 0,
        };
        let slot_origin = wgpu::Origin3d {
            x: (slot % ATLAS_TILES_PER_SIDE) * TILE_SIZE,
            y: (slot / ATLAS_TILES_PER_SIDE) * TILE_SIZE,
            z: 0,
        };
        for (which, atlas_texture) in StateTexture::PAINT_STATE.iter().zip(&atlas.textures) {
            let canvas_copy = wgpu::TexelCopyTextureInfo {
                texture: canvas.texture(*which),
                mip_level: 0,
                origin: canvas_origin,
                aspect: wgpu::TextureAspect::All,
            };
            let slot_copy = wgpu::TexelCopyTextureInfo {
                texture: atlas_texture,
                mip_level: 0,
                origin: slot_origin,
                aspect: wgpu::TextureAspect::All,
            };
            match direction {
                CopyDirection::CanvasToSlot => {
                    encoder.copy_texture_to_texture(canvas_copy, slot_copy, extent)
                }
                CopyDirection::SlotToCanvas => {
                    encoder.copy_texture_to_texture(slot_copy, canvas_copy, extent)
                }
            }
        }
    }
}
