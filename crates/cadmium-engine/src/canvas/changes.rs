use wgpu::util::DeviceExt;

use super::TILE_SIZE;
use crate::gpu::GpuContext;

pub(crate) const TILE_CHANGE_HEADER_WORDS: u64 = 2;
const WORD_BYTES: u64 = std::mem::size_of::<u32>() as u64;

pub(crate) struct TileChanges {
    buffer: wgpu::Buffer,
}

impl TileChanges {
    pub(crate) fn new(gpu: &GpuContext, width: u32, height: u32) -> TileChanges {
        let columns = width.div_ceil(TILE_SIZE);
        let rows = height.div_ceil(TILE_SIZE);
        let mut words = vec![0u32; TILE_CHANGE_HEADER_WORDS as usize + (columns * rows) as usize];
        words[0] = columns;
        words[1] = rows;
        let buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tile_changes"),
                contents: bytemuck::cast_slice(&words),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });
        TileChanges { buffer }
    }

    pub(crate) fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub(crate) fn encode_forget(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.clear_buffer(&self.buffer, TILE_CHANGE_HEADER_WORDS * WORD_BYTES, None);
    }
}
