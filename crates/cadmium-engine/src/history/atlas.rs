use crate::canvas::{Canvas, StateTexture, TILE_SIZE, TileCoord};
use crate::gpu::GpuContext;

use super::tiles::tile_extent;
use super::{ATLAS_SLOTS, ATLAS_TILES_PER_SIDE, History, MIN_SLOTS_AFTER_OUT_OF_MEMORY, Slot};

pub(super) struct Atlas {
    textures: Vec<wgpu::Texture>,
}

impl Atlas {
    pub(super) fn allocate_largest(gpu: &GpuContext, wanted_slots: u32) -> Option<(Atlas, u32)> {
        std::iter::successors(Some(wanted_slots), |&slot_capacity| {
            Some(halved_capacity(slot_capacity))
        })
        .take_while(|&slot_capacity| slot_capacity > 0)
        .find_map(|slot_capacity| Atlas::try_new(gpu, slot_capacity).zip(Some(slot_capacity)))
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

fn halved_capacity(slot_capacity: u32) -> u32 {
    if slot_capacity > MIN_SLOTS_AFTER_OUT_OF_MEMORY {
        (slot_capacity / 2).max(MIN_SLOTS_AFTER_OUT_OF_MEMORY)
    } else {
        0
    }
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

struct TileCopy<'a> {
    canvas: wgpu::TexelCopyTextureInfo<'a>,
    slot: wgpu::TexelCopyTextureInfo<'a>,
    extent: wgpu::Extent3d,
}

impl History {
    pub(super) fn copy_canvas_to_slot(
        &self,
        canvas: &Canvas,
        tile: TileCoord,
        slot: Slot,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        for copy in self.tile_copies(canvas, tile, slot) {
            encoder.copy_texture_to_texture(copy.canvas, copy.slot, copy.extent);
        }
    }

    pub(super) fn copy_slot_to_canvas(
        &self,
        canvas: &Canvas,
        tile: TileCoord,
        slot: Slot,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        for copy in self.tile_copies(canvas, tile, slot) {
            encoder.copy_texture_to_texture(copy.slot, copy.canvas, copy.extent);
        }
    }

    fn tile_copies<'a>(
        &'a self,
        canvas: &'a Canvas,
        tile: TileCoord,
        slot: Slot,
    ) -> Vec<TileCopy<'a>> {
        let (Some(extent), Some(atlas)) = (tile_extent(canvas, tile), self.atlas.as_ref()) else {
            return Vec::new();
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
        StateTexture::PAINT_STATE
            .iter()
            .zip(&atlas.textures)
            .map(|(which, atlas_texture)| TileCopy {
                canvas: texel_copy(canvas.texture(*which), canvas_origin),
                slot: texel_copy(atlas_texture, slot_origin),
                extent,
            })
            .collect()
    }
}

fn texel_copy(texture: &wgpu::Texture, origin: wgpu::Origin3d) -> wgpu::TexelCopyTextureInfo<'_> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin,
        aspect: wgpu::TextureAspect::All,
    }
}
