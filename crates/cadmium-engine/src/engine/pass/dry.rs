use wgpu::util::DeviceExt;

use crate::canvas::{Canvas, StateTexture, TILE_SIZE, TileCoord};
use crate::compute::{
    ComputeStage, buffer_binding, storage_entry, texture_binding, texture_entry, uniform_entry,
};
use crate::engine::DRY_WORKGROUP_SIZE;
use crate::engine::shader::{DRY_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

const DRY_WORKGROUPS_PER_TILE_SIDE: u32 = TILE_SIZE / DRY_WORKGROUP_SIZE;

pub(in crate::engine) struct DryPass {
    compute: ComputeStage,
    pipeline: wgpu::ComputePipeline,
}

impl DryPass {
    pub(in crate::engine) fn new(gpu: &GpuContext) -> DryPass {
        use wgpu::StorageTextureAccess::{ReadWrite, WriteOnly};
        use wgpu::TextureFormat::{R32Float, Rgba16Float};

        let compute = ComputeStage::new(
            gpu,
            "dry",
            &shader_source(&DRY_SNIPPETS, include_str!("../../shaders/dry.wgsl")),
            &[
                uniform_entry(0, None),
                storage_entry(1, true),
                texture_entry(2, ReadWrite, Rgba16Float),
                texture_entry(3, ReadWrite, Rgba16Float),
                texture_entry(4, ReadWrite, R32Float),
                texture_entry(5, ReadWrite, Rgba16Float),
                texture_entry(6, ReadWrite, Rgba16Float),
                texture_entry(7, ReadWrite, R32Float),
                texture_entry(8, WriteOnly, Rgba16Float),
                texture_entry(9, WriteOnly, Rgba16Float),
            ],
        );
        DryPass {
            pipeline: compute.pipeline(gpu, "dry", "main"),
            compute,
        }
    }

    pub(in crate::engine) fn encode(
        &self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        tiles: &[TileCoord],
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if tiles.is_empty() {
            return;
        }
        let tile_list: Vec<[u32; 2]> = tiles.iter().map(|tile| [tile.x, tile.y]).collect();
        let tile_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("dry_tiles"),
                contents: bytemuck::cast_slice(&tile_list),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let bind_group = self.bind_group(gpu, canvas, &tile_buffer);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("dry"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(
            DRY_WORKGROUPS_PER_TILE_SIDE,
            DRY_WORKGROUPS_PER_TILE_SIDE,
            tile_list.len() as u32,
        );
        drop(pass);
        mark_dried(canvas, tiles);
    }

    fn bind_group(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        tile_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        self.compute.bind_group(
            gpu,
            &[
                buffer_binding(0, canvas.slot_table()),
                buffer_binding(1, tile_buffer),
                texture_binding(2, canvas, StateTexture::WetPig0),
                texture_binding(3, canvas, StateTexture::WetPig1),
                texture_binding(4, canvas, StateTexture::WetVol),
                texture_binding(5, canvas, StateTexture::DryRefl0),
                texture_binding(6, canvas, StateTexture::DryRefl1),
                texture_binding(7, canvas, StateTexture::DryHeight),
                texture_binding(8, canvas, StateTexture::DryTopPig0),
                texture_binding(9, canvas, StateTexture::DryTopPig1),
            ],
        )
    }
}

fn mark_dried(canvas: &mut Canvas, tiles: &[TileCoord]) {
    for tile in tiles {
        let x0 = (tile.x * TILE_SIZE) as f32;
        let y0 = (tile.y * TILE_SIZE) as f32;
        let last = (TILE_SIZE - 1) as f32;
        canvas.dirty.mark_rect(x0, y0, x0 + last, y0 + last);
    }
}
