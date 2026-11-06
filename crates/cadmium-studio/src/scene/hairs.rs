//! The simulated hairs: one camera-facing ribbon per chain segment.

use cadmium_engine::engine::BristleView;
use cadmium_engine::gpu::GpuContext;
use wgpu::util::DeviceExt;

use super::brush::{BrushShader, brush_pipeline, buffer_entry, storage_entry};
use super::view::SceneView;
use super::{layout_entry, uniform_binding};

/// A hair's drawn half-width per px of its rest radius.
const HAIR_WIDTH_SHARE: f32 = 0.35;
/// The least drawn half-width, so the finest hairs still show.
const MIN_HAIR_HALF_WIDTH_PX: f32 = 0.6;
/// The lowest a ribbon reaches: just above the canvas at z = 0, so a hair
/// lying on it never fights it for depth.
const LOWEST_HAIR_PX: f32 = 0.05;
/// A quad's two triangles.
const RIBBON_VERTICES: u32 = 6;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct HairParams {
    tint: [f32; 4],
    width: [f32; 4],
}

/// Draws the hairs.
pub(super) struct HairPass {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

impl HairPass {
    pub(super) fn new(gpu: &GpuContext, view: &SceneView) -> HairPass {
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("scene_hairs"),
                entries: &[
                    layout_entry(0, uniform_binding()),
                    storage_entry(1),
                    storage_entry(2),
                ],
            });
        let shader = BrushShader {
            label: "scene_hairs",
            body: include_str!("../shaders/hair.wgsl"),
            layout: &super::pipeline_layout(gpu, view, &layout),
            buffers: &[],
        };
        HairPass {
            pipeline: brush_pipeline(gpu, shader),
            layout,
        }
    }

    /// Draws every live bristle's chain in linear `tint`.
    pub(super) fn draw(
        &self,
        gpu: &GpuContext,
        pass: &mut wgpu::RenderPass<'_>,
        bristles: &BristleView<'_>,
        tint: [f32; 3],
    ) {
        let bind_group = self.bind_group(gpu, bristles, tint);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(1, &bind_group, &[]);
        let instances = bristles.count() * bristles.segments();
        pass.draw(0..RIBBON_VERTICES, 0..instances);
    }

    fn bind_group(
        &self,
        gpu: &GpuContext,
        bristles: &BristleView<'_>,
        [r, g, b]: [f32; 3],
    ) -> wgpu::BindGroup {
        let params = HairParams {
            tint: [r, g, b, 1.0],
            width: [
                HAIR_WIDTH_SHARE,
                MIN_HAIR_HALF_WIDTH_PX,
                LOWEST_HAIR_PX,
                0.0,
            ],
        };
        let buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scene_hairs"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene_hairs"),
            layout: &self.layout,
            entries: &[
                buffer_entry(0, &buffer),
                buffer_entry(1, bristles.states()),
                buffer_entry(2, bristles.rests()),
            ],
        })
    }
}
