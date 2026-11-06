//! What every pass sees of the camera: bind group 0, shared by the passes.

use cadmium_engine::gpu::GpuContext;
use glam::Vec3;
use wgpu::util::DeviceExt;

use crate::OrbitCamera;

/// Toward the key light, in canvas space: from above, over the canvas's top
/// left (y is down).
const TOWARD_LIGHT: Vec3 = Vec3::new(-0.35, -0.5, 0.8);

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ViewParams {
    view_proj: [f32; 16],
    /// The eye, in canvas space; w unused.
    eye: [f32; 4],
    /// Unit vector toward the key light, in canvas space; w unused.
    toward_light: [f32; 4],
}

/// The layout of the camera's bind group.
pub(super) struct SceneView {
    layout: wgpu::BindGroupLayout,
}

impl SceneView {
    pub(super) fn new(gpu: &GpuContext) -> SceneView {
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("scene_view"),
                entries: &[super::layout_entry(0, super::uniform_binding())],
            });
        SceneView { layout }
    }

    pub(super) fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// The camera's bind group for one render; built fresh each time, so
    /// several renders may share one submission.
    pub(super) fn bind_group(&self, gpu: &GpuContext, camera: &OrbitCamera) -> wgpu::BindGroup {
        let params = ViewParams {
            view_proj: camera.view_proj().to_cols_array(),
            eye: camera.eye().extend(1.0).to_array(),
            toward_light: TOWARD_LIGHT.normalize().extend(0.0).to_array(),
        };
        let buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scene_view"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene_view"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }
}
