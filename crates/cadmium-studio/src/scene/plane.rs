//! The canvas plane: the studio canvas on a thin dark slab at z = 0.

use cadmium_engine::canvas::{Canvas, StateTexture};
use cadmium_engine::gpu::GpuContext;
use glam::Mat4;
use wgpu::util::DeviceExt;

use super::COLOUR_FORMAT;

/// How far the slab's sides reach below the canvas, in px.
const BODY_DEPTH_PX: f32 = 8.0;
/// The top face's two triangles, then two per side.
const PLANE_VERTICES: u32 = 6 + 4 * 6;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PlaneParams {
    view_proj: [f32; 16],
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
    pub(super) fn new(gpu: &GpuContext) -> PlanePass {
        let layout = bind_group_layout(gpu);
        let pipeline = pipeline(gpu, &layout);
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

    /// Draws `canvas` as a plane covering 0..width × 0..height at z = 0.
    pub(super) fn draw(
        &self,
        gpu: &GpuContext,
        pass: &mut wgpu::RenderPass<'_>,
        canvas: &Canvas,
        view_proj: Mat4,
    ) {
        let params = PlaneParams {
            view_proj: view_proj.to_cols_array(),
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
        pass.set_bind_group(0, &bind_group, &[]);
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

fn layout_entry(binding: u32, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty,
        count: None,
    }
}

fn bind_group_layout(gpu: &GpuContext) -> wgpu::BindGroupLayout {
    let uniform = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    };
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

fn pipeline(gpu: &GpuContext, layout: &wgpu::BindGroupLayout) -> wgpu::RenderPipeline {
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene_plane"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/plane.wgsl").into()),
        });
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene_plane"),
            bind_group_layouts: &[Some(layout)],
            immediate_size: 0,
        });
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene_plane"),
            layout: Some(&pipeline_layout),
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
