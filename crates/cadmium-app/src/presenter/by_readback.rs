use cadmium_engine::gpu::{GpuContext, GpuLost, TexelRows, read_mapped, staging_buffer};
use cadmium_engine::present::{encode_rgba8_copy, rgba8_rows};
use gtk::prelude::*;
use gtk::{gdk, glib};

use super::FramePresenter;
use crate::state::AppState;

pub(super) struct ReadbackTarget {
    texture: wgpu::Texture,
    staging: wgpu::Buffer,
    rows: TexelRows,
}

impl ReadbackTarget {
    fn new(state: &AppState, width: u32, height: u32) -> ReadbackTarget {
        let texture = state
            .session
            .presenter
            .create_target(&state.gpu, width, height);
        let rows = rgba8_rows(&texture);
        let staging = staging_buffer(&state.gpu, "present_readback", rows.padded_size());
        ReadbackTarget {
            texture,
            staging,
            rows,
        }
    }

    pub(super) fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }

    fn download(&self, gpu: &GpuContext) -> Result<gdk::Texture, GpuLost> {
        let pixels = read_mapped(gpu, &self.staging, <[u8]>::to_vec)?;
        let (width, height) = self.size();
        Ok(gdk::MemoryTexture::new(
            width as i32,
            height as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(pixels),
            self.rows.padded_row_bytes as usize,
        )
        .upcast())
    }
}

impl FramePresenter {
    pub(super) fn present_by_readback(
        &mut self,
        state: &mut AppState,
        width_px: u32,
        height_px: u32,
    ) -> Option<gdk::Texture> {
        let size = (width_px, height_px);
        if self.readback.as_ref().map(ReadbackTarget::size) != Some(size) {
            self.readback = Some(ReadbackTarget::new(state, width_px, height_px));
        }
        let target = self
            .readback
            .as_ref()
            .expect("readback target was just created");
        let gpu = state.gpu.clone();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cadmium_frame"),
            });
        self.source
            .encode(state, &gpu, &target.texture, &mut encoder);
        encode_rgba8_copy(&target.texture, &target.staging, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        target.download(&gpu).ok()
    }
}
