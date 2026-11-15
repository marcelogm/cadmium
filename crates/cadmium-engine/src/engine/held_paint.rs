use super::{
    BLADE_LOAD_BYTES, BODY_BYTES, Buffers, CAPILLARY_BYTES, TIP_CHANNELS, TIP_HALF, TIP_MARGIN,
    TIP_SIZE, WetOilEngine,
};
use crate::gpu::{GpuContext, new_encoder};

const TIP_TEXEL_BYTES: u64 = (TIP_CHANNELS * std::mem::size_of::<f32>()) as u64;

/// The square of tip texels a brush of some radius can exchange paint with.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TipSquare {
    first_texel: u32,
    side_texels: u32,
}

impl TipSquare {
    fn around(tip_radius: f32) -> TipSquare {
        let reach = tip_radius.ceil() as u32 + TIP_MARGIN;
        let first_texel = TIP_HALF.saturating_sub(reach);
        let last_texel = (TIP_HALF + reach).min(TIP_SIZE - 1);
        TipSquare {
            first_texel,
            side_texels: last_texel - first_texel + 1,
        }
    }

    fn row_bytes(self) -> u64 {
        u64::from(self.side_texels) * TIP_TEXEL_BYTES
    }

    fn bytes(self) -> u64 {
        u64::from(self.side_texels) * self.row_bytes()
    }

    /// Where each row starts, in the tip buffer and in the capture.
    fn rows(self) -> impl Iterator<Item = (u64, u64)> {
        (0..self.side_texels).map(move |row| {
            let texel = u64::from(self.first_texel + row) * u64::from(TIP_SIZE)
                + u64::from(self.first_texel);
            (texel * TIP_TEXEL_BYTES, u64::from(row) * self.row_bytes())
        })
    }
}

/// Where each part of the held paint sits in a capture.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Layout {
    tip: TipSquare,
    body_offset: u64,
    capillary_offset: u64,
    blade_offset: u64,
    bytes: u64,
}

impl Layout {
    fn for_tip(tip_radius: f32) -> Layout {
        let tip = TipSquare::around(tip_radius);
        let body_offset = tip.bytes();
        let capillary_offset = body_offset + BODY_BYTES;
        let blade_offset = capillary_offset + CAPILLARY_BYTES;
        Layout {
            tip,
            body_offset,
            capillary_offset,
            blade_offset,
            bytes: blade_offset + BLADE_LOAD_BYTES,
        }
    }

    /// The buffers captured whole, each with its place in the capture and its size.
    fn whole_parts(self, buffers: &Buffers) -> [(&wgpu::Buffer, u64, u64); 3] {
        [
            (&buffers.body, self.body_offset, BODY_BYTES),
            (
                &buffers.capillary_pools,
                self.capillary_offset,
                CAPILLARY_BYTES,
            ),
            (&buffers.blade_load, self.blade_offset, BLADE_LOAD_BYTES),
        ]
    }
}

/// The paint a tool held at some moment: the tip, the reservoir and the blade, kept on the GPU.
pub struct HeldPaint {
    buffer: wgpu::Buffer,
    layout: Layout,
    loaded_thickness_um: f32,
}

impl HeldPaint {
    pub fn bytes(&self) -> u64 {
        self.buffer.size()
    }
}

fn try_capture_buffer(gpu: &GpuContext, bytes: u64) -> Option<wgpu::Buffer> {
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("held_paint"),
        size: bytes,
        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    pollster::block_on(scope.pop()).is_none().then_some(buffer)
}

impl WetOilEngine {
    /// Counts the times the tool was loaded, dipped, cleaned or resized: held paint captured
    /// under one count is not the paint of a tool that has since moved to another.
    pub fn load_epoch(&self) -> u64 {
        self.load_epoch
    }

    pub(super) fn start_load_epoch(&mut self) {
        self.load_epoch += 1;
    }

    /// Copies the held paint as it stands where `encoder` has got to. `None` when video
    /// memory has no room for the copy; `encoder` is then left untouched.
    pub fn encode_capture_held_paint(
        &self,
        gpu: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Option<HeldPaint> {
        let layout = Layout::for_tip(self.tip_radius);
        let buffer = try_capture_buffer(gpu, layout.bytes)?;
        for (tip_offset, held_offset) in layout.tip.rows() {
            encoder.copy_buffer_to_buffer(
                &self.buffers.tip,
                tip_offset,
                &buffer,
                held_offset,
                layout.tip.row_bytes(),
            );
        }
        for (part, held_offset, bytes) in layout.whole_parts(&self.buffers) {
            encoder.copy_buffer_to_buffer(part, 0, &buffer, held_offset, bytes);
        }
        Some(HeldPaint {
            buffer,
            layout,
            loaded_thickness_um: self.loaded_thickness_um,
        })
    }

    pub fn capture_held_paint(&self, gpu: &GpuContext) -> Option<HeldPaint> {
        let mut encoder = new_encoder(gpu, "capture_held_paint");
        let held = self.encode_capture_held_paint(gpu, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        held
    }

    /// Puts `held` back in the tool. The load epoch stays where it is.
    pub fn encode_restore_held_paint(
        &mut self,
        held: &HeldPaint,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let layout = held.layout;
        for (tip_offset, held_offset) in layout.tip.rows() {
            encoder.copy_buffer_to_buffer(
                &held.buffer,
                held_offset,
                &self.buffers.tip,
                tip_offset,
                layout.tip.row_bytes(),
            );
        }
        for (part, held_offset, bytes) in layout.whole_parts(&self.buffers) {
            encoder.copy_buffer_to_buffer(&held.buffer, held_offset, part, 0, bytes);
        }
        encoder.clear_buffer(&self.buffers.tip_delta, 0, None);
        encoder.clear_buffer(&self.buffers.blade_delta, 0, None);
        encoder.clear_buffer(&self.buffers.wanted, 0, None);
        self.loaded_thickness_um = held.loaded_thickness_um;
    }

    pub fn restore_held_paint(&mut self, gpu: &GpuContext, held: &HeldPaint) {
        self.submit(gpu, "restore_held_paint", |engine, encoder| {
            engine.encode_restore_held_paint(held, encoder);
        });
    }
}

#[cfg(test)]
mod tests;
