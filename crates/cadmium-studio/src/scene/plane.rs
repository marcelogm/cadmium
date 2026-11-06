//! The canvas plane: the studio canvas on a thin dark slab at z = 0.

use cadmium_engine::canvas::{Canvas, StateTexture};
use cadmium_engine::gpu::GpuContext;
use wgpu::util::DeviceExt;

use super::view::SceneView;
use super::{COLOUR_FORMAT, layout_entry, uniform_binding};

/// How far the slab's sides reach below the canvas, in px.
const BODY_DEPTH_PX: f32 = 8.0;
/// The top face's two triangles, then two per side.
const PLANE_VERTICES: u32 = 6 + 4 * 6;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PlaneParams {
    rect: [f32; 4],
    body: [f32; 4],
}

/// Draws the canvas plane.
pub(super) struct PlanePass {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

impl PlanePass {
    pub(super) fn new(gpu: &GpuContext, view: &SceneView) -> PlanePass {
        let layout = bind_group_layout(gpu);
        let pipeline = pipeline(gpu, &super::pipeline_layout(gpu, view, &layout));
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("scene_plane"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        PlanePass {
            layout,
            pipeline,
            sampler,
        }
    }

    /// Draws `canvas` as a plane covering 0..width × 0..height at z = 0;
    /// the camera is already bound as group 0.
    pub(super) fn draw(&self, gpu: &GpuContext, pass: &mut wgpu::RenderPass<'_>, canvas: &Canvas) {
        let params = PlaneParams {
            rect: [
                0.0,
                0.0,
                canvas.spec.width as f32,
                canvas.spec.height as f32,
            ],
            body: [BODY_DEPTH_PX, 0.0, 0.0, 0.0],
        };
        let buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scene_plane"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind_group = self.bind_group(gpu, &buffer, canvas.view(StateTexture::Display));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(1, &bind_group, &[]);
        pass.draw(0..PLANE_VERTICES, 0..1);
    }

    fn bind_group(
        &self,
        gpu: &GpuContext,
        params: &wgpu::Buffer,
        display: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene_plane"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(display),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

fn bind_group_layout(gpu: &GpuContext) -> wgpu::BindGroupLayout {
    let uniform = uniform_binding();
    let texture = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    let sampler = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene_plane"),
            entries: &[
                layout_entry(0, uniform),
                layout_entry(1, texture),
                layout_entry(2, sampler),
            ],
        })
}

fn pipeline(gpu: &GpuContext, layout: &wgpu::PipelineLayout) -> wgpu::RenderPipeline {
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene_plane"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/plane.wgsl").into()),
        });
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene_plane"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: super::primitive(),
            depth_stencil: Some(super::depth_state()),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(COLOUR_FORMAT.into())],
            }),
            multiview_mask: None,
            cache: None,
        })
}
