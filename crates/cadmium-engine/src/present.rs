use wgpu::util::DeviceExt;

use crate::canvas::{Canvas, StateTexture};
use crate::gpu::{GpuContext, GpuLost, TexelRows, read_mapped, staging_buffer};

const PRESENT_WORKGROUP_SIZE: u32 = 8;
const PRESENT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const RGBA8_BYTES_PER_TEXEL: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    pub origin_x: f32,
    pub origin_y: f32,
    pub zoom: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PresentParams {
    origin: [f32; 2],
    zoom: f32,
    _pad: f32,
}

pub struct Presenter {
    pipeline: wgpu::ComputePipeline,
    sampler: wgpu::Sampler,
}

impl Presenter {
    pub fn new(gpu: &GpuContext) -> Self {
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("present"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/present.wgsl").into()),
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("present"),
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("present_display"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Presenter { pipeline, sampler }
    }

    pub fn create_target(&self, gpu: &GpuContext, width: u32, height: u32) -> wgpu::Texture {
        gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("present_target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PRESENT_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    #[expect(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn encode_present(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        view: ViewTransform,
        target: &wgpu::Texture,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let params = PresentParams {
            origin: [view.origin_x, view.origin_y],
            zoom: view.zoom,
            _pad: 0.0,
        };
        let params_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("present_params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("present"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        canvas.view(StateTexture::Display),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&target_view),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("present"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(
            target.width().div_ceil(PRESENT_WORKGROUP_SIZE),
            target.height().div_ceil(PRESENT_WORKGROUP_SIZE),
            1,
        );
    }
}

pub fn rgba8_rows(texture: &wgpu::Texture) -> TexelRows {
    TexelRows::new(texture.width() * RGBA8_BYTES_PER_TEXEL, texture.height())
}

pub fn encode_rgba8_copy(
    texture: &wgpu::Texture,
    staging: &wgpu::Buffer,
    encoder: &mut wgpu::CommandEncoder,
) {
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: staging,
            layout: rgba8_rows(texture).layout(),
        },
        texture.size(),
    );
}

pub fn readback_rgba8(gpu: &GpuContext, texture: &wgpu::Texture) -> Result<Vec<u8>, GpuLost> {
    let rows = rgba8_rows(texture);
    let staging = staging_buffer(gpu, "present_readback", rows.padded_size());
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("present_readback"),
        });
    encode_rgba8_copy(texture, &staging, &mut encoder);
    gpu.queue.submit([encoder.finish()]);
    read_mapped(gpu, &staging, |padded| rows.unpad(padded))
}
